# orderflowd

階段 0：載入參數、shadow/sim 啟動、硬拒絕 live。  
階段 1：OKX 公共成交 JSONL replay → 事件時間 1m 棒（已閉合不可改寫）。

```bash
cargo run -p orderflowd -- --mode shadow --once
cargo run -p orderflowd -- --mode live --once   # 退出碼 2，reason=params_not_calibrated
cargo run -p orderflowd -- --mode shadow --replay /tmp/sol_okx_trades.jsonl --max-trades 5000 --journal /tmp/sol_bars_closed.jsonl
```

Tokio runtime 已接上。正式公共 WS 長連仍待後續；Binance / Bybit 仍是 stub。
