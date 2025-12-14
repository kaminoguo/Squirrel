"""Embedding generation for Squirrel (IPC-002).

Uses OpenAI text-embedding-3-small (1536 dimensions).
"""

import os
from typing import List

import structlog
from openai import OpenAI

log = structlog.get_logger()

# Default model per ARCHITECTURE.md
DEFAULT_MODEL = "text-embedding-3-small"
EMBEDDING_DIM = 1536


class EmbeddingService:
    """Generate embeddings using OpenAI API."""

    def __init__(self, model: str = DEFAULT_MODEL):
        self.model = model
        self.client = OpenAI(api_key=os.environ.get("OPENAI_API_KEY"))

    def embed_text(self, text: str) -> List[float]:
        """Generate embedding for text.

        Args:
            text: Text to embed.

        Returns:
            1536-dimensional float vector.

        Raises:
            ValueError: If text is empty.
            openai.APIError: If API call fails.
        """
        if not text or not text.strip():
            raise ValueError("Empty text")

        log.debug("generating_embedding", text_length=len(text), model=self.model)

        response = self.client.embeddings.create(
            model=self.model,
            input=text,
        )

        embedding = response.data[0].embedding
        log.debug("embedding_generated", dimension=len(embedding))

        return embedding


# Module-level singleton
_service: EmbeddingService | None = None


def get_embedding_service() -> EmbeddingService:
    """Get or create embedding service singleton."""
    global _service
    if _service is None:
        _service = EmbeddingService()
    return _service


def embed_text(text: str) -> List[float]:
    """Convenience function for embedding text."""
    return get_embedding_service().embed_text(text)
