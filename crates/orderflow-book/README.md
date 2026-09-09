# orderflow-book

階段 3：分所 L2。書壞則**該所**盤口特徵 `not_evaluated`。執行所（OKX）書壞才關 DOM 依賴開倉；共振所書壞只把該所對讀標 `not_evaluated`，不擋 OKX 足跡、也不把外所價寫進 OKX。

## 完整性（不要混所規）

- **OKX**：`seqId` / `prevSeqId`。snapshot 的 `prevSeqId = -1`。生產 checksum 自 2026-06-23 起固定為 `0`，必須忽略；非 0 才對更新後的前 25 檔做 CRC32，對不上就清空重建。
- **Binance USD-M**：REST snapshot 的 `lastUpdateId`；之後 delta 用 `pu ==` 上一筆 `u`。不要用現貨的 `U == last+1`。
- **Bybit**：`type=snapshot` 重置；`type=delta` 套用；size 0 刪檔；`u==1` 整本覆蓋；序號不是 last 或 last+1 就 Bad。

三所簿互不中毒。SUI tick 是 `0.0001`，不抄 SOL `0.01`。

## 四種對讀（每根已閉合 1m）

真吃牆 / 讓路 / 吸收 / 假牆。沒有牆或書在 Bad/Rebuilding → `not_evaluated`。Python 腳本 F 狀態機仍是階段 5，維持 `not_evaluated`。
