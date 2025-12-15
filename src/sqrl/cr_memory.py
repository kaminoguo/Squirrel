"""
CR-Memory: Counterfactual Regret-based memory evaluation.

Background job that promotes/deprecates memories based on usage metrics.
Implements FLOW-004 from ARCHITECTURE.md, POLICY-001 to POLICY-006 from POLICY.md.
"""

from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from enum import Enum
from pathlib import Path
from typing import Optional

try:
    import tomllib
except ImportError:
    import tomli as tomllib


class EvalResult(Enum):
    """Result of CR-Memory evaluation."""

    NO_CHANGE = "no_change"
    PROMOTE = "promote"
    DEPRECATE = "deprecate"


@dataclass
class PromotionRule:
    """POLICY-001: When to promote memories."""

    min_opportunities: int = 5  # Require 5+ retrieval opportunities before evaluating
    min_use_ratio: float = 0.6  # Used 60%+ of opportunities = valuable
    min_regret_hits: int = 2  # At least 2 prevented errors/retries


@dataclass
class DeprecationRule:
    """POLICY-002: When to deprecate memories."""

    min_opportunities: int = 10  # Need more evidence before deprecating
    max_use_ratio: float = 0.1  # Used <10% of opportunities = not useful


@dataclass
class DecayRule:
    """POLICY-003: Time-based decay."""

    max_inactive_days: Optional[int] = None  # None = no time-based decay


@dataclass
class RegretWeights:
    """POLICY-004: Weights for regret calculation."""

    alpha_errors: float = 1.0  # Full weight for prevented errors
    beta_retries: float = 0.5  # Half weight for prevented retries (less severe)


@dataclass
class TTLDefaults:
    """POLICY-005: Default TTL values."""

    short_term_days: int = 30
    emergency_days: int = 7
    extend_on_promotion_days: int = 180
    remove_on_long_term: bool = True


@dataclass
class Policy:
    """Full policy configuration."""

    # Per-kind promotion rules
    promotion_default: PromotionRule = field(default_factory=PromotionRule)
    promotion_invariant: PromotionRule = field(
        default_factory=lambda: PromotionRule(
            min_opportunities=3,  # Invariants promoted faster (high-value)
            min_use_ratio=0.5,  # Lower threshold for core rules
            min_regret_hits=1,  # Single prevented error sufficient
        )
    )
    promotion_guard: PromotionRule = field(
        default_factory=lambda: PromotionRule(
            min_opportunities=10,  # Guards need more evidence (safety)
            min_use_ratio=0.3,  # Lower bar since guards are defensive
            min_regret_hits=3,  # Multiple prevented errors required
        )
    )

    # Per-kind deprecation rules
    deprecation_default: DeprecationRule = field(default_factory=DeprecationRule)
    deprecation_guard: DeprecationRule = field(
        default_factory=lambda: DeprecationRule(
            min_opportunities=20,  # Guards more tolerant (safety margin)
            max_use_ratio=0.05,  # Very low bar to keep safety guards
        )
    )
    deprecation_note: DeprecationRule = field(
        default_factory=lambda: DeprecationRule(
            min_opportunities=5,  # Notes deprecate faster (less critical)
            max_use_ratio=0.2,  # Higher threshold for low-value notes
        )
    )

    # Per-kind decay rules
    decay_guard: DecayRule = field(default_factory=lambda: DecayRule(max_inactive_days=90))
    decay_pattern: DecayRule = field(default_factory=lambda: DecayRule(max_inactive_days=180))
    decay_note: DecayRule = field(default_factory=lambda: DecayRule(max_inactive_days=60))
    decay_preference: DecayRule = field(default_factory=lambda: DecayRule(max_inactive_days=365))
    decay_invariant: DecayRule = field(default_factory=lambda: DecayRule(max_inactive_days=None))

    regret_weights: RegretWeights = field(default_factory=RegretWeights)
    ttl: TTLDefaults = field(default_factory=TTLDefaults)

    def get_promotion_rule(self, kind: str) -> PromotionRule:
        """Get promotion rule for memory kind."""
        rules = {
            "invariant": self.promotion_invariant,
            "guard": self.promotion_guard,
        }
        return rules.get(kind, self.promotion_default)

    def get_deprecation_rule(self, kind: str) -> DeprecationRule:
        """Get deprecation rule for memory kind."""
        rules = {
            "guard": self.deprecation_guard,
            "note": self.deprecation_note,
        }
        return rules.get(kind, self.deprecation_default)

    def get_decay_rule(self, kind: str) -> Optional[DecayRule]:
        """Get decay rule for memory kind."""
        rules = {
            "guard": self.decay_guard,
            "pattern": self.decay_pattern,
            "note": self.decay_note,
            "preference": self.decay_preference,
            "invariant": self.decay_invariant,
        }
        return rules.get(kind)


def load_policy(
    user_path: Optional[Path] = None,
    project_path: Optional[Path] = None,
) -> Policy:
    """
    Load policy from TOML files.

    Project-level overrides user-level, both override defaults.
    """
    policy = Policy()

    # User-level defaults
    if user_path is None:
        user_path = Path.home() / ".sqrl" / "memory_policy.toml"

    paths = [user_path]
    if project_path:
        paths.append(project_path)

    for path in paths:
        if not path.exists():
            continue

        with open(path, "rb") as f:
            data = tomllib.load(f)

        # Parse promotion rules
        if "promotion" in data:
            promo = data["promotion"]
            if "default" in promo:
                policy.promotion_default = _parse_promotion(promo["default"])
            if "invariant" in promo:
                policy.promotion_invariant = _parse_promotion(promo["invariant"])
            if "guard" in promo:
                policy.promotion_guard = _parse_promotion(promo["guard"])

        # Parse deprecation rules
        if "deprecation" in data:
            deprec = data["deprecation"]
            if "default" in deprec:
                policy.deprecation_default = _parse_deprecation(deprec["default"])
            if "guard" in deprec:
                policy.deprecation_guard = _parse_deprecation(deprec["guard"])
            if "note" in deprec:
                policy.deprecation_note = _parse_deprecation(deprec["note"])

        # Parse decay rules
        if "decay" in data:
            decay = data["decay"]
            if "guard" in decay:
                policy.decay_guard = _parse_decay(decay["guard"])
            if "pattern" in decay:
                policy.decay_pattern = _parse_decay(decay["pattern"])
            if "note" in decay:
                policy.decay_note = _parse_decay(decay["note"])
            if "preference" in decay:
                policy.decay_preference = _parse_decay(decay["preference"])
            if "invariant" in decay:
                policy.decay_invariant = _parse_decay(decay["invariant"])

        # Parse regret weights
        if "regret_weights" in data:
            rw = data["regret_weights"]
            policy.regret_weights = RegretWeights(
                alpha_errors=rw.get("alpha_errors", 1.0),
                beta_retries=rw.get("beta_retries", 0.5),
            )

        # Parse TTL
        if "ttl" in data:
            ttl = data["ttl"]
            if "default" in ttl:
                d = ttl["default"]
                policy.ttl.short_term_days = d.get("short_term_days", 30)
                policy.ttl.emergency_days = d.get("emergency_days", 7)
            if "on_promotion" in ttl:
                p = ttl["on_promotion"]
                policy.ttl.extend_on_promotion_days = p.get("extend_by_days", 180)
                policy.ttl.remove_on_long_term = p.get("remove_on_long_term", True)

    return policy


def _parse_promotion(d: dict) -> PromotionRule:
    """Parse promotion rule from dict."""
    return PromotionRule(
        min_opportunities=d.get("min_opportunities", 5),
        min_use_ratio=d.get("min_use_ratio", 0.6),
        min_regret_hits=d.get("min_regret_hits", 2),
    )


def _parse_deprecation(d: dict) -> DeprecationRule:
    """Parse deprecation rule from dict."""
    return DeprecationRule(
        min_opportunities=d.get("min_opportunities", 10),
        max_use_ratio=d.get("max_use_ratio", 0.1),
    )


def _parse_decay(d: dict) -> DecayRule:
    """Parse decay rule from dict."""
    max_days = d.get("max_inactive_days")
    return DecayRule(max_inactive_days=max_days)


@dataclass
class MemoryMetrics:
    """Memory metrics for evaluation (SCHEMA-003)."""

    memory_id: str
    use_count: int = 0
    opportunities: int = 0
    suspected_regret_hits: int = 0
    estimated_regret_saved: float = 0.0
    last_used_at: Optional[datetime] = None
    last_evaluated_at: Optional[datetime] = None


@dataclass
class Memory:
    """Minimal memory info for evaluation."""

    id: str
    kind: str  # preference, invariant, pattern, guard, note
    status: str  # provisional, active, deprecated
    tier: str  # short_term, long_term, emergency
    expires_at: Optional[datetime] = None


@dataclass
class EvalDecision:
    """Decision from CR-Memory evaluation."""

    memory_id: str
    result: EvalResult
    new_status: Optional[str] = None  # If changed
    new_tier: Optional[str] = None  # If promoted to long_term
    new_expires_at: Optional[datetime] = None  # Extended/removed TTL
    reason: str = ""


def days_since(dt: Optional[datetime], now: Optional[datetime] = None) -> Optional[int]:
    """Calculate days since a datetime."""
    if dt is None:
        return None
    if now is None:
        now = datetime.now(timezone.utc)
    return (now - dt).days


class CRMemoryEvaluator:
    """
    Evaluates memories for promotion/deprecation.

    Implements the algorithm from POLICY.md.
    """

    def __init__(self, policy: Optional[Policy] = None):
        self.policy = policy or Policy()

    def evaluate(
        self,
        memory: Memory,
        metrics: MemoryMetrics,
        now: Optional[datetime] = None,
    ) -> EvalDecision:
        """
        Evaluate a single memory for promotion/deprecation.

        Returns decision with new status/tier/expires_at if changed.
        """
        if now is None:
            now = datetime.now(timezone.utc)

        # Skip already deprecated
        if memory.status == "deprecated":
            return EvalDecision(
                memory_id=memory.id,
                result=EvalResult.NO_CHANGE,
                reason="Already deprecated",
            )

        opp = metrics.opportunities
        uses = metrics.use_count
        hits = metrics.suspected_regret_hits

        promo = self.policy.get_promotion_rule(memory.kind)
        deprec = self.policy.get_deprecation_rule(memory.kind)
        decay = self.policy.get_decay_rule(memory.kind)

        # Not enough evidence yet - only check time-based decay
        if opp < promo.min_opportunities:
            if decay and decay.max_inactive_days:
                inactive_days = days_since(metrics.last_used_at, now)
                if inactive_days is not None and inactive_days > decay.max_inactive_days:
                    max_days = decay.max_inactive_days
                    return EvalDecision(
                        memory_id=memory.id,
                        result=EvalResult.DEPRECATE,
                        new_status="deprecated",
                        reason=f"Inactive for {inactive_days} days (max: {max_days})",
                    )
            return EvalDecision(
                memory_id=memory.id,
                result=EvalResult.NO_CHANGE,
                reason=f"Not enough opportunities ({opp} < {promo.min_opportunities})",
            )

        use_ratio = uses / opp if opp > 0 else 0.0

        # Promotion check
        if use_ratio >= promo.min_use_ratio and hits >= promo.min_regret_hits:
            new_tier = memory.tier
            new_expires: Optional[datetime] = memory.expires_at

            # Consider promoting to long_term (80%+ usage = very valuable)
            if memory.status == "provisional" and use_ratio >= 0.8:
                new_tier = "long_term"
                if self.policy.ttl.remove_on_long_term:
                    new_expires = None  # Remove TTL for long_term
            elif memory.expires_at:
                # Extend TTL
                new_expires = now + timedelta(days=self.policy.ttl.extend_on_promotion_days)

            reason = (
                f"Promoted: use_ratio={use_ratio:.2f} >= {promo.min_use_ratio}, "
                f"hits={hits} >= {promo.min_regret_hits}"
            )
            return EvalDecision(
                memory_id=memory.id,
                result=EvalResult.PROMOTE,
                new_status="active",
                new_tier=new_tier,
                new_expires_at=new_expires,
                reason=reason,
            )

        # Deprecation check
        if opp >= deprec.min_opportunities and use_ratio <= deprec.max_use_ratio:
            reason = (
                f"Deprecated: use_ratio={use_ratio:.2f} <= {deprec.max_use_ratio}, "
                f"opp={opp} >= {deprec.min_opportunities}"
            )
            return EvalDecision(
                memory_id=memory.id,
                result=EvalResult.DEPRECATE,
                new_status="deprecated",
                reason=reason,
            )

        # Time-based decay
        if decay and decay.max_inactive_days:
            inactive_days = days_since(metrics.last_used_at, now)
            if inactive_days is not None and inactive_days > decay.max_inactive_days:
                return EvalDecision(
                    memory_id=memory.id,
                    result=EvalResult.DEPRECATE,
                    new_status="deprecated",
                    reason=f"Inactive for {inactive_days} days (max: {decay.max_inactive_days})",
                )

        return EvalDecision(
            memory_id=memory.id,
            result=EvalResult.NO_CHANGE,
            reason=f"No change: use_ratio={use_ratio:.2f}, opp={opp}, hits={hits}",
        )

    def evaluate_batch(
        self,
        memories: list[tuple[Memory, MemoryMetrics]],
        now: Optional[datetime] = None,
    ) -> list[EvalDecision]:
        """
        Evaluate a batch of memories.

        Returns list of decisions (only changed ones filtered if needed).
        """
        if now is None:
            now = datetime.now(timezone.utc)

        return [self.evaluate(m, metrics, now) for m, metrics in memories]

    def update_regret(
        self,
        metrics: MemoryMetrics,
        delta_errors: int = 0,
        delta_retries: int = 0,
    ) -> float:
        """
        Calculate regret saved for this evaluation cycle.

        Uses POLICY-004 weights.
        """
        w = self.policy.regret_weights
        err_regret = w.alpha_errors * max(0, delta_errors)
        retry_regret = w.beta_retries * max(0, delta_retries)
        return err_regret + retry_regret


__all__ = [
    "EvalResult",
    "PromotionRule",
    "DeprecationRule",
    "DecayRule",
    "RegretWeights",
    "TTLDefaults",
    "Policy",
    "load_policy",
    "MemoryMetrics",
    "Memory",
    "EvalDecision",
    "CRMemoryEvaluator",
]
