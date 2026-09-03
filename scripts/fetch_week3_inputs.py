#!/usr/bin/env python3
"""Download week-3 calibration inputs into /tmp. JSONL/CSV stay off git.

Sources (public only):
- OKX SUI-USDT-SWAP trades + 1m candles
- OKX SOL/SUI open-interest history, funding, liquidation-orders
- Binance USD-M 1m kline zips (data.binance.vision; fapi is 451 here)
- Bybit linear public trade dumps (public.bybit.com; live REST is 403 here)
"""
from __future__ import annotations

import argparse
import gzip
import io
import json
import time
import urllib.parse
import urllib.request
import zipfile
from datetime import datetime, timezone
from pathlib import Path

UA = {"User-Agent": "footprint-1m-calibration/0.1"}
OKX = "https://www.okx.com"


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def get_json(url: str, retries: int = 8) -> dict | list:
    delay = 1.0
    last = None
    for _ in range(retries):
        try:
            req = urllib.request.Request(url, headers=UA)
            with urllib.request.urlopen(req, timeout=30) as resp:
                return json.loads(resp.read().decode())
        except Exception as e:
            last = e
            time.sleep(delay)
            delay = min(delay * 1.7, 12)
    raise RuntimeError(f"{url} failed: {last!r}")


def get_bytes(url: str, retries: int = 6) -> bytes:
    delay = 1.0
    last = None
    for _ in range(retries):
        try:
            req = urllib.request.Request(url, headers=UA)
            with urllib.request.urlopen(req, timeout=60) as resp:
                return resp.read()
        except Exception as e:
            last = e
            time.sleep(delay)
            delay = min(delay * 1.7, 12)
    raise RuntimeError(f"{url} failed: {last!r}")


def fetch_okx_trades(inst: str, path: Path, until_ts_ms: int, max_pages: int) -> None:
    url = f"{OKX}/api/v5/market/history-trades"
    newest = get_json(f"{url}?{urllib.parse.urlencode({'instId': inst, 'limit': 100})}")
    rows = newest.get("data") or []
    if not rows:
        raise RuntimeError(f"no trades for {inst}")
    path.parent.mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    added = 0
    pages = 0
    oldest_ts = int(rows[-1]["ts"])
    after_id = str(rows[-1]["tradeId"])
    t0 = time.time()
    with path.open("w") as out:
        for r in rows:
            tid = str(r["tradeId"])
            if tid in seen:
                continue
            seen.add(tid)
            out.write(json.dumps(r, separators=(",", ":")) + "\n")
            added += 1
        while pages < max_pages and oldest_ts > until_ts_ms:
            q = urllib.parse.urlencode({"instId": inst, "limit": 100, "after": after_id})
            try:
                body = get_json(f"{url}?{q}", retries=4)
            except Exception as e:
                print(f"trade error {e!r}; sleep 2s", flush=True)
                time.sleep(2)
                continue
            page = body.get("data") or []
            if not page:
                print("empty trade page; stop", flush=True)
                break
            stop = False
            for r in page:
                ts = int(r["ts"])
                if ts < until_ts_ms:
                    stop = True
                    break
                tid = str(r["tradeId"])
                if tid in seen:
                    continue
                seen.add(tid)
                out.write(json.dumps(r, separators=(",", ":")) + "\n")
                added += 1
                oldest_ts = ts
            out.flush()
            pages += 1
            after_id = str(page[-1]["tradeId"])
            oldest_ts = int(page[-1]["ts"])
            if pages % 40 == 0:
                rate = added / max(time.time() - t0, 1)
                print(
                    f"{inst} pages={pages} added={added} oldest={iso(oldest_ts)} {rate:.0f}/s",
                    flush=True,
                )
            if stop:
                break
    print(f"done trades {inst} pages={pages} added={added} oldest={iso(oldest_ts)} -> {path}", flush=True)


def fetch_okx_candles(inst: str, path: Path, n_need: int = 3600) -> None:
    url = f"{OKX}/api/v5/market/history-candles"
    rows: list[list] = []
    after = None
    while len(rows) < n_need:
        q = {"instId": inst, "bar": "1m", "limit": "300"}
        if after is not None:
            q["after"] = str(after)
        body = get_json(f"{url}?{urllib.parse.urlencode(q)}")
        page = body.get("data") or []
        if not page:
            break
        rows.extend(page)
        after = page[-1][0]
        time.sleep(0.05)
    path.write_text("\n".join(json.dumps(r, separators=(",", ":")) for r in rows) + "\n")
    print(f"candles {inst} n={len(rows)} {iso(int(rows[-1][0]))} → {iso(int(rows[0][0]))} -> {path}", flush=True)


def fetch_oi(inst: str, period: str, path: Path, pages: int = 12) -> None:
    url = f"{OKX}/api/v5/rubik/stat/contracts/open-interest-history"
    rows: list[list] = []
    end = None
    for _ in range(pages):
        q = {"instId": inst, "period": period, "limit": "100"}
        if end is not None:
            q["end"] = str(end)
        body = get_json(f"{url}?{urllib.parse.urlencode(q)}")
        page = body.get("data") or []
        if not page:
            break
        rows.extend(page)
        end = int(page[-1][0])
        if len(page) < 100:
            break
    path.write_text("\n".join(json.dumps(r, separators=(",", ":")) for r in rows) + "\n")
    print(f"oi {inst} {period} n={len(rows)} -> {path}", flush=True)


def fetch_funding(inst: str, path: Path) -> None:
    url = f"{OKX}/api/v5/public/funding-rate-history?{urllib.parse.urlencode({'instId': inst, 'limit': 100})}"
    body = get_json(url)
    rows = body.get("data") or []
    path.write_text("\n".join(json.dumps(r, separators=(",", ":")) for r in rows) + "\n")
    print(f"funding {inst} n={len(rows)} -> {path}", flush=True)


def fetch_liq(uly: str, path: Path, max_pages: int = 40) -> None:
    url = f"{OKX}/api/v5/public/liquidation-orders"
    details: list[dict] = []
    after = None
    for i in range(max_pages):
        q = {"instType": "SWAP", "uly": uly, "state": "filled", "limit": 100}
        if after is not None:
            q["after"] = str(after)
        body = get_json(f"{url}?{urllib.parse.urlencode(q)}")
        data = body.get("data") or []
        page = []
        for block in data:
            page.extend(block.get("details") or [])
        if not page:
            break
        details.extend(page)
        after = min(int(x["ts"]) for x in page)
        if i % 5 == 0:
            print(f"liq {uly} pages={i+1} n={len(details)} oldest={iso(after)}", flush=True)
        if len(page) < 50:
            break
    path.write_text("\n".join(json.dumps(r, separators=(",", ":")) for r in details) + "\n")
    print(f"liq {uly} n={len(details)} -> {path}", flush=True)


def fetch_binance_klines(days: list[str], path: Path) -> None:
    rows: list[list] = []
    for day in days:
        url = (
            "https://data.binance.vision/data/futures/um/daily/klines/SOLUSDT/1m/"
            f"SOLUSDT-1m-{day}.zip"
        )
        try:
            raw = get_bytes(url)
        except Exception as e:
            print(f"binance {day} missing: {e}", flush=True)
            continue
        zf = zipfile.ZipFile(io.BytesIO(raw))
        name = zf.namelist()[0]
        text = zf.read(name).decode()
        for line in text.splitlines():
            if not line or line.startswith("open_time"):
                continue
            rows.append(line.split(","))
        print(f"binance {day} lines so far {len(rows)}", flush=True)
    path.write_text("\n".join(",".join(r) for r in rows) + "\n")
    print(f"binance klines n={len(rows)} -> {path}", flush=True)


def fetch_bybit_trades(symbol: str, days: list[str], path: Path) -> None:
    n = 0
    with path.open("w") as out:
        for day in days:
            url = f"https://public.bybit.com/trading/{symbol}/{symbol}{day}.csv.gz"
            raw = get_bytes(url)
            text = gzip.GzipFile(fileobj=io.BytesIO(raw)).read().decode()
            first = True
            for line in text.splitlines():
                if first:
                    first = False
                    if line.lower().startswith("timestamp"):
                        continue
                if line.strip():
                    out.write(line + "\n")
                    n += 1
            print(f"bybit {symbol} {day} total_lines={n}", flush=True)
    print(f"bybit {symbol} n={n} -> {path}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--until-ts-ms",
        type=int,
        default=int(datetime(2026, 8, 31, 23, 59, tzinfo=timezone.utc).timestamp() * 1000),
        help="Stop paginating OKX trades at this exchange time (default 2026-08-31 23:59 UTC)",
    )
    ap.add_argument("--max-pages", type=int, default=20000)
    ap.add_argument("--skip-sui-trades", action="store_true")
    args = ap.parse_args()

    # Default until = 2026-08-31 23:59 UTC if caller passes the week-1/2 start.
    print(f"until {iso(args.until_ts_ms)}", flush=True)

    fetch_okx_candles("SUI-USDT-SWAP", Path("/tmp/sui_okx_candles_1m.jsonl"))
    fetch_oi("SOL-USDT-SWAP", "5m", Path("/tmp/sol_okx_oi_5m.jsonl"))
    fetch_oi("SUI-USDT-SWAP", "5m", Path("/tmp/sui_okx_oi_5m.jsonl"))
    fetch_oi("SOL-USDT-SWAP", "1H", Path("/tmp/sol_okx_oi_1h.jsonl"))
    fetch_oi("SUI-USDT-SWAP", "1H", Path("/tmp/sui_okx_oi_1h.jsonl"))
    fetch_funding("SOL-USDT-SWAP", Path("/tmp/sol_okx_funding.jsonl"))
    fetch_funding("SUI-USDT-SWAP", Path("/tmp/sui_okx_funding.jsonl"))
    fetch_liq("SOL-USDT", Path("/tmp/sol_okx_liq.jsonl"))
    fetch_liq("SUI-USDT", Path("/tmp/sui_okx_liq.jsonl"))
    fetch_binance_klines(["2026-08-31", "2026-09-01", "2026-09-02"], Path("/tmp/sol_binance_um_1m.csv"))
    fetch_bybit_trades("SOLUSDT", ["2026-08-31", "2026-09-01", "2026-09-02"], Path("/tmp/sol_bybit_trades.csv"))
    if not args.skip_sui_trades:
        fetch_okx_trades(
            "SUI-USDT-SWAP",
            Path("/tmp/sui_okx_trades.jsonl"),
            until_ts_ms=args.until_ts_ms,
            max_pages=args.max_pages,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
