# orderflow-ingest

階段 0 佔位。階段 1 / 1b 才接 Binance、OKX、Bybit 公共 WS。

行程 `orderflowd` 已用 Tokio runtime；本 crate 階段 1 才開連線。禁止把外所價填進 OKX 訂單。
