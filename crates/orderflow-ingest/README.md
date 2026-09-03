# orderflow-ingest

階段 1：OKX 公共成交正規化（`side=buy` → taker buy）+ JSONL replay + 閉合棒 journal。

階段 1b 才接 Binance / Bybit 公共 WS。Bybit taker 方向必須先有黃金測試。

禁止把外所價填進 OKX 訂單。三所成交量不加總。
