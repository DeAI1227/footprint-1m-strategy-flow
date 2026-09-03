"""Sentence layer package. Stage 0: read params, boot shadow, refuse live.

Do not parse venue WebSocket or assemble footprint matrices here.
The production footprint matrix lives in Rust (not wired yet).
"""

from .boot import LiveDenied, boot_once, live_allowed
from .config import AppConfig, load_config

__all__ = [
    "SCHOOL",
    "MODES",
    "VERSION",
    "AppConfig",
    "LiveDenied",
    "boot_once",
    "live_allowed",
    "load_config",
]

SCHOOL = "footprint"
MODES = ("shadow", "sim", "live_small", "live")
VERSION = "0.0.0"
