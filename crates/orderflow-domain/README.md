# orderflow-domain

共享型別、`params/*.toml` 載入、live 閘門。不含 WS、不含足跡矩陣、不含下單。

階段 0 結束條件：`shadow` 可啟動；`live` / `live_small` 因 **參數未校準**（`calibration_complete = false`）被拒絕。
