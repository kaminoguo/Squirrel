"""Tests for CR-Memory module."""

import tempfile
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

from sqrl.cr_memory import (
    CRMemoryEvaluator,
    EvalResult,
    Memory,
    MemoryMetrics,
    Policy,
    load_policy,
)


class TestPolicy:
    """Test policy configuration."""

    def test_default_policy(self):
        """Default policy has sensible values."""
        policy = Policy()

        # Check promotion defaults
        assert policy.promotion_default.min_opportunities == 5
        assert policy.promotion_default.min_use_ratio == 0.6
        assert policy.promotion_default.min_regret_hits == 2

        # Invariants promoted faster
        assert policy.promotion_invariant.min_opportunities == 3

        # Guards need more evidence
        assert policy.promotion_guard.min_opportunities == 10

    def test_get_promotion_rule(self):
        """Get promotion rule by kind."""
        policy = Policy()

        assert policy.get_promotion_rule("invariant") == policy.promotion_invariant
        assert policy.get_promotion_rule("guard") == policy.promotion_guard
        assert policy.get_promotion_rule("note") == policy.promotion_default
        assert policy.get_promotion_rule("unknown") == policy.promotion_default

    def test_get_deprecation_rule(self):
        """Get deprecation rule by kind."""
        policy = Policy()

        assert policy.get_deprecation_rule("guard") == policy.deprecation_guard
        assert policy.get_deprecation_rule("note") == policy.deprecation_note
        assert policy.get_deprecation_rule("pattern") == policy.deprecation_default

    def test_get_decay_rule(self):
        """Get decay rule by kind."""
        policy = Policy()

        assert policy.get_decay_rule("guard").max_inactive_days == 90
        assert policy.get_decay_rule("invariant").max_inactive_days is None
        assert policy.get_decay_rule("note").max_inactive_days == 60


class TestLoadPolicy:
    """Test policy loading from TOML."""

    def test_load_nonexistent_file(self):
        """Load from nonexistent file returns defaults."""
        policy = load_policy(Path("/nonexistent/path"))
        assert policy.promotion_default.min_opportunities == 5

    def test_load_from_toml(self):
        """Load policy from TOML file."""
        toml_content = """
[promotion.default]
min_opportunities = 10
min_use_ratio = 0.8
min_regret_hits = 5

[deprecation.default]
min_opportunities = 20
max_use_ratio = 0.05

[decay.guard]
max_inactive_days = 60
"""
        with tempfile.TemporaryDirectory() as tmpdir:
            policy_path = Path(tmpdir) / "memory_policy.toml"
            policy_path.write_text(toml_content)

            policy = load_policy(policy_path)

            assert policy.promotion_default.min_opportunities == 10
            assert policy.promotion_default.min_use_ratio == 0.8
            assert policy.deprecation_default.min_opportunities == 20
            assert policy.decay_guard.max_inactive_days == 60

    def test_project_overrides_user(self):
        """Project policy overrides user policy."""
        user_toml = """
[promotion.default]
min_opportunities = 5
"""
        project_toml = """
[promotion.default]
min_opportunities = 3
"""
        with tempfile.TemporaryDirectory() as tmpdir:
            user_path = Path(tmpdir) / "user_policy.toml"
            project_path = Path(tmpdir) / "project_policy.toml"
            user_path.write_text(user_toml)
            project_path.write_text(project_toml)

            policy = load_policy(user_path, project_path)

            # Project overrides user
            assert policy.promotion_default.min_opportunities == 3


class TestCRMemoryEvaluator:
    """Test CR-Memory evaluation logic."""

    def test_skip_deprecated(self):
        """Skip already deprecated memories."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="note",
            status="deprecated",
            tier="short_term",
        )
        metrics = MemoryMetrics(memory_id="mem-1")

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.NO_CHANGE
        assert "deprecated" in decision.reason.lower()

    def test_not_enough_opportunities(self):
        """Skip when not enough opportunities."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="note",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=2,  # < 5 (default min)
        )

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.NO_CHANGE
        assert "opportunities" in decision.reason.lower()

    def test_promote_memory(self):
        """Promote memory with good metrics."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="pattern",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=10,
            use_count=8,  # 80% use ratio
            suspected_regret_hits=3,
        )

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.PROMOTE
        assert decision.new_status == "active"
        assert decision.new_tier == "long_term"  # High use ratio

    def test_promote_without_long_term(self):
        """Promote to active but not long_term."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="pattern",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=10,
            use_count=6,  # 60% use ratio - just at threshold
            suspected_regret_hits=2,
        )

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.PROMOTE
        assert decision.new_status == "active"
        assert decision.new_tier == "short_term"  # Not high enough for long_term

    def test_deprecate_low_usage(self):
        """Deprecate memory with low usage."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="note",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=10,  # >= 10 for notes
            use_count=0,  # 0% use ratio
            suspected_regret_hits=0,
        )

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.DEPRECATE
        assert decision.new_status == "deprecated"

    def test_deprecate_time_decay(self):
        """Deprecate inactive memory by time decay."""
        evaluator = CRMemoryEvaluator()
        now = datetime.now(timezone.utc)
        memory = Memory(
            id="mem-1",
            kind="note",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=3,  # Not enough for full evaluation
            use_count=2,
            last_used_at=now - timedelta(days=100),  # > 60 days for notes
        )

        decision = evaluator.evaluate(memory, metrics, now)

        assert decision.result == EvalResult.DEPRECATE
        assert "inactive" in decision.reason.lower()

    def test_no_decay_for_invariants(self):
        """Invariants don't decay by time."""
        evaluator = CRMemoryEvaluator()
        now = datetime.now(timezone.utc)
        memory = Memory(
            id="mem-1",
            kind="invariant",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=2,  # Not enough for full evaluation
            use_count=1,
            last_used_at=now - timedelta(days=365),  # Very old
        )

        decision = evaluator.evaluate(memory, metrics, now)

        assert decision.result == EvalResult.NO_CHANGE

    def test_extend_ttl_on_promotion(self):
        """Extend TTL when promoting."""
        evaluator = CRMemoryEvaluator()
        now = datetime.now(timezone.utc)
        old_expires = now + timedelta(days=10)
        memory = Memory(
            id="mem-1",
            kind="pattern",
            status="active",  # Already active
            tier="short_term",
            expires_at=old_expires,
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=10,
            use_count=7,  # 70% - promote
            suspected_regret_hits=3,
        )

        decision = evaluator.evaluate(memory, metrics, now)

        assert decision.result == EvalResult.PROMOTE
        assert decision.new_expires_at is not None
        assert decision.new_expires_at > old_expires

    def test_evaluate_batch(self):
        """Evaluate batch of memories."""
        evaluator = CRMemoryEvaluator()
        memories = [
            (
                Memory(id="mem-1", kind="note", status="provisional", tier="short_term"),
                MemoryMetrics(
                    memory_id="mem-1", opportunities=10, use_count=8, suspected_regret_hits=3
                ),
            ),
            (
                Memory(id="mem-2", kind="note", status="deprecated", tier="short_term"),
                MemoryMetrics(memory_id="mem-2"),
            ),
            (
                Memory(id="mem-3", kind="note", status="provisional", tier="short_term"),
                MemoryMetrics(memory_id="mem-3", opportunities=10, use_count=0),
            ),
        ]

        decisions = evaluator.evaluate_batch(memories)

        assert len(decisions) == 3
        assert decisions[0].result == EvalResult.PROMOTE
        assert decisions[1].result == EvalResult.NO_CHANGE
        assert decisions[2].result == EvalResult.DEPRECATE

    def test_update_regret(self):
        """Calculate regret saved."""
        evaluator = CRMemoryEvaluator()
        metrics = MemoryMetrics(memory_id="mem-1")

        regret = evaluator.update_regret(metrics, delta_errors=2, delta_retries=4)

        # alpha * 2 + beta * 4 = 1.0 * 2 + 0.5 * 4 = 4.0
        assert regret == 4.0

    def test_guard_needs_more_evidence(self):
        """Guards need more opportunities to promote."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="guard",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=5,  # < 10 required for guards
            use_count=4,
            suspected_regret_hits=3,
        )

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.NO_CHANGE
        assert "opportunities" in decision.reason.lower()

    def test_invariant_promotes_faster(self):
        """Invariants need fewer opportunities."""
        evaluator = CRMemoryEvaluator()
        memory = Memory(
            id="mem-1",
            kind="invariant",
            status="provisional",
            tier="short_term",
        )
        metrics = MemoryMetrics(
            memory_id="mem-1",
            opportunities=3,  # Exactly at threshold
            use_count=2,  # 67% > 50%
            suspected_regret_hits=1,  # Exactly at threshold
        )

        decision = evaluator.evaluate(memory, metrics)

        assert decision.result == EvalResult.PROMOTE


class TestEvaluateMemoriesHandler:
    """Test IPC handler for CR-Memory."""

    @pytest.mark.asyncio
    async def test_evaluate_memories_empty(self):
        """Empty memories list raises error."""
        from sqrl.ipc.handlers import EvaluateMemoriesHandler, IPCError

        handler = EvaluateMemoriesHandler()

        with pytest.raises(IPCError) as exc_info:
            await handler({"memories": []})

        assert exc_info.value.code == -32020

    @pytest.mark.asyncio
    async def test_evaluate_memories_success(self):
        """Evaluate memories successfully."""
        from sqrl.ipc.handlers import EvaluateMemoriesHandler

        handler = EvaluateMemoriesHandler()

        result = await handler({
            "memories": [
                {
                    "id": "mem-1",
                    "kind": "pattern",
                    "status": "provisional",
                    "tier": "short_term",
                    "metrics": {
                        "opportunities": 10,
                        "use_count": 8,
                        "suspected_regret_hits": 3,
                    },
                },
                {
                    "id": "mem-2",
                    "kind": "note",
                    "status": "provisional",
                    "tier": "short_term",
                    "metrics": {
                        "opportunities": 10,
                        "use_count": 0,
                    },
                },
            ],
        })

        assert "decisions" in result
        assert len(result["decisions"]) == 2

        # mem-1 promoted
        d1 = next(d for d in result["decisions"] if d["memory_id"] == "mem-1")
        assert d1["result"] == "promote"
        assert d1["new_status"] == "active"

        # mem-2 deprecated
        d2 = next(d for d in result["decisions"] if d["memory_id"] == "mem-2")
        assert d2["result"] == "deprecate"
        assert d2["new_status"] == "deprecated"

    @pytest.mark.asyncio
    async def test_evaluate_with_timestamp(self):
        """Evaluate with explicit timestamp."""
        from sqrl.ipc.handlers import EvaluateMemoriesHandler

        handler = EvaluateMemoriesHandler()

        result = await handler({
            "now": "2025-01-01T00:00:00Z",
            "memories": [
                {
                    "id": "mem-1",
                    "kind": "note",
                    "status": "provisional",
                    "tier": "short_term",
                    "metrics": {
                        "opportunities": 3,
                        "use_count": 2,
                        "last_used_at": "2024-01-01T00:00:00Z",  # > 60 days
                    },
                },
            ],
        })

        # Should be deprecated due to time decay
        assert len(result["decisions"]) == 1
        assert result["decisions"][0]["result"] == "deprecate"

    @pytest.mark.asyncio
    async def test_no_change_not_in_results(self):
        """No-change decisions are included with all memories."""
        from sqrl.ipc.handlers import EvaluateMemoriesHandler

        handler = EvaluateMemoriesHandler()

        result = await handler({
            "memories": [
                {
                    "id": "mem-1",
                    "kind": "note",
                    "status": "deprecated",  # Already deprecated
                    "tier": "short_term",
                    "metrics": {},
                },
            ],
        })

        # No changes, so empty decisions list
        assert result["decisions"] == []
