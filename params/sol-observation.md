# SOL 觀察日誌

對應時程：`specs/footprint-param-calibration-21d.md`  
所：OKX（執行所足跡）。需要時另註 Binance / Bybit 只比方向。

凍結假設（第 7 / 14 / 21 天更新，平時不要整表重寫）：

- 桶寬：第 1 天未校（第 2 天唯一鍵）
- 最小量規則：第 1 天未校（ES 的 10/50 不准貼 SOL）
- 記錄閾：200%（ATAS Learn standard；第 4 天再並列）
- 武裝閾（300 與 400 可並列）：Dale YT/手冊 300 ∥ Valtos YT/OFT 400，不合成
- 堆疊檔數：3（Dale YT、OFT、ATAS Learn 3+）
- 棒向一致：true（Valtos OFT）
- 忽略 0：true（ATAS Ignore Zero）
- 比較：斜對角（Dale YT、ATAS、Jigsaw Tilt）
- SWING_N：5（OFT 預設；第 10 天才用）
- 套牢收回根數：未校
- 當根 VA：70% scope=bar（OFT）
- 未完成：只顯示，不開倉

---

## 模板

```
日期：
時段（至少兩段）：
今日唯一問題：
桶寬 / 記錄閾 / 武裝閾 / 最小量 / 堆疊：

計數（已收盤 1m）：
- 單個失衡：滿屏 / 可數 / 幾乎沒有
- 3 檔堆疊：
- 4 檔堆疊：
- 混亂棒：
- 回踩區 N；有拒絕；被打穿
- 吸收後第二次打穿：
- 未完成：幾乎每根 / 偶爾 / 少

制度（清算 / 極薄 / 資金費 / 壞資料）：
結論＋明天只改：
禁止項自檢：
```

---

## 第 1 週

### 2026-09-01

時程原訂第 1 天。實際開課改到 09-02，本日空白。

### 2026-09-02

日期：2026-09-02  
標的 / 所：SOL / OKX 1m closed  
時段：定義課為主（影片 + 設圖）；盤面只抽已收盤 1m 核對讀法，不取樣計數  
今日唯一問題：把大佬的**定義**鎖進軟體；數字不校。

看圖設定（出處見 `params/day1-locked-setup.md`）：

- 斜對角 / Tilt：On — Dale [o3nfhz_M9j0](https://www.youtube.com/watch?v=o3nfhz_M9j0)、ATAS Learn、Jigsaw Tilt 預設 On
- Ignore Zero：On — ATAS 設定頁
- 記錄閾：200% — ATAS Learn「200% = imbalance」
- 武裝閾：不選邊 — Dale 影片+手冊 **300%**；Valtos 影片 [qo8LM4tyJHE](https://www.youtube.com/watch?v=qo8LM4tyJHE)「**I use 4:1**」+ OFT 預設 400%
- 堆疊：3 — Dale 影片數 1-2-3；OFT Stacked = 3+ neatly；ATAS Learn 3+
- 棒向一致：On — Valtos 綠漲棒買堆疊 / 紅跌棒賣堆疊
- 桶寬：維持軟體預設，只記下來，今晚不改
- 最小量：不填 ES 口數
- VWAP / Profile / Naked POC：關
- 開倉：關（Dale 入門片：「first week just observe, do not place any trades yet」[93QSoSMjOxs](https://www.youtube.com/watch?v=93QSoSMjOxs)）

計數（已收盤 1m）：

- 單個失衡：本日不統計（桶未校，計數無意義）
- 3 檔 / 4 檔 / 混亂棒 / 回踩：本日不統計
- 未完成：已知 1m 會很多（ATAS），第 7 天再數

制度：本日不取樣

結論：定義已鎖。300 與 400 **並列**，不平均。明天只改**桶寬**。顯示仍用 200%。

禁止項自檢：沒有用盈虧；沒有把 300/400 合成 350%；沒有貼 ES 10 口；沒有改主時鐘成 5 分鐘（Valtos 另片有 5m 舉例，當舉例不是契約）；沒有開 VWAP。

### 2026-09-03

### 2026-09-04

### 2026-09-05

### 2026-09-06

### 2026-09-07

## 第 2 週

### 2026-09-08

### 2026-09-09

### 2026-09-10

### 2026-09-11

### 2026-09-12

### 2026-09-13

### 2026-09-14

## 第 3 週

### 2026-09-15

### 2026-09-16

### 2026-09-17

### 2026-09-18

### 2026-09-19

### 2026-09-20

### 2026-09-21
