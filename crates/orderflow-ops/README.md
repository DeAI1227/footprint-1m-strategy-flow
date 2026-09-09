# orderflow-ops

階段 8：東京運行面。崩潰熔斷、日誌輪轉、磁碟水位、tick 變更重建**該標的**、漏棒停開倉不停風控、資金費黑窗當時鐘。無 API 金鑰。live 仍拒絕。

```bash
cargo test -p orderflow-ops
cargo run -p orderflowd -- --ops-check
cargo run -p orderflowd -- --mode shadow --spec-replay crates/orderflow-ops/tests/fixtures/spec_replay.jsonl
cargo run -p orderflowd -- --mode live --once   # 必須失敗
```
