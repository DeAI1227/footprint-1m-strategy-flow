# SOL 腳本 F：第一次真 L2（不是 live）

日期：**2026-09-11**  
這不是 09-07→09-09 樣本外窗。OKX 公共 REST **沒有歷史 books**。本窗是合完 #10–#13 之後，用公共 WS 往前錄的執行所盤口。

路徑：`scripts/capture_okx_l2.py`（trades + incremental `books`）→ `/tmp` JSONL → `orderflowd --replay --book-replay` → 凍結 `book_closed` → `scripts/oos_scripts_from_journal.py`。不准用成交量堆腦補牆。

---

## 覆蓋

| | |
|---|---|
| 窗 | **2026-09-11 05:11 → 06:00 UTC**（亞盤；forming 06:00 不計） |
| 成交 | **4094** 筆 OKX `SOL-USDT-SWAP` 公共 WS |
| books | **28121** 幀；**1** 次 snapshot；檔內 seq 缺口 **0** |
| 已收盤 1m | **49**；`book_closed` **49**；`okx_book_ok=true` |
| 品質 | `gap_minutes=0` `late_trade=0` `copied_price_onto_okx=false` |
| 結束原因 | 公共 WS 要文字 `ping`（協定 ping 不夠）。連線在 06:00 被踢。腳本已補應用層 ping，本窗不重接以免插入第二張 snapshot |

JSONL 留 `/tmp`。這 49 根**不夠**凍結 F，只夠證明語言能算。

---

## F 四種對讀（引擎已閉合棒）

全書健康：**49 / 49 `book_ok`**。`script_f=computed`（不再是整本 `not_evaluated`）。

| | 任意 3×中位牆 | 牆在當根 POC 或堆疊上 |
|---|---|---|
| 根數 | 49 | **2** |
| 真吃牆 | 0 | 0 |
| 讓路 | 0 | 0 |
| 吸收 | 2 | **1** |
| 假牆 | 43 | **1** |
| 無牆 / 不算 | 4 | — |

任意大牆幾乎每根都假牆：這段亞盤有一檔很厚的 bid（約 99.60、九千～一萬二 SOL）在價下反覆減補，分類器把 pull+replenish 標成假牆。那是做市報價噪音，**不是 F 的主詞。**

派內是**牆與足跡對讀**。對上 POC / 堆疊的只有 **2** 根（吸收 1、假牆 1）。真吃牆與讓路這窗都沒出現。

讓路仍是否決（本窗 0 次）。F **不當進場確認**。不准因為假牆很多就改 `wall_mult`、不准把成交量堆當成牆。

---

## 這窗不當什麼

- 不當 300 vs 400 的樣本（武裝堆疊只有 2 / 1，都沒離開）
- 不當 `out_of_sample_validated`
- 不當 live
- 不當「假牆 88% 所以 F 能賺」

第 13 天沒有 book 所以整本 `not_evaluated`。今天有健康 L2，F 能算，但樣本太短、對上足跡的牆太少，**定義不改。**

禁止項自檢：沒有腦補牆；沒有開 live；沒有選邊；沒有把 09-07 樣本外 journal 改寫進去。
