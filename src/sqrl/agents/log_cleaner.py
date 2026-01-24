"""PROMPT-001: Log Cleaner agent."""

import os

from pydantic_ai import Agent
from pydantic_ai.models.openai import OpenAIModel
from pydantic_ai.providers.openai import OpenAIProvider

from sqrl.models.episode import EpisodeEvent
from sqrl.models.extraction import CleanerOutput

SYSTEM_PROMPT = """You are the Log Cleaner for Squirrel, a coding memory system.

Your job: Detect if the user CORRECTED the AI or expressed FRUSTRATION.

## KEEP (worth remembering):
- User corrects AI: "no, use X instead of Y", "I said Z not W"
- User frustrated: profanity, "I told you many times", "can't you understand?", repeated corrections
- User states preference: "always do X", "never do Y", "I prefer Z"

## SKIP (not worth remembering):
- AI solved problem through trial and error (no user correction)
- Pure browsing: listing files, reading code
- User just says "ok", "sure", "continue", "looks good"
- Technical errors that AI fixed on its own
- Architecture discussions (should go in docs, not memory)

## Key Principle:
If AI figured it out on its own → SKIP
If user had to correct AI → KEEP

OUTPUT (JSON only):
{
  "skip": true | false,
  "skip_reason": "string if skip=true",
  "correction_context": "if skip=false: what did AI do wrong, what did user say to correct it"
}"""


def _get_model() -> OpenAIModel:
    """Get model configured for OpenRouter."""
    model_name = os.getenv("SQRL_CHEAP_MODEL", "google/gemini-2.0-flash-001")
    api_key = os.getenv("OPENROUTER_API_KEY")
    provider = OpenAIProvider(
        base_url="https://openrouter.ai/api/v1",
        api_key=api_key,
    )
    return OpenAIModel(model_name, provider=provider)


class LogCleaner:
    """Log Cleaner agent using PydanticAI."""

    def __init__(self) -> None:
        self.agent = Agent(
            model=_get_model(),
            system_prompt=SYSTEM_PROMPT,
            output_type=CleanerOutput,
            retries=3,
        )

    async def clean(
        self, project_id: str, events: list[EpisodeEvent]
    ) -> CleanerOutput:
        """Clean and compress episode events."""
        events_text = "\n".join(
            f"[{e.ts}] {e.role}: {e.content_summary}" for e in events
        )
        user_prompt = f"""PROJECT: {project_id}

EVENTS:
{events_text}

Did the user correct the AI or express frustration? Return JSON only."""

        result = await self.agent.run(user_prompt)
        return result.output
