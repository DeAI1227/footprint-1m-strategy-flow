# Tokyo run notes (stage 8)

- Region: ap-northeast-1 conceptually; this repo does not store cloud secrets.
- Clock: NTP / chrony is a monitored input, not an assumption. 1m bars use exchange event time.
- Funding black window is **clock** (UTC 00/08/16 ± minutes from toml), not a kill-zone and not an entry.
- Disk: `[ops] disk_min_free_bytes` watermark. Unknown free space does not false-trip.
- Logs: size rotate `log_max_bytes` / keep `log_keep`. Journal hot window `hot_journal_days`.
- Crash fuse: N crashes in T seconds (`crash_burst` / `crash_window_s`) persist and **stop**. Restart does not auto-clear (`clear_crash_on_start = false`). systemd `StartLimitBurst=5` / `StartLimitIntervalSec=120` matches. Do not loop-hit the API.
- Tick / lot / contract change: rebuild **that symbol's** forming footprint, cancel its working orders, pause new opens until the next clean closed 1m. SOL must not rebuild SUI.
- Missed closed 1m: stop opens (degrade), never stop flatten/risk.
- IP allowlists belong in the operator's vault, never in git.
- One venue stall must not block the other two (ingest queues, stage 1b).
- Live still gated. No secrets in units or JSON logs.
