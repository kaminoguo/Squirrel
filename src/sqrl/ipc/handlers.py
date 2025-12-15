"""
IPC method handlers for Squirrel.

Implements IPC-001, IPC-002, IPC-003, IPC-004 from INTERFACES.md.
"""

from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Optional

from sqrl.cr_memory import (
    CRMemoryEvaluator,
    EvalResult,
    Memory,
    MemoryMetrics,
    Policy,
    load_policy,
)
from sqrl.embeddings import (
    EmbeddingConfig,
    EmbeddingError,
    embed_text,
)
from sqrl.memory_writer import MemoryWriter, MemoryWriterConfig
from sqrl.parsers.base import Event, EventKind, Role


class IPCError(Exception):
    """Error with JSON-RPC error code."""

    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message
        super().__init__(message)


# Error codes from INTERFACES.md
ERROR_CHUNK_EMPTY = -32001
ERROR_INVALID_PROJECT = -32002
ERROR_LLM_ERROR = -32003
ERROR_COMPOSE_EMPTY_TASK = -32010
ERROR_SEARCH_PROJECT_NOT_INIT = -32010
ERROR_SEARCH_EMPTY_QUERY = -32012
ERROR_CR_MEMORY_EMPTY = -32020
ERROR_CR_MEMORY_EVAL = -32021


def _parse_event(data: dict) -> Event:
    """Parse event dict to Event object."""
    return Event(
        ts=datetime.fromisoformat(data["ts"].replace("Z", "+00:00")),
        role=Role(data.get("role", "assistant")),
        kind=EventKind(data.get("kind", "message")),
        summary=data.get("summary", ""),
        tool_name=data.get("tool_name"),
        file=data.get("file"),
        raw_snippet=data.get("raw_snippet"),
        is_error=data.get("is_error", False),
    )


@dataclass
class IngestChunkHandler:
    """
    Handler for IPC-001: ingest_chunk.

    Daemon → Memory Service. Process event chunk with Memory Writer.
    """

    writer: MemoryWriter

    def __init__(self, config: Optional[MemoryWriterConfig] = None):
        self.writer = MemoryWriter(config=config)

    async def __call__(self, params: dict) -> dict:
        """
        Process ingest_chunk request.

        Args:
            params: Request params with project_id, events, etc.

        Returns:
            Dict with episodes, memories, carry_state, discard_reason
        """
        # Validate required params
        events = params.get("events", [])
        if not events:
            raise IPCError(ERROR_CHUNK_EMPTY, "Chunk empty")

        project_id = params.get("project_id")
        if not project_id:
            raise IPCError(ERROR_INVALID_PROJECT, "Invalid project")

        owner_type = params.get("owner_type", "user")
        owner_id = params.get("owner_id", "default")
        chunk_index = params.get("chunk_index", 0)
        carry_state = params.get("carry_state")
        recent_memories = params.get("recent_memories", [])

        # Parse raw event dicts to Event objects
        parsed_events = [_parse_event(e) for e in events]

        try:
            result = await self.writer.process_chunk(
                events=parsed_events,
                project_id=project_id,
                owner_type=owner_type,
                owner_id=owner_id,
                chunk_index=chunk_index,
                carry_state=carry_state,
                recent_memories=recent_memories,
            )

            # Convert to JSON-serializable dict
            return {
                "episodes": [
                    {
                        "start_idx": ep.start_idx,
                        "end_idx": ep.end_idx,
                        "label": ep.label,
                    }
                    for ep in result.episodes
                ],
                "memories": [
                    {
                        "op": mem.op.value,
                        "target_memory_id": mem.target_memory_id,
                        "episode_idx": mem.episode_idx,
                        "scope": mem.scope.value,
                        "owner_type": mem.owner_type.value,
                        "owner_id": mem.owner_id,
                        "kind": mem.kind.value,
                        "tier": mem.tier.value,
                        "polarity": mem.polarity,
                        "key": mem.key,
                        "text": mem.text,
                        "ttl_days": mem.ttl_days,
                        "confidence": mem.confidence,
                        "evidence": {
                            "source": mem.evidence.source.value,
                            "frustration": mem.evidence.frustration.value,
                        },
                    }
                    for mem in result.memories
                ],
                "carry_state": result.carry_state,
                "discard_reason": result.discard_reason,
            }

        except Exception as e:
            raise IPCError(ERROR_LLM_ERROR, f"LLM error: {e}") from e


@dataclass
class EmbedTextHandler:
    """
    Handler for IPC-002: embed_text.

    Daemon → Memory Service. Generate embedding vector for text.
    """

    config: EmbeddingConfig

    def __init__(self, config: Optional[EmbeddingConfig] = None):
        self.config = config or EmbeddingConfig.from_env()

    async def __call__(self, params: dict) -> dict:
        """
        Process embed_text request.

        Args:
            params: Request params with text field

        Returns:
            Dict with embedding array
        """
        text = params.get("text", "")

        try:
            embedding = await embed_text(text, self.config)
            return {"embedding": embedding}

        except EmbeddingError as e:
            raise IPCError(e.code, e.message) from e


@dataclass
class ComposeContextHandler:
    """
    Handler for IPC-003: compose_context.

    Optional LLM-based context composition. v1 uses templates.
    """

    async def __call__(self, params: dict) -> dict:
        """
        Compose context from memories.

        For v1, uses simple template-based composition.
        """
        task = params.get("task", "")
        if not task:
            raise IPCError(ERROR_COMPOSE_EMPTY_TASK, "Empty task")

        memories = params.get("memories", [])
        token_budget = params.get("token_budget", 400)

        # Group memories by kind
        by_kind: dict[str, list[dict]] = {}
        for mem in memories:
            kind = mem.get("kind", "note")
            if kind not in by_kind:
                by_kind[kind] = []
            by_kind[kind].append(mem)

        # Build context prompt
        lines = []
        used_ids = []

        # Order: guards first (warnings), then invariants, preferences, patterns, notes
        kind_order = ["guard", "invariant", "preference", "pattern", "note"]

        for kind in kind_order:
            if kind not in by_kind:
                continue

            kind_label = kind.title() + "s"
            lines.append(f"**{kind_label}:**")

            for mem in by_kind[kind]:
                mem_id = mem.get("id", "")
                text = mem.get("text", "")
                lines.append(f"- {text}")
                if mem_id:
                    used_ids.append(mem_id)

            lines.append("")

        context_prompt = "\n".join(lines).strip()

        # Simple token estimation (rough)
        estimated_tokens = len(context_prompt.split()) * 1.3
        if estimated_tokens > token_budget:
            # Truncate (simple approach for v1)
            words = context_prompt.split()
            max_words = int(token_budget / 1.3)
            context_prompt = " ".join(words[:max_words]) + "..."

        return {
            "context_prompt": context_prompt,
            "used_memory_ids": used_ids,
        }


@dataclass
class SearchMemoriesHandler:
    """
    Handler for IPC-004: search_memories.

    Note: In production, search is handled by Rust daemon with sqlite-vec.
    This handler is for testing/development with Python-side search.
    """

    def __init__(self, search_fn: Optional[callable] = None):
        self.search_fn = search_fn

    async def __call__(self, params: dict) -> dict:
        """
        Search memories by query.

        Args:
            params: Request params with project_id, query, top_k, filters

        Returns:
            Dict with results array
        """
        query = params.get("query", "")
        if not query:
            raise IPCError(ERROR_SEARCH_EMPTY_QUERY, "Empty query")

        project_id = params.get("project_id")
        if not project_id:
            raise IPCError(ERROR_SEARCH_PROJECT_NOT_INIT, "Project not initialized")

        top_k = params.get("top_k", 10)
        filters = params.get("filters", {})

        # If search function provided, use it
        if self.search_fn:
            results = await self.search_fn(
                project_id=project_id,
                query=query,
                top_k=top_k,
                filters=filters,
            )
            return {"results": results}

        # Otherwise return empty (search handled by Rust daemon)
        return {"results": []}


@dataclass
class EvaluateMemoriesHandler:
    """
    Handler for IPC-005: evaluate_memories.

    Daemon → Memory Service. Run CR-Memory evaluation on memories.
    """

    policy: Policy
    evaluator: CRMemoryEvaluator

    def __init__(self, policy: Optional[Policy] = None):
        self.policy = policy or load_policy()
        self.evaluator = CRMemoryEvaluator(self.policy)

    async def __call__(self, params: dict) -> dict:
        """
        Evaluate memories for promotion/deprecation.

        Args:
            params: Request params with memories list

        Returns:
            Dict with decisions array
        """
        memories_data = params.get("memories", [])
        if not memories_data:
            raise IPCError(ERROR_CR_MEMORY_EMPTY, "No memories to evaluate")

        now_str = params.get("now")
        if now_str:
            now = datetime.fromisoformat(now_str.replace("Z", "+00:00"))
        else:
            now = datetime.now(timezone.utc)

        decisions = []

        for m_data in memories_data:
            # Parse memory
            memory = Memory(
                id=m_data["id"],
                kind=m_data.get("kind", "note"),
                status=m_data.get("status", "provisional"),
                tier=m_data.get("tier", "short_term"),
                expires_at=_parse_datetime(m_data.get("expires_at")),
            )

            # Parse metrics
            metrics_data = m_data.get("metrics", {})
            metrics = MemoryMetrics(
                memory_id=memory.id,
                use_count=metrics_data.get("use_count", 0),
                opportunities=metrics_data.get("opportunities", 0),
                suspected_regret_hits=metrics_data.get("suspected_regret_hits", 0),
                estimated_regret_saved=metrics_data.get("estimated_regret_saved", 0.0),
                last_used_at=_parse_datetime(metrics_data.get("last_used_at")),
                last_evaluated_at=_parse_datetime(metrics_data.get("last_evaluated_at")),
            )

            # Evaluate
            decision = self.evaluator.evaluate(memory, metrics, now)

            # Only include if there's a change
            if decision.result != EvalResult.NO_CHANGE:
                new_expires = None
                if decision.new_expires_at:
                    new_expires = decision.new_expires_at.isoformat()
                decisions.append({
                    "memory_id": decision.memory_id,
                    "result": decision.result.value,
                    "new_status": decision.new_status,
                    "new_tier": decision.new_tier,
                    "new_expires_at": new_expires,
                    "reason": decision.reason,
                })

        return {"decisions": decisions}


def _parse_datetime(value: Optional[str]) -> Optional[datetime]:
    """Parse ISO 8601 datetime string."""
    if value is None:
        return None
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def create_handlers(
    writer_config: Optional[MemoryWriterConfig] = None,
    embedding_config: Optional[EmbeddingConfig] = None,
    policy: Optional[Policy] = None,
) -> dict[str, Any]:
    """
    Create all IPC handlers.

    Returns:
        Dict mapping method names to handler instances
    """
    return {
        "ingest_chunk": IngestChunkHandler(writer_config),
        "embed_text": EmbedTextHandler(embedding_config),
        "compose_context": ComposeContextHandler(),
        "search_memories": SearchMemoriesHandler(),
        "evaluate_memories": EvaluateMemoriesHandler(policy),
    }
