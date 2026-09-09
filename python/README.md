# Python 句子層

讀 `params/*.toml`，以 `shadow` 啟動。階段 5 起可用 `--journal` 讀 Rust 已閉合 1m 快照，跑腳本 A–G。JSON 日誌不含密鑰。live / live_small 仍因參數未校準而拒絕。階段 8：`--ops-check` 看東京健康快照。階段 9：`--calibrate-check` 只報填數字狀態，不選 300 vs 400。

```bash
PYTHONPATH=python python3 -m orderflow --mode shadow --once
PYTHONPATH=python python3 -m orderflow --mode live --once   # 退出碼 2
PYTHONPATH=python python3 -m orderflow --mode shadow --once --journal /tmp/sol_ctx.jsonl
PYTHONPATH=python python3 -m orderflow --mode sim --once --reconcile-local /tmp/local.json --reconcile-exchange /tmp/exchange.json
PYTHONPATH=python python3 -m orderflow --mode shadow --once --journal /tmp/sol.jsonl --journal-sui /tmp/sui.jsonl
PYTHONPATH=python python3 -m orderflow --ops-check
PYTHONPATH=python python3 -m orderflow --calibrate-check
PYTHONPATH=python python3 -m orderflow --calibrate-journal crates/orderflow-calibrate/tests/fixtures/shadow_stats.jsonl
PYTHONPATH=python python3 -m orderflow --promote-live   # 退出碼 2
python3 scripts/oos_scripts_from_journal.py /tmp/sol_oos_journal.jsonl
```

禁止在這裡解析行情 WebSocket，禁止用 pandas 組第二套生產足跡矩陣。共振預設 `off`，不把外所價抄到 OKX。G 與未完成拍賣不是進場。
