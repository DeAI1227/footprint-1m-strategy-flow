# orderflow-footprint

階段 2：分所 1m 足跡矩陣。三所各一張，禁止加總成交量。

- 斜對角、Ignore Zero、堆疊 3、棒向=開收盤
- 當根 POC / 當根 70% VA（scope=bar，不是日盤 TPO）
- 未完成可顯示，**不當進場**
- 300%（Dale）與 400%（Valtos）並列，不合成 350%
- 最小量 = 該時段非空單側 p25（先算歷史再套當根）
- 吸收 / 腳本 F = `not_evaluated`（要 L2）

Python 研究腳本（`scripts/`）不得提升成生產熱路徑。
