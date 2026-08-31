# 流派元素：專家參數基準稿（訂策略用，不是寫代碼）

本文只收 **足跡圖流派**（cluster / footprint / bid×ask / delta / 失衡堆疊 / 未完成拍賣 / 吸收）。  
下列 **不屬本派，已刪除、不再當流派元素**：Market Profile / TPO、Initial Balance、Dalton/Steidlmayer 的 68%–70% 日價值區算法、Naked/Virgin POC、poor high/low、VWAP / AVWAP / σ 帶、形態學、SMC。

Orderflows 軟體裡對 **單根足跡棒** 算的 70% 價值區、當根 POC，仍屬足跡軟體功能，保留。那不是日盤 TPO Profile。

讀法先鎖死：

- **軟體預設**：平台出廠值，用來顯示，不一定是他本人拿來開倉的閾。
- **教材數字**：該作者在文章、手冊、課程裡明確寫下的。
- **用法**：他們把這個元素當支撐、當過濾、還是當磁鐵。這比數字重要。
- **未公開**：找不到可靠出處就寫「未公開」，不編。

本輪能核到的一手來源（2026-08-31 查閱）：

- ATAS 說明：<https://help.atas.net/en/support/solutions/articles/72000606631-footprint-settings>
- ATAS Learn 失衡課：<https://learn.atas.net/volume-basics/volume-analysis/imbalances>
- ATAS Learn 形態課：<https://learn.atas.net/volume-basics/volume-analysis/footprint-patterns>
- ATAS 部落格失衡（ES 5 分鐘範例）：<https://atas.net/blog/imbalance-trade-on-the-side-of-superior-forces/>
- ATAS 未完成拍賣：<https://atas.net/blog/unfinished-auction-what-it-is-and-how-to-trade-it/>
- Orderflows Trader 7 使用手冊（**Michael Valtos**，不是 Tom Alexander）：<https://indicatormall.com/files/products/orderflows-trader-7-nt8/files/OFT7UserGuide.pdf>
- Orderflows 產品頁：<https://www.orderflows.com/special.html>、<https://www.orderflows.com/oft8/>
- Trader Dale Order Flow 手冊（PDF）：<https://www.trader-dale.com/wp-content/uploads/2020/05/TD-Order-Flow-Manual-2.0-4-5-2024.pdf>
- Trader Dale 入門：<https://www.trader-dale.com/beginners-guide-to-order-flow-part-2-special-features/>
- Sierra Chart Numbers Bars / VAP Threshold Alert V2：<https://www.sierrachart.com/index.php?page=doc%2FNumbersBars.php>、<https://www.sierrachart.com/index.php?ID=386&page=doc%2FStudiesReference.php>
- Jigsaw 足跡說明與部落格：<https://daytradr.jigsawtrading.com/jt-footprint.html>、<https://www.jigsawtrading.com/blog/footprint-charts/>
- Bookmap Absorption：<https://bookmap.com/knowledgebase/docs/KB-Indicators-Absorption>

這些人主要教的是 **ES / NQ 等股指期貨、有 RTH 的市場**。把他們的「10 口、50 口」直接貼到 SOL/SUI 永續，是錯用，不是學習。

---

## 0. 專家真正共識（比任何單一百分比重要）

1. **斜對角比，不要同一格左右互除。** ATAS Learn、Sierra、Jigsaw Tilt Mode、Trader Dale 手冊都寫死：Ask 在 P 對 Bid 在 P−1。
2. **單個失衡當噪音。** ATAS Learn：「A single imbalance at one level may be random」。ATAS 2018 文：「A single buy or sell imbalance… doesn’t mean anything yet。」
3. **堆疊才進入「區域」語言，教材與範例多落在連續 3 檔。** Orderflows 與 Dale 是軟體預設 3。ATAS Learn 把 3+ 叫 stacking（2 檔「already noticeable」）。Sierra V2 的 stack=3 是官方**示範**，文件沒寫出廠整數；支援 2026 回覆「沒有正確設定」。ATAS 堆疊檔數出廠值**未公布**。
4. **空檔 / 對 0 做除法會製造假巨幅失衡。** Orderflows 手冊原文：薄量市場裡 `1 vs 5`、`0 vs 4` 經常出現，**不實用**。ATAS 有 `Ignore Zero Values`。Sierra 有 `Enable Zero Bid/Ask Compares`（對 0 要比要嘛忽略，要嘛把 0 當 1，否則比率爆炸）。
5. **單根足跡的價值區：Orderflows 預設 70% of that bar’s volume。** 這是 footprint 棒內統計，不是日盤 Market Profile，不要再拉 Steidlmayer/TPO。
6. **未完成拍賣的定義幾乎統一**：正確高點 = 最高檔 Bid 為 0；正確低點 = 最低檔 Ask 為 0。兩邊都非 0 = 未完成。ATAS 明講：週期越長，未完成越少；**不能當獨立進場工具**。
7. **堆疊區的用法是回測進場，不是當根追價。** ATAS 2018：等價格去 **test**，最好有前一個堆疊當確認。Dale：畫成 S/R 盒子。Valtos（[HiddenTradeLocations.pdf](https://www.orderflows.com/freebies/HiddenTradeLocations.pdf)）：買回踩買堆疊區、賣回踩賣堆疊區；**ES 止損放在區外 1 tick，也可 2–3 tick**；ES 舉例止盈 **5 points**。這是「貼結構的 tick 止損」，不是加密文案那種固定 0.15% 價格百分比，也不能把 1 tick ES 直接換成 SOL 的 0.15%。
8. **所有認真的來源都說：比例要依品種與週期改。** ATAS Learn、Jigsaw、Valtos 2017 文都要求依市場調最小量。沒有人發表過「SOL 1 分鐘用 400%、止損 0.15%」。TradingLite 官方文件本輪已找不到（產品站關閉）。

加密文案裡的鐵板、看 A 打 B，**不在上述一手文獻裡**。Valtos 的窄止損存在，但是 **ES tick + 區外**，見第 3 節。

---

## 1. 流派元素：失衡比率（Imbalance Rate）

比較定義（ATAS Learn 原文原則）：Ask(P) / Bid(P−1) 或反向。≥2x 叫失衡，≥4x 叫強失衡。

| 來源 | 數字 | 性質 | 他們怎麼用 |
|---|---|---|---|
| ATAS 軟體預設 | **150%** | 出廠顯示閾 | 調高才只顯示更強的。說明書例子用 350% 只是算術示範（30 的 ask 要 bid>105） |
| ATAS Learn 課程 | **200% = 失衡；400% = 強失衡 / 壓制** | 官方教材「standard」 | 用來分類格子，不是開倉公式。明寫可依品種與週期改 |
| ATAS 部落格 2018 | 圖上亮綠色 = **買方超過 300%** | 教材範例（ES 5 分鐘） | 單個不算；堆疊才是「炮火」 |
| Orderflows Trader 7（Michael Valtos） | **軟體預設 400%（4 對 1）**；2017 文他本人也寫 I use 400 | 全域，連動堆疊 / 逆失衡 / 多重失衡 | 薄量還要加最小量。Price Rejector 手冊稱 400 為 industry standard，也可改 300 或 1000 |
| Trader Dale | **預設 300%；他寫 I prefer the Default setting (300%)** | 作者本人偏好 | 斜對角；買失衡在 Ask、賣失衡在 Bid |
| Jigsaw 官方 help | **出廠失衡%與最小量未公布**；Tilt Mode **預設 On** | 軟體預設 | 斜對角靠 Tilt |
| Jigsaw 部落格 walkthrough | **200%** + **50 口**（「for this example」） | 示範 | 依品種改 |
| Quantower 官方 help | 比率**未公布**；布局 Bid 左 Ask 右；步長預設 1 tick | 軟體 | |
| Quantower 部落格 | Ratio=3 → 300%；文案寫多數人約 **200–300%**；堆疊舉 3 檔 | 廠商例子，非 default= | |
| Sierra Chart Numbers Bars 出廠 | **`.25, .50, .75`**（即 25/50/75%，**不是** 200/300/400） | 軟體預設 | 官方着色建議另寫 **`1.25, 1.50, 2.0`**（125/150/200%），並說應自己分析買賣量差 |
| Sierra 若要 200/300/400 分色 | 把閾設成 **`2, 3, 4`** | 支援回覆，非出廠 | Range3 = 400%+；只要 300% 高亮用 **`0, 0, 3`**；只要 400% 用官方例 **`0, 0, 4`** |
| Sierra V2 堆疊 | 示範：Difference 1/−1、相鄰 **3** 檔；**0 = 不畫高亮** | 示範，非出廠 | 支援：「沒有正確設定」 |
| TradingView Volume Footprint | **預設 300%（3×）**，所有品種同一套 | 軟體預設 | 非加密校準 |
| Exocharts（加密原生廠商） | 「**250–400 commonly used and recommended**」 | 廠商建議區間，**無 BTC/ETH/SOL 分表** | 本輪最接近加密的公開區間 |

訂策略時請把這個元素拆成兩層，這是專家實際在做的事，不是我們發明的：

- **記錄閾**（圖上要看見）：接近 ATAS 出廠 150% 或 Learn 的 200%，用來觀察。
- **腳本武裝閾**（才允許進 A 腳本的「失衡」）：接近教材強失衡 **300%–400%** 這一帶。Orderflows 偏嚴（400），Dale 偏 300，ATAS 課把 200/400 分級。

SOL/SUI 1 分鐘若用 150% 且 1 tick 一桶，會整片紅——這正是 Orderflows 說的薄量假失衡。先解決桶寬與忽略 0，再談百分比。

---

## 2. 流派元素：最小成交量過濾（Min Volume）

| 來源 | 數字 | 原文要點 |
|---|---|---|
| Orderflows 軟體 | **預設 10 口** | 擋 `1 vs 5`、`0 vs 4` |
| Valtos 本人 2017 | **常用 50**（「I usually use a setting of 50」） | **不要把 10 與 50 混成同一個數**；並寫應依市場改 |
| Orderflows 失衡反轉 | **最少 10 口** | 期貨口數 |
| Jigsaw 部落格示範 | ES **50 口** + 失衡 **200** | 官方 help **沒寫出廠預設**；200/50 是 walkthrough「for this example」 |
| ATAS | 有 Volume Filter、Minimum Difference，**預設值未在說明頁寫死** | 元素存在 |
| Sierra | Minimum Volume Value for Ratio Comparisons，**預設 0**（等於關掉） | 要自己開 |

10 口、50 口是 **ES 合約單位**。SOL/SUI 必須改成「該標的、該所、該 1 分鐘的量分位」，例如相對近端成交量的分位數。專家原則可搬：薄量格子不准進失衡。專家的絕對口數不能搬。

---

## 3. 流派元素：失衡堆疊（Stacked Imbalance）

| 來源 | 連續檔數 | 用法 |
|---|---|---|
| ATAS Learn | **3 檔以上**才叫 stacking；2 檔「already noticeable」；1 檔常是噪音 | 「filter」；檔數越多越像持續發起 |
| ATAS 2018 文 | 「two, three and more」；範例 **3 然後 4** | 堆疊區會被測試，當 S/R；進場要等第二次堆疊 + 回測確認，不要看見第一個堆疊就衝 |
| Orderflows / Valtos | **3 檔**（可調）；多頭須在**綠上漲 K**，空頭在**紅下跌 K**；軟體 **default enabled** | 綠區支撐、紅區壓力；**回踩區內進場**。ES：止損區外 1–3 tick，舉例止盈 5 points。他警告堆疊區會對撞（買堆疊緊接賣堆疊） |
| Orderflows Multiple Imbalance | **3 個以上同向但不整齊相鄰** | **動能**，不是回測 S/R（與 Stacked 不同） |
| Trader Dale | **預設 3** | 自動畫盒子，當強 S/R |
| Sierra 官方堆疊範例 | **Highlight Adjacent Alerts Minimum Size = 3** | 工具範例，不是保證勝率 |

Orderflows 多了一條很多人略過的規則：**堆疊方向要和 K 線顏色一致**（上漲棒裡的買堆疊、下跌棒裡的賣堆疊）。混亂棒裡雙向堆疊，不在他們的「漂亮堆疊」定義裡。

ATAS 2018 的進場句子（這才是運用，不是百分比）：

> 等到圖上出現**第二個**堆疊，而且價格在測試先前的堆疊區，才順勢站隊。

這與我們系統腳本 A「離開再回踩」是同一類用法，不是當根市價追尖端。

---

## 4. 流派元素：忽略零值 / 空桶

這是訂 SOL/SUI 策略時最該先抄的專家規則。

- ATAS：`Ignore Zero Values` — 排除含 0 的比較。
- Sierra：`Enable Zero Bid/Ask Compares` 預設討論是 No 則不算；若 Yes，可把 0 改成 1，或把比率釘成 ±1000%（人工巨幅，極易假訊號）。
- Orderflows：直接用最小量 10 口擋掉 `0 vs 4` 這類。

**專家共識方向：空桶不要當 0 去除。** 我們規格裡「空桶不得當 0 去除」與原廠一致。

---

## 5. 流派元素：未完成拍賣 / Unfinished Business / Failed Auction

定義（ATAS Learn、Trader Dale、Orderflows 一致）：

- 完成的高：最高檔 **Bid = 0**，Ask > 0
- 完成的低：最低檔 **Ask = 0**，Bid > 0
- 未完成：極端檔兩邊都不是 0

用法：

- 當**磁鐵 / 目標**（Dale、ATAS：價格傾向回來補完），不是當必進場。
- ATAS：週期越長，未完成越少。1 分鐘會非常多——這是他們已經警告過的現象。
- ATAS 總結原文大意：不能當獨立決策工具，要放在趨勢、關鍵位、總量裡。
- Orderflows：軟體裡 **Detect Unfinished Business 預設關閉**；可選只在擺動高/低過濾。Valtos 把它當目標/磁鐵，不是進場 trigger。
- Dale：常拿來當持倉目標（例如空單看到下方未完成，目標看到那裡）。

訂策略：1 分鐘 SOL 上，未完成只在「已有當根 VA / 擺動 / 堆疊」的關鍵位才進腳本 G；其餘只記錄。這與 ATAS、Orderflows 預設關閉或過濾是同一精神。

---

## 6. 流派元素：吸收（Absorption）

ATAS Learn 給的是**形態簽名，不是固定合約數**：

- 某一價位量異常大（他們 ES 5 分鐘例子：單檔 1400 口，約鄰檔 15 倍）
- Delta 接近 0，或一邊極強但價格不走
- 價格停住

並明確寫：吸收之後可以守住，也可以被打穿，**不是預測**。

Bookmap Absorption：可設時間窗與最小量；官方說不同品種、不同時段量不同，**手動閾要改**。沒有全球預設口數。Iceberg Resistance 算法有一個技術預設「size threshold default = 1」（偵測靈敏度，不是交易參數）。

訂策略：吸收用「相對鄰檔量的倍數 + 推進失敗 + 位置」，不要用 ES 的 1400 口。

---

## 7. 流派元素：單根足跡的 POC 與價值區（不是日盤 Profile）

只保留足跡棒自己的量價統計。Orderflows Trader 對 **每一根 footprint** 算 POC 與 70% VA。

| 元素 | 來源 | 用法 |
|---|---|---|
| 當根 POC | 該 1m 足跡成交量最大的價位桶 | 棒內最接受的價；Aligned POC = 連續兩根 POC 同價（Orderflows） |
| 當根 VA | Orderflows：**Value Area Percent 預設 70% of the bar’s volume** | 綠棒綠 VA、紅棒紅 VA；看價值有沒有在棒與棒之間遷移 |
| Engulfing VA | 當根足跡 VA 吞掉前一根 VA | Orderflows 獨立訊號 |
| Prominent POC | OFT 有此工具，**沒有公開百分比公式** | 當 S/R 著色；不引入 Naked POC |

**已刪除（非本派）：** 日盤 TPO 價值區、IB、Naked/Virgin POC 磁鐵、poor high/low。

Dale 若把進場掛在厚量區邊緣，那是在讀足跡/水平量，可以留；不要再引入他的全日 Volume Profile 磁鐵敘事。

---

## 8. （已刪）VWAP 不是足跡圖流派

σ 帶、AVWAP、日 VWAP 已移出本文件。位置用堆疊區與當根 POC，不用 VWAP。

---

## 9. 流派元素：Delta、CVD、極端主動性

| 來源 | 數字 | 用法 |
|---|---|---|
| Orderflows Extreme Delta/Volume | **預設 25%**（棒 delta / 棒量）超過則標極端主動 | 分正常 vs 極端攻擊 |
| Orderflows Ratio | **≥ 30** 視為價格耗竭；**0–0.69** 視為防守 | 專有比率，不是通用 CVD |
| Trader Dale CVD | 無固定閾；**在 1 分鐘、而且要在強 S/R 附近**看價與 CVD 背離 | 用法鎖在關鍵位，不是全圖背離 |
| ATAS Learn 棒 delta% | **<5%** 平衡；**10%+** 發起；5–10% 灰區 | 與 Orderflows 的 25% 極端不是同一層：5/10 是讀單根，25% 是 OFT 著色極端 |
| ATAS Learn 吸收 | 課內**兩種簽名並存**：takeaway/quiz 用 **delta≈0 + 量異常 + 價格不走**；同頁 ES 例 Ask 1285 / Bid 115 卻是**單邊極強但不走** | 不要把吸收壓成單一公式 |

沒有專家把「CVD 背離」寫成單獨高勝率進場。Dale 明確限制在 S/R 附近的 1 分鐘。

---

## 10. 流派元素：擺動、衰竭印、尾巴、逆失衡

| 元素 | 來源數字 | 用法 |
|---|---|---|
| Swing Period | Orderflows **預設 5** | 找擺動高/低 |
| Exhaustion Prints | Valtos 免費小冊：極端是 **個位數成交 1–9**（不必是 1）；出現在下跌棒頂 offer、上漲棒底 bid | 等 K 線收完、下一根再考慮反轉；止損他寫區外 1 tick。這是 ES 小冊，不是 SOL 張數 |
| OFT Delta Divergence | 新高配負 delta / 新低配正 delta；**預設只在當日高/低啟用** | 反轉確認，無公開「delta 要大於幾」 |
| OFT 最佳化對象 | 手冊：**預設為 1-minute ES**（也寫對多數流動市場可用） | 解釋為什麼 10 口、400%、3 檔是 ES 校準，不是加密校準 |
| Inverse Imbalance | 與堆疊相反的「被困」結構 | 回來陷阱位看反應 |
| Tails / Excess | 足跡棒極端的厚量拒絕 vs 薄量滑針（ATAS excess）；Valtos 個位數 print | 拒絕。不引用 TPO 尾巴規則 |
| Bookmap 牆 | 無通用「多大算牆」 | 依品種深度；靠近時補/撤才有意義 |

---

## 11. 流派元素：盤口（Jigsaw / Bookmap）

Jigsaw Depth & Sales：深度檔數跟數據源（ES 常見 10 檔）。可把「大於某口數」的牆高亮，範例是 **50 口** 的 ES 深度，不是加密預設。

核心用法（Jigsaw 部落格）：靜態大牆在遠方**沒有意義**；要看靠近時是補還是撤。這與我們「靜態牆當布景」一致，沒有新的神聖數字。

---

## 12. 專家範例用的週期（請注意：不是 SOL 聖經）

ATAS 2018 失衡文的圖是 **ES 5 分鐘**。  
Dale 看 CVD 背離用 **1 分鐘，但綁 S/R**。  
ATAS 未完成拍賣：週期越長越少。

ATAS 自己的 BTC 足跡示範是 **60 分鐘、Scale=200**（若 tick $0.10 約每列 $20），不是 1 分鐘配方。Bookmap 只寫過 BTC 約 $10,000 時 tick 建議 1–10（過時價位）。Exocharts 有依週期加粗 tick 的亂區間，且頁面自相矛盾；**沒有 SOL、沒有 SUI**。

所以「主時鐘 1 分鐘」可以跟 Dale 的觀察層、以及 Orderflows「預設給 1m ES」對上，但 ATAS 拿來教堆疊 S/R 與 BTC 示範往往更粗。訂策略時合理的專家折衷是：

- 1 分鐘：確認、失效、CVD、未完成（過濾後）
- 由 1 分鐘滾 5/15/60 根：堆疊區是否還算「被接受的位置」
- 桶寬：加密只能用「活躍時段 3 檔堆疊可數、而非每根都有」來校，不能抄 ATAS 60m BTC 的 Scale 200

---

## 13. 找不到、因此不准假裝找到的東西

以下在本輪專家文獻中 **沒有** 可引用的公開數字：

- SOL 或 SUI 分品種的失衡%、堆疊檔數、桶寬（Exocharts 連 BTC/ETH 都沒拆表）
- ATAS / Sierra 的堆疊檔數**出廠整數**（未公布）
- 「350% 是 BTC 黃金標準」類聯盟文（無方法、無樣本）
- 已移出本派的東西（Market Profile、VWAP、Naked POC、IB）不再找參數
- 「連三檔 400% = 未來幾小時鐵板」
- 跨所 25–35ms 當訂單流參數
- 吸收的全球口數、冰山的全球口數
- Prominent POC 的精確百分比公式（手冊沒寫死）

加密專業平台（TradingLite 等）本輪沒有找到與 ATAS/Orderflows 同級、可引用的 SOL 參數表。若之後兩週肉眼觀測，那是**我們自己的校準**，不要掛專家的名。

---

## 14. 未來 2–3 週訂策略：建議怎麼「運用」這些元素（仍不寫代碼）

目標不是選一個 400% 當信仰，而是建一套**分層**，與專家實際做法同構。

**層 A — 顯示 / 記錄（寬）**  
失衡 150%–200%，堆疊 2 檔也記，未完成全記，VA 70%。用來建立盤感。對應 ATAS 出廠與 Learn 的弱閾。

**層 B — 結構候選（中）**  
失衡 **300%**（Dale 本人偏好）或分級：200 弱 / 400 強（ATAS Learn）。堆疊 **≥3** 且方向與棒向一致（Orderflows）。忽略 0。最小量用 SOL/SUI 自己的分位，不用 10 口。堆疊區只在價格離開後標記為「待回測」。

**層 C — 允許進腳本（嚴）**  
必須同時有：層 B 的區 + 位置（70% VA 外側或擺動沿，Orderflows swing 5 可當起點）+ 回測當根的拒絕/吸收簽名 + 非清算。這是 ATAS 2018「等第二個堆疊、等 test」的句子，寫成檢查表。

**層 D — 明確降權**  
1 分鐘未完成除非落在已有堆疊區或近根 POC 上（Orderflows 預設連偵測都關）。單因子不開倉（ATAS Learn：one element without context is not a signal）。

**校準時怎麼判斷數字該鬆該緊（專家方法，不是亂搜）**

1. 固定定義（斜對角、忽略 0、單根 VA 70%、完成拍賣的 0）。這些不要校。
2. 只校「依品種而變」的：桶寬、最小量分位、失衡 300 vs 400、堆疊 3 vs 4。
3. 觀察指標用專家自己的語言：若 1 分鐘上「單個失衡滿屏」，閾太鬆（ATAS：單個無意義）。若整天沒有一個 3 檔堆疊，桶太細或比率太嚴。
4. SOL 與 SUI **分開看**，因為最小量與空桶率會差一截；專家已要求依 symbol 調。
5. 不要用盈虧曲線選參數。先看：回測堆疊區時，當根有沒有對向拒絕；沒有拒絕還當鐵板的次數（ATAS 說會被 test，沒說會守住）。

---

## 15. 可直接寫進參數檔的「有出處起點」（觀察用，不是實盤承諾）

這些是把專家預設翻譯成我們的佔位，**實盤仍禁止**，只供兩週觀測對照：

- `IMBALANCE_RATE_RECORD` ← ATAS 預設 150 或 Learn 200
- `IMBALANCE_RATE_STRONG` ← ATAS Learn 400；Dale 偏好 300（建議兩週都記，比較哪個在 SOL 上不至於滿屏）
- `STACK_MIN_LEVELS` ← 3（Orderflows / Dale **預設**；ATAS Learn 定義；Sierra **示範**。ATAS/Sierra 出廠檔數未公布）
- `VALUE_AREA_PCT` ← 70（Orderflows **單根足跡棒**，不是日盤 Profile）
- `VALUE_AREA_SCOPE` ← bar（禁止做成 TPO/IB）
- `STACK_REQUIRE_BAR_DIRECTION` ← true（Orderflows：漲棒買堆疊、跌棒賣堆疊）
- `IGNORE_ZERO_COMPARE` ← true（ATAS / Sierra / Orderflows 薄量警告）
- `SWING_N` ← 5（Orderflows Swing Period 預設）
- `DELTA_VOL_EXTREME` ← 0.25（Orderflows Extreme Delta/Volume）
- `UNFINISHED_DEFAULT_ENABLED` ← false 作為開倉；true 僅記錄（Orderflows 預設關閉偵測）
- `MIN_IMBALANCE_VOLUME` ← 不填 ES 的 10；改為「該標的 1 分鐘單桶量的分位」，兩週內用資料估
- Orderflows 手冊還寫：預設是給 **1 分鐘 ES** 優化的。兩週觀測若覺 400%+3 檔在 SOL 上太密或太稀，先動桶寬與最小量分位，不要先怪學派元素本身

桶寬：專家沒給 SOL。方法論來自「不要 1 tick 假堆疊」——用能讓 3 檔堆疊在活躍時段每天出現可數次數、而不是每根都有的粗細。這要靠觀測，不能靠名人語錄。

---

## 16. 這一小時研究的誠實邊界

已核對：只收足跡圖原廠與教育者（ATAS、Valtos/Orderflows、Dale 的 Order Flow 手冊、Sierra Numbers Bars、Jigsaw FootPrint）。ATAS 出廠 **150%** 與 Learn **200/400**、部落格 **300** 不要合成一個數。Sierra 出廠着色閾是 **.25/.50/.75**。TradingView 足跡預設 **300%**。Exocharts 250–400 僅作加密常見區間，無 SOL 表。**已刪除** Market Profile、VWAP、Naked POC、IB。Valtos 窄止損仍是 **ES 堆疊區外 1–3 tick**。

未完成：付費課程錄影裡可能還有口頭參數；本輪只用公開手冊與官網。若你之後兩週要「學專家」，正確動作是：用層 A–C 在 TradingLite/ATAS 上看 SOL 1 分鐘，用他們的句子做筆記，而不是再找一個 Telegram 400%。
