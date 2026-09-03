# 足跡圖一分鐘策略流

死守**足跡圖流派**的 SOL / SUI 1 分鐘訂單流規格與參數校準。  
不是形態學、不是 Market Profile、不是 VWAP、不是 ICT。

目前這個 repo 只放規格與觀察日誌。參數沒校完之前，不寫實盤、不開 live。

## 文件

| 路徑 | 用途 |
|---|---|
| [specs/orderflow-footprint-school-guide.md](specs/orderflow-footprint-school-guide.md) | 流派本身：語言、讀圖、誤用 |
| [specs/orderflow-1m-tokyo-system-elements.md](specs/orderflow-1m-tokyo-system-elements.md) | 系統元素與契約（東京、1m、三所、腳本 A–G） |
| [specs/orderflow-rust-python-boundary.md](specs/orderflow-rust-python-boundary.md) | Rust 熱路徑 / Python 句子層；Binance、OKX、Bybit |
| [specs/school-elements-expert-parameters.md](specs/school-elements-expert-parameters.md) | 專家出廠與教材數字（有出處，不編 SOL 常數） |
| [specs/footprint-param-calibration-21d.md](specs/footprint-param-calibration-21d.md) | **21 天、每天 1 小時**的參數校準時程 |
| [params/day1-locked-setup.md](params/day1-locked-setup.md) | 第 1 天：大佬影片/教材鎖定的看圖定義 |
| [params/day2-sol-bucket.md](params/day2-sol-bucket.md) | 第 2 天：SOL 桶寬，觀察用 0.01 |
| [params/day3-sol-min-volume.md](params/day3-sol-min-volume.md) | 第 3 天：SOL 忽略 0 + 最小量 p25 |
| [params/day4-sol-imbalance-rate.md](params/day4-sol-imbalance-rate.md) | 第 4 天：200 記錄；300∥400 武裝並列 |
| [params/day5-sol-stack-direction.md](params/day5-sol-stack-direction.md) | 第 5 天：堆疊 3、棒向一致 |
| [params/day6-sol-poc-va.md](params/day6-sol-poc-va.md) | 第 6 天：當根 POC / 70% VA |
| [params/day7-sol-unfinished-week1-freeze.md](params/day7-sol-unfinished-week1-freeze.md) | 第 7 天：未完成密度 + 第一週凍結 |
| [params/day8-sol-script-a.md](params/day8-sol-script-a.md) | 第 8 天：腳本 A；`LEAVE_BARS=1` |
| [params/day9-sol-script-b.md](params/day9-sol-script-b.md) | 第 9 天：腳本 B；關鍵位第二次打穿約一半 |
| [params/day10-sol-script-c.md](params/day10-sol-script-c.md) | 第 10 天：腳本 C；`TRAP_BARS=3` |
| [params/day11-sol-script-d.md](params/day11-sol-script-d.md) | 第 11 天：腳本 D；接受後回踩仍會打穿 |
| [params/day12-sol-script-e.md](params/day12-sol-script-e.md) | 第 12 天：腳本 E；第一次 CVD 背離不反手 |
| [params/day13-sol-script-f.md](params/day13-sol-script-f.md) | 第 13 天：腳本 F；無 L2 → `not_evaluated` |
| [params/day14-sol-week2-freeze.md](params/day14-sol-week2-freeze.md) | 第 14 天：腳本 G + 第二週凍結 |
| [params/sample-size-verdict.md](params/sample-size-verdict.md) | 跟數：59 根作廢；1790 根仍不選 300 vs 400；週 2 用 3060 根只數失敗畫面 |
| [params/sol-observation.md](params/sol-observation.md) | SOL 每日觀察日誌 |
| [params/sui-observation.md](params/sui-observation.md) | SUI 每日觀察日誌 |
| [scripts/rebuild_sol_footprint_stats.py](scripts/rebuild_sol_footprint_stats.py) | 用 OKX 公共成交自組 1m 足跡 |
| [scripts/recompute_days_2_7.py](scripts/recompute_days_2_7.py) | 一次重跑第 2–7 天校準表 |
| [scripts/week2_scripts_a_g.py](scripts/week2_scripts_a_g.py) | 週 2：劇本 A–G 失敗畫面（F 因無 L2 標 not_evaluated） |
| [docs/implementation-plan.md](docs/implementation-plan.md) | 之後寫程式的階段計劃（參數沒填完禁止 live） |

## 派內眼睛（校準只動這些）

斜對角失衡、忽略空桶、最小量、堆疊、當根 POC / 當根 VA、未完成拍賣、delta / CVD、吸收與 excess、tape、DOM 對讀。  
位置：堆疊區、當根 POC、近窗量堆、擺動。  
制度過濾（清算、OI、時段、資金費）只否決，不當進場主詞。

## 明確不在本派

Market Profile / TPO、VWAP / AVWAP、Naked POC、Kill Zone / IPDA、布林 / 肯特納擠壓、看 A 打 B、整數關當主詞。

## 宇宙與時鐘

- 標的：SOL 主樣本，SUI 驗證；參數表分開
- 時鐘：交易所事件時間 1 分鐘 `[t, t+60)`，只對 **closed** 棒做句子
- 行情：Binance、OKX、Bybit 公共 WS 全接；預設 OKX 執行、用 OKX 自己的足跡

## 接下來

按 [21 天時程](specs/footprint-param-calibration-21d.md) 填兩張觀察日誌。  
第一週 SOL 眼睛已凍結（300∥400 仍並列）。第二週句子層已凍結（`LEAVE_BARS=1`、`TRAP_BARS=3`、F `not_evaluated`、G 仍不開倉）。第三週才開 SUI 分表與制度否決。21 天結束後才談影子程式，不談 live。
