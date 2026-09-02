#!/usr/bin/env python3
"""Paginate OKX public history-trades for SOL-USDT-SWAP into a JSONL file.

Newest-first file layout (matches existing /tmp/sol_okx_trades.jsonl):
the last line is the oldest trade. We append older pages using `after=tradeId`.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone

INST = "SOL-USDT-SWAP"
URL = "https://www.okx.com/api/v5/market/history-trades"


def parse_ts_ms(s: str) -> int:
    return int(s)


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def last_trade(path: str) -> dict | None:
    last = None
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                last = line
    return json.loads(last) if last else None


def fetch_page(after_id: str, limit: int = 100) -> list[dict]:
    q = urllib.parse.urlencode({"instId": INST, "limit": str(limit), "after": after_id})
    req = urllib.request.Request(
        f"{URL}?{q}",
        headers={"User-Agent": "footprint-1m-calibration/0.1"},
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        body = json.loads(resp.read().decode())
    if str(body.get("code")) != "0":
        raise RuntimeError(body)
    return body.get("data") or []


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--path", default="/tmp/sol_okx_trades.jsonl")
    ap.add_argument("--until-ts-ms", type=int, required=True, help="Stop when oldest ts <= this")
    ap.add_argument("--sleep", type=float, default=0.0)
    ap.add_argument("--max-pages", type=int, default=20000)
    args = ap.parse_args()

    last = last_trade(args.path)
    if last is None:
        print("empty file", file=sys.stderr)
        return 1
    after_id = str(last["tradeId"])
    oldest_ts = parse_ts_ms(last["ts"])
    pages = 0
    added = 0
    t0 = time.time()
    print(f"resume after={after_id} oldest={iso(oldest_ts)} target={iso(args.until_ts_ms)}", flush=True)

    with open(args.path, "a") as out:
        while pages < args.max_pages and oldest_ts > args.until_ts_ms:
            try:
                rows = fetch_page(after_id)
            except Exception as e:
                print(f"error {e!r}; sleep 2s", flush=True)
                time.sleep(2)
                continue
            if not rows:
                print("empty page; history window ended", flush=True)
                break
            for r in rows:
                out.write(json.dumps(r, separators=(",", ":")) + "\n")
            out.flush()
            added += len(rows)
            pages += 1
            after_id = str(rows[-1]["tradeId"])
            oldest_ts = parse_ts_ms(rows[-1]["ts"])
            if pages % 50 == 0:
                rate = added / max(time.time() - t0, 1)
                print(
                    f"pages={pages} added={added} oldest={iso(oldest_ts)} {rate:.0f} trades/s",
                    flush=True,
                )
            time.sleep(args.sleep)

    print(f"done pages={pages} added={added} oldest={iso(oldest_ts)}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
