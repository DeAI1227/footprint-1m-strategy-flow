"""JSON logs. Never write API keys, secrets, or passphrases."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from typing import Any, Mapping

_FORBIDDEN = ("secret", "apikey", "api_key", "passphrase", "private_key")


def _contains_secret(value: Any) -> bool:
    if isinstance(value, str):
        lower = value.lower()
        return any(token in lower for token in _FORBIDDEN)
    if isinstance(value, Mapping):
        for k, v in value.items():
            if any(token in str(k).lower() for token in _FORBIDDEN):
                return True
            if _contains_secret(v):
                return True
        return False
    if isinstance(value, (list, tuple)):
        return any(_contains_secret(v) for v in value)
    return False


def json_log(level: str, payload: Mapping[str, Any]) -> str:
    if _contains_secret(payload):
        raise ValueError("refusing to log payload that looks like a secret")
    line_obj = {
        "ts": datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        "level": level,
        **dict(payload),
    }
    line = json.dumps(line_obj, ensure_ascii=False, separators=(",", ":"))
    lower = line.lower()
    if any(token in lower for token in _FORBIDDEN):
        raise ValueError("refusing to log line that looks like a secret")
    return line
