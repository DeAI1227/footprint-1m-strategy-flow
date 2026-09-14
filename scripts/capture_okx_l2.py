#!/usr/bin/env python3
"""Capture OKX public trades + incremental L2 for script F.

Public WS only. No keys. Writes JSONL under /tmp (stay off git).
Trades are flattened to history-trades rows so orderflowd --replay can load them.
Books are raw books-channel frames for --book-replay.

Historical books are not on public REST. This records a live-forward window.
Do not invent walls from footprint volume. Do not authorize live.

Uses websocket-client with a socket timeout. The asyncio websockets recv()
can hang the event loop so keepers and wait_for never fire; an external
mtime watchdog is still the last line of defense.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

try:
    import websocket
except ImportError:
    print("need: pip install websocket-client", file=sys.stderr)
    raise SystemExit(2)

WS = "wss://ws.okx.com:8443/ws/v5/public"
RECV_TIMEOUT_S = 12.0
STALE_AFTER_S = 25.0


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def run(args: argparse.Namespace) -> int:
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
    sub = json.dumps(
        {
            "op": "subscribe",
            "args": [
                {"channel": "trades", "instId": args.inst},
                {"channel": "books", "instId": args.inst},
            ],
        }
    )
    with trades_path.open("a") as tf, books_path.open("a") as bf:
        while time.time() < deadline:
            ws = None
            try:
                ws = websocket.create_connection(WS, timeout=RECV_TIMEOUT_S)
                ws.settimeout(RECV_TIMEOUT_S)
                ws.send(sub)
                last_data = time.time()
                last_ping = 0.0
                print(f"ws connected now={iso(int(time.time() * 1000))}", flush=True)
                while time.time() < deadline:
                    now = time.time()
                    if now - last_ping >= 12:
                        try:
                            ws.send("ping")
                        except Exception as e:
                            print(f"ping send fail {e!r}", flush=True)
                            break
                        last_ping = now
                    if now - last_data > STALE_AFTER_S:
                        print("stale 25s, closing", flush=True)
                        break
                    try:
                        raw = ws.recv()
                    except websocket.WebSocketTimeoutException:
                        continue
                    last_data = time.time()
                    if raw == "pong":
                        continue
                    if isinstance(raw, bytes):
                        raw = raw.decode()
                    if raw == "ping":
                        ws.send("pong")
                        continue
                    try:
                        msg = json.loads(raw)
                    except json.JSONDecodeError:
                        continue
                    ev = msg.get("event")
                    if ev in {"subscribe", "unsubscribe", "error", "login"}:
                        print("ws", raw[:240], flush=True)
                        if ev == "error":
                            break
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
                    if (n_tr + n_bk) % 2000 == 0:
                        print(
                            f"trades={n_tr} books={n_bk} now={iso(int(time.time() * 1000))}",
                            flush=True,
                        )
            except Exception as e:
                print(f"ws drop {e!r}; sleep 2s", flush=True)
                time.sleep(2)
            finally:
                if ws is not None:
                    try:
                        ws.close()
                    except Exception:
                        pass
    print(f"done trades={n_tr} books={n_bk} dir={out}", flush=True)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--inst", default="SOL-USDT-SWAP")
    ap.add_argument("--out-dir", default="/tmp/sol_l2")
    ap.add_argument("--minutes", type=float, default=90)
    args = ap.parse_args()
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
