# orderflow-ingest

階段 1：OKX 公共成交正規化（`side=buy` → taker buy）+ JSONL replay + 閉合棒 journal。

階段 1b：Binance USD-M aggTrade 與 Bybit linear `publicTrade` 接到同一內部 `Trade`。

- Bybit 官方欄位：`S` = **Side of taker**（`Buy`/`Sell`）；`T` = 成交時間（毫秒）。黃金測試必須保持綠。
- Binance 官方欄位：`m` / `isBuyerMaker`；`true` → taker sell，`false` → taker buy。`T` 為毫秒。
- 時間戳單位用數值量級判斷（秒 / 毫秒 / 微秒），禁止用「這是哪一所」猜。
- `ThreeLanes` 有界佇列：一所滿只標該所 `gap`，不得阻塞 OKX。
- 禁止把外所價填進 OKX 訂單。三所成交量不加總。共振模式仍是 `off`。
