"""
Retrieval module for Squirrel.

Handles embedding generation and memory retrieval.
For v1 testing: in-memory storage with cosine similarity.
Production: Rust daemon handles SQLite + sqlite-vec.
"""

import os
from dataclasses import dataclass
from typing import Optional

import litellm
import numpy as np

from sqrl.memory_writer.models import MemoryKind, MemoryOp, MemoryTier


@dataclass
class RetrievalConfig:
    """Configuration for retrieval.

    embedding_model: OpenAI text-embedding-3-small (1536 dimensions)
    - Default uses text-embedding-3-small as per IPC-002 spec
    - Set SQRL_EMBEDDING_MODEL env var to override
    """

    # Default: text-embedding-3-small (1536 dimensions, per IPC-002)
    embedding_model: str | None = None

    def __post_init__(self):
        self.embedding_model = os.getenv(
            "SQRL_EMBEDDING_MODEL", "text-embedding-3-small"
        )


@dataclass
class StoredMemory:
    """Memory with embedding for retrieval."""

    memory: MemoryOp
    embedding: list[float]
    # For testing: simple unique id
    id: str = ""


@dataclass
class RetrievalResult:
    """Result of a retrieval query."""

    memories: list[StoredMemory]
    scores: list[float]


class MemoryStore:
    """
    In-memory store for testing.

    Production uses SQLite + sqlite-vec via Rust daemon.
    """

    def __init__(self):
        self.memories: list[StoredMemory] = []
        self._next_id = 0

    def add(self, memory: MemoryOp, embedding: list[float]) -> str:
        """Add a memory with its embedding."""
        mem_id = f"mem_{self._next_id}"
        self._next_id += 1
        self.memories.append(
            StoredMemory(memory=memory, embedding=embedding, id=mem_id)
        )
        return mem_id

    def clear(self):
        """Clear all memories."""
        self.memories = []
        self._next_id = 0


class Retriever:
    """
    Retrieval engine for memories.

    For v1 testing: in-memory storage + cosine similarity.
    """

    def __init__(
        self,
        config: Optional[RetrievalConfig] = None,
        store: Optional[MemoryStore] = None,
    ):
        self.config = config or RetrievalConfig()
        self.store = store or MemoryStore()

    async def embed_text(self, text: str) -> list[float]:
        """Generate embedding for text using LiteLLM."""
        response = await litellm.aembedding(
            model=self.config.embedding_model,
            input=text,
        )
        return response.data[0]["embedding"]

    def embed_text_sync(self, text: str) -> list[float]:
        """Synchronous embedding generation."""
        response = litellm.embedding(
            model=self.config.embedding_model,
            input=text,
        )
        return response.data[0]["embedding"]

    def _cosine_similarity(self, a: list[float], b: list[float]) -> float:
        """Compute cosine similarity between two vectors."""
        a_np = np.array(a)
        b_np = np.array(b)
        return float(np.dot(a_np, b_np) / (np.linalg.norm(a_np) * np.linalg.norm(b_np)))

    def _rank_score(self, memory: MemoryOp, similarity: float) -> float:
        """
        Compute ranking score for a memory.

        Based on FLOW-002 ranking:
        - Semantic similarity (base weight)
        - Tier: emergency/long_term > short_term
        - Kind: invariant/preference > pattern > note/guard
        """
        score = similarity

        # Tier boost
        tier_boost = {
            MemoryTier.EMERGENCY: 0.3,
            MemoryTier.LONG_TERM: 0.2,
            MemoryTier.SHORT_TERM: 0.0,
        }
        score += tier_boost.get(memory.tier, 0.0)

        # Kind boost
        kind_boost = {
            MemoryKind.INVARIANT: 0.15,
            MemoryKind.PREFERENCE: 0.15,
            MemoryKind.PATTERN: 0.1,
            MemoryKind.GUARD: 0.05,
            MemoryKind.NOTE: 0.0,
        }
        score += kind_boost.get(memory.kind, 0.0)

        return score

    async def add_memory(self, memory: MemoryOp) -> str:
        """Add a memory to the store with its embedding."""
        embedding = await self.embed_text(memory.text)
        return self.store.add(memory, embedding)

    def add_memory_sync(self, memory: MemoryOp) -> str:
        """Synchronous add memory."""
        embedding = self.embed_text_sync(memory.text)
        return self.store.add(memory, embedding)

    async def retrieve(
        self,
        query: str,
        top_k: int = 10,
        min_similarity: float = 0.0,
    ) -> RetrievalResult:
        """
        Retrieve memories relevant to a query.

        Args:
            query: Search query text
            top_k: Maximum number of results
            min_similarity: Minimum cosine similarity threshold

        Returns:
            RetrievalResult with ranked memories and scores
        """
        if not self.store.memories:
            return RetrievalResult(memories=[], scores=[])

        query_embedding = await self.embed_text(query)

        # Compute similarities and rank
        scored = []
        for stored in self.store.memories:
            similarity = self._cosine_similarity(query_embedding, stored.embedding)
            if similarity >= min_similarity:
                rank_score = self._rank_score(stored.memory, similarity)
                scored.append((stored, similarity, rank_score))

        # Sort by rank score descending
        scored.sort(key=lambda x: x[2], reverse=True)

        # Take top_k
        top = scored[:top_k]

        return RetrievalResult(
            memories=[s[0] for s in top],
            scores=[s[1] for s in top],  # Return similarity scores
        )

    def retrieve_sync(
        self,
        query: str,
        top_k: int = 10,
        min_similarity: float = 0.0,
    ) -> RetrievalResult:
        """Synchronous retrieve."""
        import asyncio

        return asyncio.run(
            self.retrieve(query=query, top_k=top_k, min_similarity=min_similarity)
        )

    def format_context(self, result: RetrievalResult) -> str:
        """
        Format retrieval results as context for injection.

        Simple template-based formatting for v1.
        PROMPT-002 (LLM-based composition) is optional.
        """
        if not result.memories:
            return ""

        lines = ["## Relevant Memories\n"]

        # Group by kind
        by_kind: dict[MemoryKind, list[StoredMemory]] = {}
        for stored in result.memories:
            kind = stored.memory.kind
            if kind not in by_kind:
                by_kind[kind] = []
            by_kind[kind].append(stored)

        # Format each kind
        kind_order = [
            MemoryKind.GUARD,  # Warnings first
            MemoryKind.INVARIANT,
            MemoryKind.PREFERENCE,
            MemoryKind.PATTERN,
            MemoryKind.NOTE,
        ]

        for kind in kind_order:
            if kind not in by_kind:
                continue

            lines.append(f"### {kind.value.title()}s\n")
            for stored in by_kind[kind]:
                mem = stored.memory
                polarity_prefix = "⚠️ " if mem.polarity == -1 else ""
                lines.append(f"- {polarity_prefix}{mem.text}")
            lines.append("")

        return "\n".join(lines)
