# orderflow-py

PyO3 橋。階段 0 只匯出版本、`live_allowed() -> false`、以及假的凍結 1m 快照型別。

從 Python 載入（需先編譯 extension）：

```bash
pip install maturin
maturin develop --manifest-path crates/orderflow-py/pyproject.toml --features extension-module
python3 -c "import orderflow_native; print(orderflow_native.live_allowed())"
```

沒有行情解析。句子層請用 `python/orderflow`。
