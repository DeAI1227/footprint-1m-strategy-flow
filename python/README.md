# Python 句子層

階段 0 套件。讀 `params/*.toml`，以 `shadow` 啟動，JSON 日誌不含密鑰。

```bash
PYTHONPATH=python python3 -m orderflow --mode shadow --once
PYTHONPATH=python python3 -m orderflow --mode live --once   # 退出碼 2
```

禁止在這裡解析行情 WebSocket，禁止用 pandas 組生產足跡矩陣。
