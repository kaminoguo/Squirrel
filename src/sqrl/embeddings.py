"""
Embeddings module for Squirrel (IPC-002).

Handles text embedding generation for memory storage and retrieval.
Uses LiteLLM for provider-agnostic embeddings.
"""

import asyncio
import logging
import os
import struct
import time
from dataclasses import dataclass, field
from typing import Optional

import litellm

logger = logging.getLogger(__name__)


class EmbeddingError(Exception):
    """Error during embedding generation."""

    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message
        super().__init__(message)


# Error codes per IPC-002 spec
ERROR_EMPTY_TEXT = -32040
ERROR_EMBEDDING_FAILED = -32041


@dataclass
class EmbeddingConfig:
    """Configuration for embedding generation.

    Attributes:
        model: Embedding model to use (default: text-embedding-3-small)
        dimensions: Expected embedding dimensions (default: 1536)
        max_retries: Maximum retry attempts on failure (default: 3)
        retry_delay: Base delay between retries in seconds (default: 1.0)
        retry_backoff: Backoff multiplier for retry delay (default: 2.0)
    """

    model: str = "text-embedding-3-small"
    dimensions: int = 1536
    max_retries: int = 3
    retry_delay: float = 1.0
    retry_backoff: float = 2.0

    @classmethod
    def from_env(cls) -> "EmbeddingConfig":
        """Create config from environment variables."""
        return cls(
            model=os.getenv("SQRL_EMBEDDING_MODEL", "text-embedding-3-small"),
            dimensions=int(os.getenv("SQRL_EMBEDDING_DIMS", "1536")),
            max_retries=int(os.getenv("SQRL_EMBEDDING_MAX_RETRIES", "3")),
            retry_delay=float(os.getenv("SQRL_EMBEDDING_RETRY_DELAY", "1.0")),
            retry_backoff=float(os.getenv("SQRL_EMBEDDING_RETRY_BACKOFF", "2.0")),
        )


def _is_retryable_error(e: Exception) -> bool:
    """Check if an error is retryable (transient network/API issues)."""
    error_str = str(e).lower()
    retryable_patterns = [
        "rate limit",
        "timeout",
        "connection",
        "temporarily unavailable",
        "503",
        "502",
        "500",
        "429",
    ]
    return any(pattern in error_str for pattern in retryable_patterns)


async def embed_text(
    text: str,
    config: Optional[EmbeddingConfig] = None,
) -> list[float]:
    """
    Generate embedding for text (IPC-002) with retry logic.

    Args:
        text: Text to embed (must not be empty)
        config: Optional embedding config

    Returns:
        1536-dim float32 vector

    Raises:
        EmbeddingError: With code -32040 if text empty, -32041 if API fails after retries
    """
    if not text or not text.strip():
        raise EmbeddingError(ERROR_EMPTY_TEXT, "Empty text")

    cfg = config or EmbeddingConfig.from_env()

    last_error: Optional[Exception] = None
    delay = cfg.retry_delay

    for attempt in range(cfg.max_retries + 1):
        try:
            response = await litellm.aembedding(
                model=cfg.model,
                input=text,
            )
            return response.data[0]["embedding"]
        except Exception as e:
            last_error = e

            # Don't retry on non-retryable errors
            if not _is_retryable_error(e):
                logger.warning(f"Embedding failed (non-retryable): {e}")
                break

            # Don't wait after the last attempt
            if attempt < cfg.max_retries:
                logger.warning(
                    f"Embedding attempt {attempt + 1}/{cfg.max_retries + 1} failed: {e}. "
                    f"Retrying in {delay:.1f}s..."
                )
                await asyncio.sleep(delay)
                delay *= cfg.retry_backoff

    raise EmbeddingError(
        ERROR_EMBEDDING_FAILED,
        f"Embedding failed after {cfg.max_retries + 1} attempts: {last_error}",
    ) from last_error


def embed_text_sync(
    text: str,
    config: Optional[EmbeddingConfig] = None,
) -> list[float]:
    """
    Synchronous version of embed_text (IPC-002) with retry logic.

    Args:
        text: Text to embed (must not be empty)
        config: Optional embedding config

    Returns:
        1536-dim float32 vector

    Raises:
        EmbeddingError: With code -32040 if text empty, -32041 if API fails after retries
    """
    if not text or not text.strip():
        raise EmbeddingError(ERROR_EMPTY_TEXT, "Empty text")

    cfg = config or EmbeddingConfig.from_env()

    last_error: Optional[Exception] = None
    delay = cfg.retry_delay

    for attempt in range(cfg.max_retries + 1):
        try:
            response = litellm.embedding(
                model=cfg.model,
                input=text,
            )
            return response.data[0]["embedding"]
        except Exception as e:
            last_error = e

            # Don't retry on non-retryable errors
            if not _is_retryable_error(e):
                logger.warning(f"Embedding failed (non-retryable): {e}")
                break

            # Don't wait after the last attempt
            if attempt < cfg.max_retries:
                logger.warning(
                    f"Embedding attempt {attempt + 1}/{cfg.max_retries + 1} failed: {e}. "
                    f"Retrying in {delay:.1f}s..."
                )
                time.sleep(delay)
                delay *= cfg.retry_backoff

    raise EmbeddingError(
        ERROR_EMBEDDING_FAILED,
        f"Embedding failed after {cfg.max_retries + 1} attempts: {last_error}",
    ) from last_error


def embedding_to_bytes(embedding: list[float]) -> bytes:
    """
    Convert embedding list to bytes for SQLite storage.

    Uses little-endian float32 format for sqlite-vec compatibility.

    Args:
        embedding: List of float values

    Returns:
        Packed bytes (4 bytes per float)
    """
    return struct.pack(f"<{len(embedding)}f", *embedding)


def bytes_to_embedding(data: bytes) -> list[float]:
    """
    Convert bytes back to embedding list.

    Args:
        data: Packed bytes from embedding_to_bytes

    Returns:
        List of float values
    """
    count = len(data) // 4  # 4 bytes per float32
    return list(struct.unpack(f"<{count}f", data))


def make_embedding_getter(
    config: Optional[EmbeddingConfig] = None,
) -> callable:
    """
    Create an embedding getter function for commit_memory_ops.

    Returns a sync function that takes text and returns bytes.

    Args:
        config: Optional embedding config

    Returns:
        Function: (text: str) -> bytes
    """
    cfg = config or EmbeddingConfig.from_env()

    def get_embedding(text: str) -> bytes:
        embedding = embed_text_sync(text, cfg)
        return embedding_to_bytes(embedding)

    return get_embedding
