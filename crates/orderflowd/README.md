# orderflowd

階段 0：載入參數、shadow/sim 啟動、硬拒絕 live。  
階段 1：OKX 公共成交 JSONL replay → 事件時間 1m 棒（已閉合不可改寫）。  
階段 2：replay 時同時凍結分所足跡矩陣（`footprint_closed` JSONL）。300∥400 並列。live 仍拒絕。

```bash
cargo run -p orderflowd -- --mode shadow --once
cargo run -p orderflowd -- --mode live --once   # 退出碼 2，reason=params_not_calibrated
cargo run -p orderflowd -- --mode shadow --replay /tmp/sol_okx_trades.jsonl --max-trades 5000 --journal /tmp/sol_bars_closed.jsonl
cargo run -p orderflowd -- --mode shadow --replay /tmp/sol_binance_agg.jsonl --venue binance
cargo run -p orderflowd -- --mode shadow --replay-bybit /tmp/sol_bybit_trades.csv
```

Replay 所不是執行所。執行仍是 OKX。live 仍拒絕。
