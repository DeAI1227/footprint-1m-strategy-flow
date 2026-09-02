# 第 1 天鎖定：看圖定義與顯示設定

日期：**2026-09-02**（時程第 1 天；原表 09-01，實際從今天起算）  
標的 / 所：**SOL-USDT 永續，OKX**  
週期：**1 分鐘已收盤棒**  
今晚：**不校桶寬、不選 SOL 最小量分位、不開倉。**

本頁只抄足跡圖流派大佬**自己講出口或寫進手冊**的東西。YouTube 聯盟文、加密 400% 鐵板、Kill Zone、VWAP **不抄**。

---

## 1. 今晚看哪幾支（定義課，不是找聖盃）

| 誰 | 影片 / 教材 | 他親口鎖死的 |
|---|---|---|
| **Michael Valtos**（Orderflows） | [Imbalances in the Footprint](https://www.youtube.com/watch?v=qo8LM4tyJHE) | 「**I use 4:1**；多數人用 4:1，有人仍用 3:1。」買失衡＝打 ask 遠大於打 bid。綠漲棒裡看買失衡、紅跌棒裡看賣失衡。 |
| **Michael Valtos** | [Effective OF Strategies](https://www.youtube.com/watch?v=7Ds_iYwVi0A) | **整齊相鄰 3 檔**才叫 Stacked（畫盒子、當回踩區）。同向但不整齊叫 Multiple，是動能不是 S/R。影片裡有 5 分鐘舉例——**那是他舉例，不是叫我們改主時鐘。** OFT 手冊：預設給 **1-minute ES**。 |
| **Trader Dale** | [The ONLY Order Flow Trading Guide](https://www.youtube.com/watch?v=o3nfhz_M9j0) | 「way stronger = **三倍或以上**」。**斜對角**比（例：313 vs 下一檔 96）。**連三檔**才叫 stacked；常當回踩的 S/R。單個藍色數字不夠。 |
| **Trader Dale** | [Beginner’s Guide](https://www.youtube.com/watch?v=93QSoSMjOxs) | 「**first week just observe… do not place any trades yet。**」單個失衡沒故事，要堆疊。 |
| **Trader Dale** | 手冊 + [How to Read Order Flow](https://www.trader-dale.com/how-to-read-order-flow-a-simple-guide-to-trading-like-the-big-guys/) | 預設 **300%**；他寫 **I prefer the Default (300%)**。堆疊預設 **3**。用法：離開後拉回盒子。 |
| **ATAS Learn** | [Imbalances](https://learn.atas.net/volume-basics/volume-analysis/imbalances) | 斜對角。**200% = 失衡，400% = 強失衡。** 可依品種與週期改。軟體出廠顯示 **150%**（[設定頁](https://help.atas.net/en/support/solutions/articles/72000606631-footprint-settings)）。 |
| **ATAS Learn** | [Patterns](https://learn.atas.net/volume-basics/volume-analysis/footprint-patterns) | 單個可能隨機；**3 檔以上**才叫 stacking。完成拍賣：極端一側為 0。 |
| **Jigsaw** | 官方 Footprint + [部落格](https://www.jigsawtrading.com/blog/footprint-charts/) | **Tilt Mode 預設 On**＝逼你斜著比。示範影片用 **200% + 50 口 ES**，文案寫 *for this example*、**依品種改**。出廠%未公布。 |

Dale 有一篇文把「5 分鐘足跡 + 1 分鐘 CVD」當他的工作流。那是他的多窗，**不是**本系統改主時鐘。我們主時鐘仍是 1 分鐘收盤，與 OFT「預設 1m ES」同一層觀察。

---

## 2. 鎖死定義（不是參數，21 天內不准改）

從上面大佬抄來，寫成檢查表。看圖前先默一次：

1. **斜對角。** Ask(P) 對 Bid(P−1)。同一格左右互除＝看錯。Dale 影片 313 vs 96；ATAS Learn；Jigsaw Tilt。
2. **空桶不當 0 去除。** ATAS `Ignore Zero Values` 打開。Valtos 手冊：薄量 `1 vs 5`、`0 vs 4` 不實用。
3. **單個失衡 = 噪音。** ATAS Learn 原文；Dale 入門影片「just one number doesn’t mean much」。
4. **堆疊 = 連續相鄰 ≥3 同向。** Dale 影片數「one, two, three」；OFT 產品頁 Stacked = 3 or more neatly stacked。
5. **堆疊用法 = 離開再回踩，不是當根追。** Dale 影片 pullback to the zone；ATAS 2018 test；Valtos Hidden Trade Locations。
6. **完成高 / 完成低。** 最高檔 Bid=0 / 最低檔 Ask=0。未完成兩邊都不是 0。ATAS：不能當獨立進場。OFT 偵測預設關。
7. **當根 VA = 該棒成交量 70%，scope=bar。** 只抄 Orderflows 棒內功能。不是日盤 Profile。
8. **只讀已收盤 1 分鐘。** forming 棒的「快要堆疊」不算今天的定義課。

---

## 3. 今晚軟體怎麼設（顯示用，不是 live）

大佬的數字**不要合成一個**。Valtos 影片說他用 4:1，Dale 影片說 3 倍——並列，不平均成 350%。

| 軟體欄位 | 第 1 天設定 | 出處 | 明天以後 |
|---|---|---|---|
| 品種 | OKX SOL 永續 | 本系統 | 不動 |
| 週期 | 1 分鐘 | OFT 預設給 1m ES；本系統主時鐘 | 不動 |
| 比較方式 | 斜對角 / Tilt On | 全派共識 | **鎖死** |
| Ignore Zero | On | ATAS | **鎖死** |
| 失衡**顯示 / 記錄** | **200%** | ATAS Learn「standard imbalance」 | 第 4 天才並列 300/400 |
| 失衡**武裝（先記著，不開倉）** | **300 與 400 並列，不選邊** | Dale YT+本人 300；Valtos YT+本人 400 | 第 4–14 天比較誰不滿屏 |
| 堆疊長度 | **3** | Dale YT、OFT、ATAS Learn 3+ | 第 5 天才對照 4 |
| 堆疊要跟 K 線同向 | **On** | Valtos OFT：綠棒買堆疊、紅棒賣堆疊 | **鎖死** |
| 未完成拍賣 | 可顯示；**不當進場** | OFT 預設關偵測；ATAS 警告 1m 很多 | 第 7 天數密度 |
| 當根 POC | 開 | 該 1m 量最大桶 | 第 6 天 |
| 當根 VA | 70%，只算當根 | OFT | **鎖死 scope=bar** |
| 最小量 | **先開過濾，數字留空** | Valtos：「依市場改」；ES 的 10 或 50 **不准貼上 SOL** | 第 3 天 |
| 桶寬 | **先維持軟體預設並記下來** | 無人發表 SOL 桶寬 | **第 2 天唯一要改的鍵** |
| VWAP / 日盤 Profile / Naked POC | **關** | 本派已刪 | 鎖死關 |
| 三所加總 / 看 A 打 B | **關** | 本系統 | 鎖死關 |

Exocharts 公開區間 250–400 是加密廠商「常見」，**沒有 SOL 表**，第 1 天不當選邊依據。

---

## 4. 這一小時怎麼用（09-02）

| 分鐘 | 做什麼 |
|---|---|
| 0–10 | 把上表設進你的足跡軟體。核對：斜對角、Ignore Zero、1m、SOL OKX。 |
| 10–25 | 看 Dale [o3nfhz_M9j0](https://www.youtube.com/watch?v=o3nfhz_M9j0) 裡「斜對角 + 連三檔 + 回踩」那一段，對照你圖上的格子。 |
| 25–40 | 看 Valtos [qo8LM4tyJHE](https://www.youtube.com/watch?v=qo8LM4tyJHE) 裡 4:1 與漲棒/跌棒那一段。記住他用 400，Dale 用 300，**不要改成一個數**。 |
| 40–55 | 只看 **已收盤** SOL 1m：亞洲或當下時段即可。**不計堆疊個數當結論**（桶寬明天才校）。只問三句：這格是不是斜著比？有沒有 0 去除的假巨幅？單個紅格我有沒有想追？ |
| 55–60 | 填 `params/sol-observation.md` 的 09-02。結論必須是「定義已鎖，數字未校」。 |

---

## 5. 第 1 天禁止

- 把 Valtos 400 與 Dale 300 平均成 350%
- 把 ES 10 口 / 50 口寫進 SOL
- 因影片裡出現 5 分鐘就把主時鐘改成 5 分鐘
- 抄 Jigsaw 某支影片的個人 100%（講者自己說比 Dale 300 鬆）
- 開倉、追 forming 棒、用盈虧判斷設定好不好
- 打開 VWAP、整數關、爆倉地圖

明天（第 2 天）**只改桶寬**。顯示失衡維持 200%。

---

## 6. 09-02 重做說明（定義不改）

第 2–6 天舊表只有亞盤 **59 根**，美盤／歐盤缺。09-02 用同一套第 1 天定義，把樣本補成 **1790 根已收盤 1m**（2026-08-31 23:59 → 09-02 05:48 UTC；亞 829、歐 300、美 480、極薄 181），**重新跑第 2–7 天**。

本頁 **8 條定義與顯示表不准改**：斜對角、Ignore Zero、200 記錄、300∥400 並列、堆疊 3、棒向 On、VA 70% 當根、未完成不開倉。重做只填數字與第 7 天凍結假設。

