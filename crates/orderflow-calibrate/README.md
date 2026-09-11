# orderflow-calibrate

階段 9：填數字**接口**。本計劃不做完校準。觀察稿凍結 ≠ `calibration_complete`。300∥400 仍並列。不准手填教材 400%。不准把 SOL 0.01 抄到 SUI。

```bash
cargo test -p orderflow-calibrate
cargo run -p orderflowd -- --calibrate-check
cargo run -p orderflowd -- --calibrate-journal crates/orderflow-calibrate/tests/fixtures/shadow_stats.jsonl
# 時段拆表（仍不選 300 vs 400）
python3 scripts/oos_shadow_from_journal.py /tmp/sol_oos_journal.jsonl
cargo run -p orderflowd -- --promote-live   # 必須失敗
```
