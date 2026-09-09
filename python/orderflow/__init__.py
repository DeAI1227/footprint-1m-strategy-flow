"""Sentence layer package. Read params, boot shadow, refuse live.

Stage 5: A–G machines read frozen Rust 1m snapshots.
Stage 6: fixture reconcile (no API keys). Do not parse venue WebSocket
or assemble a second footprint matrix here.
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
