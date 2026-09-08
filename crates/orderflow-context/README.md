# orderflow-context

階段 4：位置與制度欄位。輸入只能是 **已閉合的 1m 足跡**，不准另開一根 OHLC 時鐘。

- 近窗量堆：5 / 15 / 60 / 240 / 1440 根 + 時段窗。輸出近窗 POC 與相鄰厚量帶。**禁止**日盤 TPO / Naked POC / VWAP。
- 擺動：`swing_n`（觀察稿 5），只給套牢腳本當區間。
- 離開後連續 `accept_bars` 根 POC 在區外 → `stack_accepted`；立刻縮回 → `fake_leave`。
- 制度：資金費黑窗用時鐘；清算靠 OI / 強平 sidecar。缺流 → `not_evaluated` + `liq_stream_missing`。只否決，不當進場。
- 共振：三所 delta 符號寫進快照。模式預設 `off`，仍計算、不驅動 OKX 限價。缺所當根 `not_evaluated`，不准拿舊棒充數。
