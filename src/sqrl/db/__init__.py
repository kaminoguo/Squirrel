"""Database module for Squirrel."""

from sqrl.db.repository import (
    commit_memory_ops,
    deprecate_memory,
    get_active_memories,
    get_memories_by_key,
    get_memory_by_id,
    get_metrics,
    get_unprocessed_episodes,
    increment_opportunities,
    increment_use_count,
    insert_episode,
    insert_memory,
    mark_episode_processed,
    search_memories_by_text,
)
from sqrl.db.schema import get_connection, get_db_path, init_db

__all__ = [
    # Schema
    "init_db",
    "get_connection",
    "get_db_path",
    # Episodes
    "insert_episode",
    "mark_episode_processed",
    "get_unprocessed_episodes",
    # Memories
    "insert_memory",
    "deprecate_memory",
    "get_memory_by_id",
    "get_memories_by_key",
    "get_active_memories",
    "search_memories_by_text",
    # Metrics
    "increment_use_count",
    "increment_opportunities",
    "get_metrics",
    # Commit layer
    "commit_memory_ops",
]
