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
| [params/sample-size-verdict.md](params/sample-size-verdict.md) | 跟數：59 根太少；1499 根（含美盤）仍不選 300 vs 400 |
| [params/sol-observation.md](params/sol-observation.md) | SOL 每日觀察日誌 |
| [params/sui-observation.md](params/sui-observation.md) | SUI 每日觀察日誌 |
| [scripts/rebuild_sol_footprint_stats.py](scripts/rebuild_sol_footprint_stats.py) | 用 OKX 公共成交自組 1m 足跡、重算校準表（不是 ATAS） |
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
21 天結束後才談影子程式，不談 live。
