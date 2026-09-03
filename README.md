# 足跡圖一分鐘策略流

死守**足跡圖流派**的 SOL / SUI 1 分鐘訂單流規格與參數校準。  
不是形態學、不是 Market Profile、不是 VWAP、不是 ICT。

目前這個 repo 有規格、觀察日誌，以及**階段 0 骨架**：`orderflowd` / `python -m orderflow` 能以 `shadow` 啟動，並硬拒絕 live。還沒有 WS、足跡熱路徑、也沒有下單。**交易系統尚未運行。**

## 啟動（階段 0）

```bash
make test
# 或分開（Rust 1.85+、Python 3.12+）：
cargo test --workspace --exclude orderflow-py
cargo run -p orderflowd -- --mode shadow --once
PYTHONPATH=python python3 -m orderflow --mode shadow --once
cargo run -p orderflowd -- --mode live --once    # 必須失敗：params_not_calibrated
```

配置：[`params/runtime.toml`](params/runtime.toml)、[`params/sol.toml`](params/sol.toml)、[`params/sui.toml`](params/sui.toml)。數字只從 toml 讀，程式裡不准寫死 400%。300∥400 仍是 `parallel`。`calibration_complete = false`，`live_enabled = false`。觀察稿不是樣本外驗證。

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
| [params/day15-sui-bucket.md](params/day15-sui-bucket.md) | 第 15 天：SUI 桶寬 0.0001；禁止抄 SOL 0.01 |
| [params/day16-sui-min-volume.md](params/day16-sui-min-volume.md) | 第 16 天：SUI 時段 p25；300∥400 仍並列 |
| [params/day17-sui-script-a.md](params/day17-sui-script-a.md) | 第 17 天：SUI 驗證 A；打穿約 23% |
| [params/day18-sol-liq-oi-veto.md](params/day18-sol-liq-oi-veto.md) | 第 18 天：OI −2% / 強平 p95 否決 |
| [params/day19-sol-session-funding.md](params/day19-sol-session-funding.md) | 第 19 天：極薄 vs 美盤；資金費 ±15m 黑窗 |
| [params/day20-sol-three-venue.md](params/day20-sol-three-venue.md) | 第 20 天：三所只比方向；共振 `off` |
| [params/sol.toml](params/sol.toml) | SOL 觀察稿數字（程式讀這個，不是 day markdown） |
| [params/sui.toml](params/sui.toml) | SUI 分表 |
| [params/runtime.toml](params/runtime.toml) | 模式、三所 endpoint、live 閘門、共振 `off` |
| [crates/orderflowd](crates/orderflowd) | 階段 0 行程：shadow 可開、live 拒絕 |
| [python/orderflow](python/orderflow) | 句子層套件（階段 0 只轉呼叫 orderflowd） |
| [params/sample-size-verdict.md](params/sample-size-verdict.md) | 跟數：59 根作廢；1790 根仍不選 300 vs 400；週 2 用 3060 根只數失敗畫面 |
| [params/sol-observation.md](params/sol-observation.md) | SOL 每日觀察日誌 |
| [params/sui-observation.md](params/sui-observation.md) | SUI 每日觀察日誌 |
| [scripts/rebuild_sol_footprint_stats.py](scripts/rebuild_sol_footprint_stats.py) | 用 OKX 公共成交自組 1m 足跡 |
| [scripts/recompute_days_2_7.py](scripts/recompute_days_2_7.py) | 一次重跑第 2–7 天校準表 |
| [scripts/week2_scripts_a_g.py](scripts/week2_scripts_a_g.py) | 週 2：劇本 A–G 失敗畫面（F 因無 L2 標 not_evaluated） |
| [scripts/week3_sui_regime.py](scripts/week3_sui_regime.py) | 週 3：SUI 分表 + 清算/黑窗 + 三所方向 |
| [scripts/fetch_week3_inputs.py](scripts/fetch_week3_inputs.py) | 週 3 輸入：SUI 成交/K 線、OI、強平、Binance/Bybit 日檔 |
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

第一週 SOL 眼睛已凍結（300∥400 仍並列）。第二週句子層已凍結。第三週 SUI 分表與制度否決已凍結。  
**階段 0 骨架已進 repo**：shadow 可啟動，live 因觀察稿未樣本外驗證而拒絕。下一框是階段 1（OKX 公共成交 + 1m 棒契約）。**禁止 live。**
