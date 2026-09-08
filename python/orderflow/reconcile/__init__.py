# REST reconcile orchestration. Local order book stays in Rust.
# Stage 6 compares fixtures (no API keys). Exchange is truth.

from .engine import reconcile

WIRED = True

__all__ = ["WIRED", "reconcile"]
