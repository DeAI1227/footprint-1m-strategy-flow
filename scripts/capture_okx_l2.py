#!/usr/bin/env python3
"""Capture OKX public trades + incremental L2 for script F.

Public WS only. No keys. Writes JSONL under /tmp (stay off git).
Trades are flattened to history-trades rows so orderflowd --replay can load them.
Books are raw books-channel frames for --book-replay.

Historical books are not on public REST. This records a live-forward window.
Do not invent walls from footprint volume. Do not authorize live.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

try:
    import websockets
except ImportError:
    print("need: pip install websockets", file=sys.stderr)
    raise SystemExit(2)

WS = "wss://ws.okx.com:8443/ws/v5/public"


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


async def run(args: argparse.Namespace) -> int:
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    trades_path = out / "trades.jsonl"
    books_path = out / "books.jsonl"
    deadline = time.time() + args.minutes * 60
    n_tr = n_bk = 0
    print(
        f"capture inst={args.inst} minutes={args.minutes} "
        f"until={iso(int((deadline) * 1000))} dir={out}",
        flush=True,
    )
    async with websockets.connect(WS, ping_interval=20, ping_timeout=20, max_size=8_000_000) as ws:
        sub = {
            "op": "subscribe",
            "args": [
                {"channel": "trades", "instId": args.inst},
                {"channel": "books", "instId": args.inst},
            ],
        }
        await ws.send(json.dumps(sub))
        last_ping = time.time()
        with trades_path.open("a") as tf, books_path.open("a") as bf:
            while time.time() < deadline:
                try:
                    raw = await asyncio.wait_for(ws.recv(), timeout=10)
                except asyncio.TimeoutError:
                    if time.time() >= deadline:
                        break
                    # OKX public WS wants a text ping at least every 30s.
                    await ws.send("ping")
                    last_ping = time.time()
                    continue
                if time.time() - last_ping >= 15:
                    await ws.send("ping")
                    last_ping = time.time()
                if raw == "pong":
                    continue
                if isinstance(raw, bytes):
                    raw = raw.decode()
                if raw == "ping":
                    await ws.send("pong")
                    continue
                try:
                    msg = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                ev = msg.get("event")
                if ev in {"subscribe", "unsubscribe", "error", "login"}:
                    print("ws", raw[:240], flush=True)
                    if ev == "error":
                        return 1
                    continue
                arg = msg.get("arg") or {}
                ch = arg.get("channel")
                if ch == "trades":
                    for row in msg.get("data") or []:
                        tf.write(json.dumps(row, separators=(",", ":")) + "\n")
                        n_tr += 1
                    tf.flush()
                elif ch == "books":
                    bf.write(raw if raw.endswith("\n") else raw + "\n")
                    n_bk += 1
                    bf.flush()
                if (n_tr + n_bk) % 400 == 0:
                    print(f"trades={n_tr} books={n_bk} now={iso(int(time.time()*1000))}", flush=True)
    print(f"done trades={n_tr} books={n_bk} dir={out}", flush=True)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--inst", default="SOL-USDT-SWAP")
    ap.add_argument("--out-dir", default="/tmp/sol_l2")
    ap.add_argument("--minutes", type=float, default=90)
    args = ap.parse_args()
    return asyncio.run(run(args))


if __name__ == "__main__":
    raise SystemExit(main())
