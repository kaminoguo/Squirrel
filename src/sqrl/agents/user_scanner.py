"""PROMPT-001: User Scanner agent.

Scans user messages from a completed session to identify
which messages contain behavioral patterns worth extracting.

Timing: Called at session-end (idle timeout or explicit flush), not per-message.
"""

import json
import os

from openai import AsyncOpenAI

from sqrl.models.extraction import ScannerOutput

SYSTEM_PROMPT = """You scan user messages from a coding session to identify behavioral patterns.

Look for messages where:
- User corrects AI behavior ("no, use X instead", "don't do Y")
- User expresses preferences ("I prefer...", "always...", "never...")
- User shows frustration with AI's approach
- User redirects AI's direction

Skip messages that are:
- Just acknowledgments ("ok", "sure", "continue", "looks good")
- Questions or requests without correction intent
- One-off task instructions unrelated to preferences

Output JSON only:
{"has_patterns": true, "indices": [1, 5, 12]}
or
{"has_patterns": false, "indices": []}"""


class UserScanner:
    """User Scanner using raw OpenAI client.

    Scans user messages at session-end to identify behavioral patterns.
    """

    def __init__(self) -> None:
        self.client = AsyncOpenAI(
            base_url="https://openrouter.ai/api/v1",
            api_key=os.getenv("OPENROUTER_API_KEY"),
        )
        self.model = os.getenv("SQRL_CHEAP_MODEL", "google/gemini-3-flash-preview")

    async def scan(self, user_messages: list[str]) -> ScannerOutput:
        """Scan user messages for behavioral patterns.

        Args:
            user_messages: All user messages from the session.

        Returns:
            ScannerOutput with has_patterns flag and indices of flagged messages.
        """
        messages_text = "\n".join(
            f"[{i}] {msg}" for i, msg in enumerate(user_messages)
        )
        user_prompt = f"""USER MESSAGES FROM SESSION:
{messages_text}

Which messages (if any) contain behavioral patterns worth extracting? Return JSON with indices."""

        response = await self.client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": user_prompt},
            ],
        )

        content = response.choices[0].message.content or "{}"
        # Extract JSON from response
        if "```json" in content:
            content = content.split("```json")[1].split("```")[0]
        elif "```" in content:
            content = content.split("```")[1].split("```")[0]

        try:
            data = json.loads(content.strip())
            return ScannerOutput(**data)
        except (json.JSONDecodeError, ValueError):
            return ScannerOutput(has_patterns=False, indices=[])
