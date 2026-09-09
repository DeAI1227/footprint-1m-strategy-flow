#!/usr/bin/env python3
"""Fetch OKX public SOL-USDT-SWAP trades for SOL out-of-sample shadow.

Writes newest-first JSONL under /tmp. Stay off git. Public REST only; no keys.
Windows are jumped by tradeId then paginated older (same as observation dumps).
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

URL = "https://www.okx.com/api/v5/market/history-trades"
UA = {"User-Agent": "footprint-1m-calibration/0.1"}
INST = "SOL-USDT-SWAP"


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def fetch_page(after_id: str | None = None, limit: int = 100) -> list[dict]:
    q: dict[str, str] = {"instId": INST, "limit": str(limit)}
    if after_id is not None:
        q["after"] = str(after_id)
    req = urllib.request.Request(f"{URL}?{urllib.parse.urlencode(q)}", headers=UA)
    with urllib.request.urlopen(req, timeout=20) as resp:
        body = json.loads(resp.read().decode())
    if str(body.get("code")) != "0":
        raise RuntimeError(body)
    return body.get("data") or []


def newest_trade() -> dict:
    rows = fetch_page()
    if not rows:
        raise RuntimeError("no trades")
    return rows[0]


def trade_near(target_ts_ms: int, newest_id: int, newest_ts: int) -> dict:
    """Find a trade at or slightly newer than target_ts_ms via tradeId jumps."""
    hours = max((newest_ts - target_ts_ms) / 3_600_000, 0.1)
    # Observation-era SOL was ~35–50k trades/hour; start from that guess.
    guess = newest_id - int(hours * 40_000)
    lo = max(1, newest_id - 4_000_000)
    hi = newest_id + 1
    mid = max(lo + 1, min(hi - 1, guess))
    best = None
    for _ in range(24):
        try:
            rows = fetch_page(after_id=str(mid))
        except Exception:
            time.sleep(0.4)
            continue
        if not rows:
            hi = mid
            mid = (lo + hi) // 2
            continue
        row = rows[0]
        ts = int(row["ts"])
        tid = int(row["tradeId"])
        best = row
        if ts >= target_ts_ms:
            hi = mid
        else:
            lo = max(lo, tid)
        if hi - lo <= 200:
            break
        mid = (lo + hi) // 2
    if best is None:
        raise RuntimeError(f"no trade near {iso(target_ts_ms)}")
    # Nudge newer until the page straddles the boundary.
    after = str(int(best["tradeId"]) + 1)
    for _ in range(40):
        try:
            rows = fetch_page(after_id=after)
        except Exception:
            break
        if not rows:
            break
        if int(rows[0]["ts"]) < target_ts_ms:
            break
        best = rows[0]
        after = str(int(rows[-1]["tradeId"]))
        if int(rows[-1]["ts"]) < target_ts_ms:
            break
    print(
        f"near {iso(target_ts_ms)} -> id={best['tradeId']} {iso(int(best['ts']))} "
        f"(newest was {newest_id} {iso(newest_ts)})",
        flush=True,
    )
    return best


def session_windows(end_ts_ms: int, until_ts_ms: int) -> list[tuple[str, int, int]]:
    """Split [until, end) into UTC session slices, newest first.

    Asia 0–8, EU 8–13, US 13–21, thin 21–24.
    """
    from datetime import timedelta

    cuts = []
    t = until_ts_ms
    end = end_ts_ms
    while t < end:
        dt = datetime.fromtimestamp(t / 1000, timezone.utc)
        hour = dt.hour
        if hour < 8:
            sess, nxt_h = "asia", 8
        elif hour < 13:
            sess, nxt_h = "eu", 13
        elif hour < 21:
            sess, nxt_h = "us", 21
        else:
            sess, nxt_h = "thin", 24
        nxt = dt.replace(hour=0, minute=0, second=0, microsecond=0)
        if nxt_h == 24:
            nxt = nxt + timedelta(days=1)
        else:
            nxt = nxt.replace(hour=nxt_h)
        nxt_ms = min(int(nxt.timestamp() * 1000), end)
        day = dt.strftime("%Y%m%d")
        cuts.append((f"{sess}_{day}", nxt_ms, t))
        t = nxt_ms
    cuts.reverse()  # newest session first
    return cuts


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", default="/tmp/sol_oos")
    ap.add_argument(
        "--end-ts-ms",
        type=int,
        default=0,
        help="Exclusive newer bound (default: now)",
    )
    ap.add_argument(
        "--until-ts-ms",
        type=int,
        default=int(datetime(2026, 9, 8, tzinfo=timezone.utc).timestamp() * 1000),
        help="Oldest timestamp to keep",
    )
    ap.add_argument("--max-pages", type=int, default=8000)
    args = ap.parse_args()

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    newest = newest_trade()
    nid, nts = int(newest["tradeId"]), int(newest["ts"])
    print(f"newest id={nid} {iso(nts)} px={newest['px']}", flush=True)

    end_ts = args.end_ts_ms if args.end_ts_ms > 0 else nts + 1
    bounds = session_windows(end_ts, args.until_ts_ms)
    if not bounds:
        print("empty window", flush=True)
        return 1

    script = Path(__file__).with_name("fetch_okx_trade_window.py")
    procs: list[subprocess.Popen] = []
    for name, newer_ts, until_ts in bounds:
        if newer_ts <= until_ts:
            print(f"skip {name}: empty window", flush=True)
            continue
        near = trade_near(newer_ts, nid, nts)
        after_id = str(int(near["tradeId"]) + 1)
        path = out_dir / f"{name}.jsonl"
        cmd = [
            sys.executable,
            str(script),
            "--path",
            str(path),
            "--after-id",
            after_id,
            "--until-ts-ms",
            str(until_ts),
            "--max-pages",
            str(args.max_pages),
        ]
        print("spawn", " ".join(cmd), flush=True)
        log = (out_dir / f"{name}.log").open("w")
        procs.append(subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT))

    rc = 0
    for p in procs:
        rc = max(rc, p.wait())
    print(f"windows done rc={rc} dir={out_dir}", flush=True)
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
