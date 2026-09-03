# orderflow-clock

階段 1：**交易所事件時間**切 1 分鐘棒 `[t, t+60)`。

- 只有 `Closed` 可當進場時鐘
- `Forming` 只准撤單 / 風控 / 預警
- **已閉合棒禁止改寫**；晚到成交只加 `late_trade`
- 亂序在 forming 內依事件時間修正 open/close
- 重連不重開已閉合棒

使用：`BarCutter::push` / `flush`。
