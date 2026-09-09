# orderflow-exec

階段 6：用 **OKX 自己的盤口**做本地假成交（排隊、部分成交）。Kill switch、日虧、距強平緩衝、降載順序在熱路徑。

階段 7：OKX 私有 WS **解碼** + 下單 JSON **編碼** + ACK 分類。默認 shadow。`live_send = false`。即使把校準旗標翻開，這裡也下不了 live HTTP。SUI 影子用自己的 instId / tick，不抄 SOL。無 API 金鑰就能跑完測試。

```bash
cargo test -p orderflow-exec
cargo run -p orderflowd -- --mode sim --sim-fixture crates/orderflow-exec/tests/fixtures/okx_sim.jsonl
cargo run -p orderflowd -- --mode shadow --private-replay crates/orderflow-exec/tests/fixtures/okx_private.jsonl
cargo run -p orderflowd -- --mode live --once   # 必須失敗
```
