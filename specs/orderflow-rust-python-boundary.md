# 語言與三所連線契約

本文鎖定兩件事：Binance / OKX / Bybit 公共 WebSocket 全接；熱路徑用 Rust，句子層用 Python。與 [orderflow-1m-tokyo-system-elements.md](orderflow-1m-tokyo-system-elements.md) 衝突時，以元素總表為準，本文只把邊界寫到實作不會走樣的程度。

## 三所

| 角色 | 所 | 公共 WS | 私有 WS | 足跡用途 |
|---|---|---|---|---|
| 執行 | OKX linear | 必接 | 必接 | 進出場價位與結構的唯一來源 |
| 共振 | Binance USD-M | 必接 | 預設不接 | 當根方向確認 / 鉛滯研究 |
| 共振 | Bybit linear | 必接 | 預設不接 | 與 Binance 平級，不是備援 |

三所各算各的 1m 矩陣。共振比方向，不比同一價格、不加總成交量。任一所熱路徑阻塞不得拖死另外兩所。

Bybit 適配必須有黃金測試：public trade 的 taker 方向欄位、時間戳單位、orderbook checksum 重建。猜錯方向等於全校反號。

## 為什麼熱路徑是 Rust

三所同時推送 SOL/SUI 的成交與 L2 時，Python asyncio 單執行緒解 JSON、更新簿、切桶，在高峰容易漏棒或讓 book 增量堆積。漏棒比「決策慢 2ms」更致命。Rust 負責在時限內把事件變成凍結的 1m 快照；Python 負責讀快照說句子。

## 必須 Rust

- 三所 WS 連線、訂閱、重連、frame 解碼
- Trade 正規化為 `taker_buy` / `taker_sell`
- L2 apply、checksum、重建、牆的成交核對
- 事件時間切棒、足跡矩陣、CVD 增量、品質向量（含分所）
- OKX 私有 WS 解碼
- 下單/撤單寫出與 ACK（含 kill switch），避免 GIL 卡住平倉

## 必須 Python

- 腳本 A–G、確認否決、失效句子
- `params/sol.toml`、`params/sui.toml`、共振模式開關
- 影子統計、日報、告警
- REST 對帳編排（比對的本地單簿仍在 Rust）
- replay 測試驅動（計算核心仍呼叫 Rust）

## 交接

每根 closed 1m：Rust 產出版本化凍結快照。Python 只讀快照，輸出 OrderIntent。forming 上的撤單與緊急平倉不進 Python 決策迴圈。

首選 PyO3 單進程、Rust 多執行緒、GIL 釋放。若拆 `orderflowd`，Python 死亡不得阻止 Rust 風控。

禁止生產熱路徑用 Python 解析行情或用 pandas 組足跡。禁止為了「先跑通」雙寫第二套 Python 矩陣。
