# orderflowd

階段 0 行程。Tokio runtime 已接上，尚未開 WS。

```bash
cargo run -p orderflowd -- --mode shadow --once
cargo run -p orderflowd -- --mode live --once   # 退出碼 2，reason=params_not_calibrated
```
