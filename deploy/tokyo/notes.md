# Tokyo run notes (stage 0 placeholder; ops land in stage 8)

- Region: ap-northeast-1 conceptually; this repo does not store cloud secrets.
- Clock: NTP / chrony is a monitored input, not an assumption. 1m bars use exchange event time.
- Disk: journal and logs need a water-mark; not wired yet.
- IP allowlists belong in the operator's vault, never in git.
- One venue stall must not block the other two (ingest queues, stage 1b).
