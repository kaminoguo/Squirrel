"""PROMPT-003: Project Summarizer agent.

Generates a short summary of what a project does from its README.
Used to give the Memory Extractor context about the project.

Timing: Called once per project, cached for reuse.
"""

import os
from pathlib import Path

from openai import AsyncOpenAI

SYSTEM_PROMPT = """Summarize this project in 2-3 sentences. Include:
- What the project does (its purpose)
- Main tech stack
- Key domain concepts

Be concise. Output plain text only, no markdown or formatting."""


class ProjectSummarizer:
    """Project Summarizer using raw OpenAI client.

    Generates a short project summary from README for context.
    """

    def __init__(self) -> None:
        self.client = AsyncOpenAI(
            base_url="https://openrouter.ai/api/v1",
            api_key=os.getenv("OPENROUTER_API_KEY"),
        )
        self.model = os.getenv("SQRL_STRONG_MODEL", "google/gemini-3-pro-preview")

    async def summarize(self, project_root: str) -> str | None:
        """Generate a summary of the project from its README.

        Args:
            project_root: Absolute path to the project.

        Returns:
            Short summary string, or None if no README found.
        """
        readme_path = self._find_readme(project_root)
        if not readme_path:
            return None

        readme_content = readme_path.read_text(encoding="utf-8", errors="ignore")
        if not readme_content.strip():
            return None

        # Truncate if too long (keep first 4000 chars)
        if len(readme_content) > 4000:
            readme_content = readme_content[:4000] + "\n...[truncated]"

        response = await self.client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": readme_content},
            ],
            max_tokens=1000,  # Gemini 3 Pro uses reasoning tokens
        )

        content = response.choices[0].message.content or ""
        return content.strip() if content.strip() else None

    def _find_readme(self, project_root: str) -> Path | None:
        """Find README file in project root."""
        root = Path(project_root)
        if not root.exists():
            return None

        # Check common README names
        for name in ["README.md", "README.rst", "README.txt", "README"]:
            readme = root / name
            if readme.exists() and readme.is_file():
                return readme

        return None
