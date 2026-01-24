"""PROMPT-002: Memory Extractor agent.

Extracts actionable memories from flagged messages.
Focus: "What should AI do differently in future sessions?"

Timing: Called only for messages flagged by Stage 1 (User Scanner).
"""

import json
import os

from openai import AsyncOpenAI

from sqrl.models.episode import ExistingProjectMemory, ExistingUserStyle
from sqrl.models.extraction import CONFIDENCE_THRESHOLD, ExtractorOutput

SYSTEM_PROMPT = """Extract behavioral adjustments from user feedback. Output JSON only.

Your task: What should AI do DIFFERENTLY in future sessions?

User Style = global preference (applies to ALL projects)
Project Memory = specific to THIS project only

Example output:
{
  "user_styles": [
    {"op": "ADD", "text": "never use emoji", "confidence": 0.9}
  ],
  "project_memories": []
}

Another example:
{
  "user_styles": [],
  "project_memories": [
    {"op": "ADD", "category": "backend", "text": "use httpx not requests", "confidence": 0.85}
  ]
}

Rules:
- op: "ADD", "UPDATE", or "DELETE"
- category (project only): "frontend", "backend", "docs_test", "other"
- confidence: 0.0-1.0 (only memories with confidence > 0.8 will be stored)
- Empty arrays if nothing worth remembering

Do NOT extract:
- One-off tasks ("make a video about X")
- Temporary debugging steps
- Universal best practices everyone knows
- Things that only apply to this exact moment
- Creative/content specifications (video scripts, character designs, marketing copy)
- Project statistics or data points for specific contexts
- Tasks unrelated to the core project (see PROJECT CONTEXT below)"""


class MemoryExtractor:
    """Memory Extractor using raw OpenAI client.

    Extracts behavioral adjustments from flagged user messages.
    """

    def __init__(self) -> None:
        self.client = AsyncOpenAI(
            base_url="https://openrouter.ai/api/v1",
            api_key=os.getenv("OPENROUTER_API_KEY"),
        )
        self.model = os.getenv("SQRL_STRONG_MODEL", "google/gemini-3-pro-preview")

    async def extract(
        self,
        project_id: str,
        project_root: str,
        trigger_message: str,
        ai_context: str,
        existing_user_styles: list[ExistingUserStyle],
        existing_project_memories: list[ExistingProjectMemory],
        project_summary: str | None = None,
        apply_confidence_filter: bool = True,
    ) -> ExtractorOutput:
        """Extract memories from user message with AI context.

        Args:
            project_id: Project identifier.
            project_root: Absolute path to project.
            trigger_message: The user message flagged by Stage 1.
            ai_context: The 3 AI turns before the trigger message.
            existing_user_styles: Current user style items.
            existing_project_memories: Current project memories.
            project_summary: Short summary of what the project does.
            apply_confidence_filter: If True, filter out low-confidence memories.

        Returns:
            ExtractorOutput with extracted memories (filtered by confidence if enabled).
        """
        styles_json = json.dumps(
            [{"id": s.id, "text": s.text} for s in existing_user_styles],
        )
        memories_json = json.dumps(
            [
                {"id": m.id, "category": m.category, "text": m.text}
                for m in existing_project_memories
            ],
        )

        # Build project context section
        if project_summary:
            project_context = f"""PROJECT CONTEXT:
{project_summary}

Only extract memories relevant to this project. Skip unrelated side tasks."""
        else:
            project_context = f"PROJECT: {project_id}"

        user_prompt = f"""{project_context}

EXISTING USER STYLES: {styles_json}
EXISTING PROJECT MEMORIES: {memories_json}

AI CONTEXT (what AI did before user's message):
{ai_context}

USER MESSAGE (the feedback):
{trigger_message}

What behavioral adjustments should be remembered for future sessions?"""

        response = await self.client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": user_prompt},
            ],
        )

        content = response.choices[0].message.content or "{}"
        # Extract JSON from response (may be wrapped in markdown)
        if "```json" in content:
            content = content.split("```json")[1].split("```")[0]
        elif "```" in content:
            content = content.split("```")[1].split("```")[0]

        try:
            data = json.loads(content.strip())
            output = ExtractorOutput(**data)

            # Apply confidence threshold filter
            if apply_confidence_filter:
                output = output.filter_by_confidence(CONFIDENCE_THRESHOLD)

            return output
        except (json.JSONDecodeError, ValueError):
            return ExtractorOutput()
