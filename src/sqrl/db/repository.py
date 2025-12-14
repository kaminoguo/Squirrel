"""
Repository functions for Squirrel database operations.

Provides CRUD operations for memories, evidence, episodes, and metrics.
"""

import sqlite3
import uuid
from datetime import datetime, timezone
from typing import Optional

from sqrl.memory_writer.models import MemoryOp, OpType


def now_iso() -> str:
    """Get current timestamp in ISO 8601 format."""
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def generate_id() -> str:
    """Generate a new UUID."""
    return str(uuid.uuid4())


# --- Episode Operations ---


def insert_episode(
    conn: sqlite3.Connection,
    project_id: str,
    start_ts: str,
    end_ts: str,
    events_json: str,
) -> str:
    """Insert a new episode. Returns episode_id."""
    episode_id = generate_id()
    conn.execute(
        """
        INSERT INTO episodes (id, project_id, start_ts, end_ts, events_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        (episode_id, project_id, start_ts, end_ts, events_json, now_iso()),
    )
    conn.commit()
    return episode_id


def mark_episode_processed(conn: sqlite3.Connection, episode_id: str) -> None:
    """Mark an episode as processed by Memory Writer."""
    conn.execute(
        "UPDATE episodes SET processed = 1 WHERE id = ?",
        (episode_id,),
    )
    conn.commit()


def get_unprocessed_episodes(
    conn: sqlite3.Connection, project_id: str, limit: int = 10
) -> list[sqlite3.Row]:
    """Get unprocessed episodes for a project."""
    cursor = conn.execute(
        """
        SELECT * FROM episodes
        WHERE project_id = ? AND processed = 0
        ORDER BY start_ts ASC
        LIMIT ?
        """,
        (project_id, limit),
    )
    return cursor.fetchall()


# --- Memory Operations ---


def insert_memory(
    conn: sqlite3.Connection,
    op: MemoryOp,
    episode_id: str,
    embedding: Optional[bytes] = None,
) -> str:
    """
    Insert a new memory from Memory Writer output.
    Returns memory_id.
    """
    memory_id = generate_id()
    now = now_iso()

    # Calculate expires_at if ttl_days is set
    expires_at = None
    if op.ttl_days:
        from datetime import timedelta
        exp = datetime.now(timezone.utc) + timedelta(days=op.ttl_days)
        expires_at = exp.isoformat().replace("+00:00", "Z")

    conn.execute(
        """
        INSERT INTO memories (
            id, project_id, scope, owner_type, owner_id,
            kind, tier, polarity, key, text,
            status, confidence, expires_at, embedding,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            memory_id,
            None,  # project_id set later based on scope
            op.scope.value,
            op.owner_type.value,
            op.owner_id,
            op.kind.value,
            op.tier.value,
            op.polarity,
            op.key,
            op.text,
            "provisional",
            op.confidence,
            expires_at,
            embedding,
            now,
            now,
        ),
    )

    # Initialize memory_metrics
    conn.execute(
        "INSERT INTO memory_metrics (memory_id) VALUES (?)",
        (memory_id,),
    )

    # Insert evidence linking memory to episode
    conn.execute(
        """
        INSERT INTO evidence (id, memory_id, episode_id, source, frustration, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        (
            generate_id(),
            memory_id,
            episode_id,
            op.evidence.source.value,
            op.evidence.frustration.value,
            now,
        ),
    )

    conn.commit()
    return memory_id


def deprecate_memory(conn: sqlite3.Connection, memory_id: str) -> None:
    """Mark a memory as deprecated."""
    conn.execute(
        "UPDATE memories SET status = 'deprecated', updated_at = ? WHERE id = ?",
        (now_iso(), memory_id),
    )
    conn.commit()


def get_memory_by_id(conn: sqlite3.Connection, memory_id: str) -> Optional[sqlite3.Row]:
    """Get a memory by ID."""
    cursor = conn.execute("SELECT * FROM memories WHERE id = ?", (memory_id,))
    return cursor.fetchone()


def get_memories_by_key(
    conn: sqlite3.Connection,
    key: str,
    status: str = "active",
) -> list[sqlite3.Row]:
    """Get memories by declarative key."""
    cursor = conn.execute(
        "SELECT * FROM memories WHERE key = ? AND status = ?",
        (key, status),
    )
    return cursor.fetchall()


def get_active_memories(
    conn: sqlite3.Connection,
    project_id: Optional[str] = None,
    owner_type: Optional[str] = None,
    owner_id: Optional[str] = None,
    kind: Optional[str] = None,
    limit: int = 100,
) -> list[sqlite3.Row]:
    """Get active/provisional memories with optional filters."""
    query = "SELECT * FROM memories WHERE status IN ('provisional', 'active')"
    params: list = []

    if project_id:
        query += " AND (project_id = ? OR scope = 'global')"
        params.append(project_id)
    if owner_type:
        query += " AND owner_type = ?"
        params.append(owner_type)
    if owner_id:
        query += " AND owner_id = ?"
        params.append(owner_id)
    if kind:
        query += " AND kind = ?"
        params.append(kind)

    query += " ORDER BY tier DESC, created_at DESC LIMIT ?"
    params.append(limit)

    cursor = conn.execute(query, params)
    return cursor.fetchall()


def search_memories_by_text(
    conn: sqlite3.Connection,
    query: str,
    limit: int = 10,
) -> list[sqlite3.Row]:
    """Simple text search (FTS would be better, but this works for v1)."""
    cursor = conn.execute(
        """
        SELECT * FROM memories
        WHERE status IN ('provisional', 'active')
        AND text LIKE ?
        ORDER BY tier DESC, created_at DESC
        LIMIT ?
        """,
        (f"%{query}%", limit),
    )
    return cursor.fetchall()


# --- Metrics Operations ---


def increment_use_count(conn: sqlite3.Connection, memory_id: str) -> None:
    """Increment use_count when memory is used in context."""
    now = now_iso()
    conn.execute(
        """
        UPDATE memory_metrics
        SET use_count = use_count + 1, last_used_at = ?
        WHERE memory_id = ?
        """,
        (now, memory_id),
    )
    conn.commit()


def increment_opportunities(conn: sqlite3.Connection, memory_ids: list[str]) -> None:
    """Increment opportunities for memories that could have been used."""
    if not memory_ids:
        return
    placeholders = ",".join("?" * len(memory_ids))
    conn.execute(
        f"""
        UPDATE memory_metrics
        SET opportunities = opportunities + 1
        WHERE memory_id IN ({placeholders})
        """,
        memory_ids,
    )
    conn.commit()


def get_metrics(conn: sqlite3.Connection, memory_id: str) -> Optional[sqlite3.Row]:
    """Get metrics for a memory."""
    cursor = conn.execute(
        "SELECT * FROM memory_metrics WHERE memory_id = ?",
        (memory_id,),
    )
    return cursor.fetchone()


# --- Commit Layer ---


def commit_memory_ops(
    conn: sqlite3.Connection,
    ops: list[MemoryOp],
    episode_id: str,
    get_embedding: Optional[callable] = None,
) -> list[str]:
    """
    Commit Memory Writer operations to database.
    Returns list of new memory IDs.

    Args:
        conn: Database connection
        ops: List of MemoryOp from Memory Writer
        episode_id: Episode these memories came from
        get_embedding: Optional function to generate embeddings (text -> bytes)
    """
    new_ids = []

    for op in ops:
        if op.op == OpType.ADD:
            # Generate embedding if function provided
            embedding = None
            if get_embedding:
                embedding = get_embedding(op.text)

            memory_id = insert_memory(conn, op, episode_id, embedding)
            new_ids.append(memory_id)

        elif op.op == OpType.UPDATE:
            if op.target_memory_id:
                deprecate_memory(conn, op.target_memory_id)

            embedding = None
            if get_embedding:
                embedding = get_embedding(op.text)

            memory_id = insert_memory(conn, op, episode_id, embedding)
            new_ids.append(memory_id)

        elif op.op == OpType.DEPRECATE:
            if op.target_memory_id:
                deprecate_memory(conn, op.target_memory_id)

    return new_ids
