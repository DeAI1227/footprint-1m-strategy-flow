#!/usr/bin/env python3
"""Fetch one OKX trade window by jumping near a start tradeId, then paginating older.

Trade IDs on SOL-USDT-SWAP are sequential enough to jump hours. Output JSONL
newest-first (same as history-trades pages).
"""
from __future__ import annotations

import argparse
import json
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone

INST = "SOL-USDT-SWAP"
URL = "https://www.okx.com/api/v5/market/history-trades"
UA = {"User-Agent": "footprint-1m-calibration/0.1"}


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def fetch_page(after_id: str, limit: int = 100) -> list[dict]:
    q = urllib.parse.urlencode({"instId": INST, "limit": str(limit), "after": after_id})
    req = urllib.request.Request(f"{URL}?{q}", headers=UA)
    with urllib.request.urlopen(req, timeout=20) as resp:
        body = json.loads(resp.read().decode())
    if str(body.get("code")) != "0":
        raise RuntimeError(body)
    return body.get("data") or []


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--path", required=True)
    ap.add_argument("--after-id", required=True, help="tradeId newer than the window (exclusive)")
    ap.add_argument("--until-ts-ms", type=int, required=True)
    ap.add_argument("--max-pages", type=int, default=8000)
    args = ap.parse_args()

    after_id = str(args.after_id)
    pages = added = 0
    oldest_ts = None
    t0 = time.time()
    print(f"window after={after_id} until={iso(args.until_ts_ms)} -> {args.path}", flush=True)
    with open(args.path, "w") as out:
        while pages < args.max_pages:
            try:
                rows = fetch_page(after_id)
            except Exception as e:
                print(f"error {e!r}; sleep 1.5s", flush=True)
                time.sleep(1.5)
                continue
            if not rows:
                print("empty page", flush=True)
                break
            stop = False
            for r in rows:
                ts = int(r["ts"])
                if ts < args.until_ts_ms:
                    stop = True
                    break
                out.write(json.dumps(r, separators=(",", ":")) + "\n")
                added += 1
                oldest_ts = ts
            out.flush()
            pages += 1
            after_id = str(rows[-1]["tradeId"])
            if pages % 40 == 0:
                rate = added / max(time.time() - t0, 1)
                print(
                    f"pages={pages} added={added} oldest={iso(oldest_ts or 0)} {rate:.0f}/s",
                    flush=True,
                )
            if stop:
                break
    print(f"done pages={pages} added={added} oldest={iso(oldest_ts or 0)}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
