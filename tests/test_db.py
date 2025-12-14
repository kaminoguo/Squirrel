"""Tests for database schema and repository operations."""

import tempfile
from pathlib import Path

import pytest

from sqrl.db import (
    commit_memory_ops,
    deprecate_memory,
    get_active_memories,
    get_memory_by_id,
    get_metrics,
    get_unprocessed_episodes,
    increment_use_count,
    init_db,
    insert_episode,
    insert_memory,
    mark_episode_processed,
)
from sqrl.memory_writer.models import (
    Evidence,
    EvidenceSource,
    Frustration,
    MemoryKind,
    MemoryOp,
    MemoryTier,
    OpType,
    OwnerType,
    Scope,
)


@pytest.fixture
def db_conn():
    """Create a temporary database for testing."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        conn = init_db(db_path)
        yield conn
        conn.close()


class TestSchema:
    """Test database schema initialization."""

    def test_init_creates_tables(self, db_conn):
        """Verify all tables are created."""
        cursor = db_conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
        )
        tables = [row[0] for row in cursor.fetchall()]

        assert "memories" in tables
        assert "evidence" in tables
        assert "memory_metrics" in tables
        assert "episodes" in tables
        assert "schema_version" in tables

    def test_init_is_idempotent(self, db_conn):
        """Re-initializing should not fail."""
        # Get the path from connection
        cursor = db_conn.execute("PRAGMA database_list")
        row = cursor.fetchone()
        db_path = Path(row[2])

        # Re-init should work
        conn2 = init_db(db_path)
        assert conn2 is not None
        conn2.close()


class TestEpisodes:
    """Test episode operations."""

    def test_insert_episode(self, db_conn):
        """Insert and retrieve an episode."""
        episode_id = insert_episode(
            db_conn,
            project_id="test-project",
            start_ts="2024-01-01T10:00:00Z",
            end_ts="2024-01-01T10:30:00Z",
            events_json='{"events": []}',
        )

        assert episode_id is not None

        cursor = db_conn.execute("SELECT * FROM episodes WHERE id = ?", (episode_id,))
        row = cursor.fetchone()

        assert row["project_id"] == "test-project"
        assert row["processed"] == 0

    def test_mark_processed(self, db_conn):
        """Mark episode as processed."""
        episode_id = insert_episode(
            db_conn,
            project_id="test-project",
            start_ts="2024-01-01T10:00:00Z",
            end_ts="2024-01-01T10:30:00Z",
            events_json='{"events": []}',
        )

        mark_episode_processed(db_conn, episode_id)

        cursor = db_conn.execute(
            "SELECT processed FROM episodes WHERE id = ?", (episode_id,)
        )
        assert cursor.fetchone()["processed"] == 1

    def test_get_unprocessed(self, db_conn):
        """Get unprocessed episodes."""
        # Insert two episodes
        ep1 = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )
        ep2 = insert_episode(
            db_conn, "proj", "2024-01-01T11:00:00Z", "2024-01-01T11:30:00Z", "{}"
        )

        # Mark one as processed
        mark_episode_processed(db_conn, ep1)

        # Should only get ep2
        unprocessed = get_unprocessed_episodes(db_conn, "proj")
        assert len(unprocessed) == 1
        assert unprocessed[0]["id"] == ep2


class TestMemories:
    """Test memory operations."""

    def _make_op(self, text: str = "Test memory") -> MemoryOp:
        """Create a test MemoryOp."""
        return MemoryOp(
            op=OpType.ADD,
            episode_idx=0,
            scope=Scope.PROJECT,
            owner_type=OwnerType.USER,
            owner_id="alice",
            kind=MemoryKind.PATTERN,
            tier=MemoryTier.SHORT_TERM,
            polarity=1,
            text=text,
            confidence=0.85,
            evidence=Evidence(
                source=EvidenceSource.FAILURE_THEN_SUCCESS,
                frustration=Frustration.MILD,
            ),
        )

    def test_insert_memory(self, db_conn):
        """Insert a memory from MemoryOp."""
        # First insert an episode
        episode_id = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )

        op = self._make_op("SSL errors require httpx")
        memory_id = insert_memory(db_conn, op, episode_id)

        # Verify memory
        memory = get_memory_by_id(db_conn, memory_id)
        assert memory is not None
        assert memory["text"] == "SSL errors require httpx"
        assert memory["kind"] == "pattern"
        assert memory["status"] == "provisional"
        assert memory["confidence"] == 0.85

        # Verify metrics initialized
        metrics = get_metrics(db_conn, memory_id)
        assert metrics is not None
        assert metrics["use_count"] == 0
        assert metrics["opportunities"] == 0

        # Verify evidence created
        cursor = db_conn.execute(
            "SELECT * FROM evidence WHERE memory_id = ?", (memory_id,)
        )
        evidence = cursor.fetchone()
        assert evidence is not None
        assert evidence["source"] == "failure_then_success"
        assert evidence["frustration"] == "mild"

    def test_deprecate_memory(self, db_conn):
        """Deprecate a memory."""
        episode_id = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )
        op = self._make_op()
        memory_id = insert_memory(db_conn, op, episode_id)

        deprecate_memory(db_conn, memory_id)

        memory = get_memory_by_id(db_conn, memory_id)
        assert memory["status"] == "deprecated"

    def test_get_active_memories(self, db_conn):
        """Filter memories by status."""
        episode_id = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )

        # Insert two memories
        op1 = self._make_op("Memory 1")
        op2 = self._make_op("Memory 2")
        m1 = insert_memory(db_conn, op1, episode_id)
        m2 = insert_memory(db_conn, op2, episode_id)

        # Deprecate one
        deprecate_memory(db_conn, m1)

        # Should only get m2
        active = get_active_memories(db_conn)
        assert len(active) == 1
        assert active[0]["id"] == m2

    def test_increment_use_count(self, db_conn):
        """Track memory usage."""
        episode_id = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )
        op = self._make_op()
        memory_id = insert_memory(db_conn, op, episode_id)

        increment_use_count(db_conn, memory_id)
        increment_use_count(db_conn, memory_id)

        metrics = get_metrics(db_conn, memory_id)
        assert metrics["use_count"] == 2
        assert metrics["last_used_at"] is not None


class TestCommitLayer:
    """Test the commit layer that processes Memory Writer output."""

    def test_commit_add_ops(self, db_conn):
        """Commit ADD operations."""
        episode_id = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )

        ops = [
            MemoryOp(
                op=OpType.ADD,
                episode_idx=0,
                scope=Scope.PROJECT,
                owner_type=OwnerType.USER,
                owner_id="alice",
                kind=MemoryKind.PATTERN,
                tier=MemoryTier.SHORT_TERM,
                text="Pattern 1",
                confidence=0.8,
                evidence=Evidence(
                    source=EvidenceSource.FAILURE_THEN_SUCCESS,
                    frustration=Frustration.NONE,
                ),
            ),
            MemoryOp(
                op=OpType.ADD,
                episode_idx=0,
                scope=Scope.GLOBAL,
                owner_type=OwnerType.USER,
                owner_id="alice",
                kind=MemoryKind.PREFERENCE,
                tier=MemoryTier.SHORT_TERM,
                key="user.pref.style",
                text="Prefers async/await",
                confidence=0.9,
                evidence=Evidence(
                    source=EvidenceSource.USER_CORRECTION,
                    frustration=Frustration.MILD,
                ),
            ),
        ]

        new_ids = commit_memory_ops(db_conn, ops, episode_id)
        assert len(new_ids) == 2

        # Verify both memories exist
        for mid in new_ids:
            assert get_memory_by_id(db_conn, mid) is not None

    def test_commit_update_op(self, db_conn):
        """UPDATE should deprecate old and create new."""
        episode_id = insert_episode(
            db_conn, "proj", "2024-01-01T10:00:00Z", "2024-01-01T10:30:00Z", "{}"
        )

        # Create initial memory
        initial_op = MemoryOp(
            op=OpType.ADD,
            episode_idx=0,
            scope=Scope.PROJECT,
            owner_type=OwnerType.USER,
            owner_id="alice",
            kind=MemoryKind.INVARIANT,
            tier=MemoryTier.SHORT_TERM,
            key="project.http.client",
            text="Use requests library",
            confidence=0.7,
            evidence=Evidence(
                source=EvidenceSource.EXPLICIT_STATEMENT,
                frustration=Frustration.NONE,
            ),
        )
        old_ids = commit_memory_ops(db_conn, [initial_op], episode_id)
        old_id = old_ids[0]

        # Now update it
        update_op = MemoryOp(
            op=OpType.UPDATE,
            target_memory_id=old_id,
            episode_idx=0,
            scope=Scope.PROJECT,
            owner_type=OwnerType.USER,
            owner_id="alice",
            kind=MemoryKind.INVARIANT,
            tier=MemoryTier.LONG_TERM,
            key="project.http.client",
            text="Use httpx library (requests has SSL issues)",
            confidence=0.9,
            evidence=Evidence(
                source=EvidenceSource.FAILURE_THEN_SUCCESS,
                frustration=Frustration.MODERATE,
            ),
        )
        new_ids = commit_memory_ops(db_conn, [update_op], episode_id)

        # Old should be deprecated
        old_mem = get_memory_by_id(db_conn, old_id)
        assert old_mem["status"] == "deprecated"

        # New should exist
        new_mem = get_memory_by_id(db_conn, new_ids[0])
        assert new_mem["text"] == "Use httpx library (requests has SSL issues)"
        assert new_mem["tier"] == "long_term"
