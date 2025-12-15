"""Memory Writer - LLM-based episode processing using PydanticAI."""

import json
import os
from dataclasses import dataclass, field
from typing import Any, Optional

from pydantic_ai import Agent, RunContext
from pydantic_ai.models.openai import OpenAIChatModel

from sqrl.chunking import events_to_json
from sqrl.memory_writer.models import MemoryWriterOutput
from sqrl.memory_writer.prompts import SYSTEM_PROMPT, USER_PROMPT_TEMPLATE
from sqrl.parsers.base import Event

# Supported PydanticAI providers
SUPPORTED_PROVIDERS = {"openrouter", "openai", "anthropic", "ollama", "together", "fireworks"}


@dataclass
class MemoryWriterConfig:
    """Configuration for Memory Writer.

    Model format: 'provider/model-name'
    - Examples: 'openrouter/anthropic/claude-3-haiku', 'openai/gpt-4o'
    - For OpenRouter: 'openrouter/<provider>/<model>'
    - Set SQRL_STRONG_MODEL env var

    max_memories_per_episode: Default 5. Limits noise while capturing key learnings.
    - Set SQRL_MAX_MEMORIES_PER_EPISODE env var to override
    """

    provider: str = field(default="")
    model_name: str = field(default="")
    max_memories_per_episode: int = 5

    def __post_init__(self):
        # Model from env var (required)
        env_model = os.getenv("SQRL_STRONG_MODEL")
        if not env_model:
            raise ValueError(
                "SQRL_STRONG_MODEL env var required. "
                "Format: 'provider/model' (e.g., 'openrouter/anthropic/claude-3-haiku')"
            )

        # Parse provider/model format
        parts = env_model.split("/", 1)
        if len(parts) < 2:
            raise ValueError(
                f"Invalid model format: {env_model}. "
                "Expected 'provider/model' (e.g., 'openrouter/anthropic/claude-3-haiku')"
            )

        self.provider = parts[0]
        self.model_name = parts[1]

        if self.provider not in SUPPORTED_PROVIDERS:
            raise ValueError(
                f"Unsupported provider: {self.provider}. "
                f"Supported: {', '.join(sorted(SUPPORTED_PROVIDERS))}"
            )

        # max_memories_per_episode from env or default (5)
        env_max = os.getenv("SQRL_MAX_MEMORIES_PER_EPISODE")
        if env_max:
            self.max_memories_per_episode = int(env_max)

    def create_model(self) -> Any:
        """Create PydanticAI model instance."""
        return OpenAIChatModel(model_name=self.model_name, provider=self.provider)


@dataclass
class ChunkContext:
    """Context passed to PydanticAI agent for each chunk."""

    events_json: str
    project_id: str
    owner_type: str
    owner_id: str
    chunk_index: int
    carry_state: str
    recent_memories: str
    # Default 5 per episode
    max_memories_per_episode: int


class MemoryWriter:
    """
    Memory Writer for processing event chunks.

    Uses PydanticAI to call LLM with PROMPT-001:
    1. Detect episode boundaries
    2. Extract memories
    3. Manage carry-over state
    """

    def __init__(self, config: Optional[MemoryWriterConfig] = None):
        self.config = config or MemoryWriterConfig()
        self._agent = self._create_agent()

    def _create_agent(self) -> Agent[ChunkContext, MemoryWriterOutput]:
        """Create PydanticAI agent with structured output."""
        model = self.config.create_model()
        agent = Agent(
            model,
            deps_type=ChunkContext,
            output_type=MemoryWriterOutput,
        )

        @agent.system_prompt
        def build_system_prompt(ctx: RunContext[ChunkContext]) -> str:
            return SYSTEM_PROMPT.format(
                max_memories_per_episode=ctx.deps.max_memories_per_episode,
            )

        return agent

    def _format_memories(self, memories: list[dict]) -> str:
        """Format existing memories for context."""
        if not memories:
            return "(none)"

        lines = []
        for m in memories:
            lines.append(f"- [{m.get('kind', '?')}] {m.get('text', '')}")
        return "\n".join(lines)

    def _build_user_prompt(self, ctx: ChunkContext) -> str:
        """Build the user prompt from context."""
        return USER_PROMPT_TEMPLATE.format(
            project_id=ctx.project_id,
            owner_type=ctx.owner_type,
            owner_id=ctx.owner_id,
            carry_state=ctx.carry_state,
            recent_memories=ctx.recent_memories,
            chunk_index=ctx.chunk_index,
            events=ctx.events_json,
            max_memories_per_episode=ctx.max_memories_per_episode,
        )

    async def process_chunk(
        self,
        events: list[Event],
        project_id: str,
        owner_type: str = "user",
        owner_id: str = "default",
        chunk_index: int = 0,
        carry_state: Optional[str] = None,
        recent_memories: Optional[list[dict]] = None,
    ) -> MemoryWriterOutput:
        """
        Process a chunk of events through the Memory Writer.

        Args:
            events: List of Event objects to process
            project_id: Project identifier
            owner_type: Owner type (user/team/org)
            owner_id: Owner identifier
            chunk_index: Index of this chunk (for logging)
            carry_state: State carried from previous chunk
            recent_memories: Existing memories for context

        Returns:
            MemoryWriterOutput with episodes, memories, and carry_state
        """
        if recent_memories is None:
            recent_memories = []

        events_json = json.dumps(events_to_json(events), indent=2)

        ctx = ChunkContext(
            events_json=events_json,
            project_id=project_id,
            owner_type=owner_type,
            owner_id=owner_id,
            chunk_index=chunk_index,
            carry_state=carry_state or "(none)",
            recent_memories=self._format_memories(recent_memories),
            max_memories_per_episode=self.config.max_memories_per_episode,
        )

        user_prompt = self._build_user_prompt(ctx)
        result = await self._agent.run(user_prompt, deps=ctx)

        return result.output

    def process_chunk_sync(
        self,
        events: list[Event],
        project_id: str,
        owner_type: str = "user",
        owner_id: str = "default",
        chunk_index: int = 0,
        carry_state: Optional[str] = None,
        recent_memories: Optional[list[dict]] = None,
    ) -> MemoryWriterOutput:
        """Synchronous wrapper for process_chunk."""
        if recent_memories is None:
            recent_memories = []

        events_json = json.dumps(events_to_json(events), indent=2)

        ctx = ChunkContext(
            events_json=events_json,
            project_id=project_id,
            owner_type=owner_type,
            owner_id=owner_id,
            chunk_index=chunk_index,
            carry_state=carry_state or "(none)",
            recent_memories=self._format_memories(recent_memories),
            max_memories_per_episode=self.config.max_memories_per_episode,
        )

        user_prompt = self._build_user_prompt(ctx)
        result = self._agent.run_sync(user_prompt, deps=ctx)

        return result.output
