.PHONY: test shadow live-deny native fmt replay replay-three book-replay

shadow:
	cargo run -p orderflowd -- --mode shadow --once
	PYTHONPATH=python python3 -m orderflow --mode shadow --once

live-deny:
	- cargo run -p orderflowd -- --mode live --once
	- PYTHONPATH=python python3 -m orderflow --mode live --once

replay:
	cargo run -p orderflowd -- --mode shadow --replay /tmp/sol_okx_trades.jsonl --max-trades 5000 --journal /tmp/sol_bars_closed.jsonl

replay-three:
	cargo run -p orderflowd -- --mode shadow \
		--replay /tmp/sol_okx_trades.jsonl \
		--replay-binance /tmp/sol_binance_agg.jsonl \
		--replay-bybit /tmp/sol_bybit_trades.csv \
		--max-trades 5000

book-replay:
	cargo run -p orderflowd -- --mode shadow \
		--book-replay crates/orderflow-book/tests/fixtures/sol_okx_books.jsonl


test:
	cargo test --workspace --exclude orderflow-py
	PYTHONPATH=python python3 -m unittest tests.test_live_gate tests.test_scripts_stage5 tests.test_reconcile_stage6 tests.test_gateway_stage7 tests.test_ops_stage8

fmt:
	cargo fmt --all

native:
	cargo build -p orderflow-py --features extension-module
	python3 -c "import pathlib,shutil,sysconfig; dest=pathlib.Path('python')/'orderflow_native'+sysconfig.get_config_var('EXT_SUFFIX'); shutil.copy('target/debug/liborderflow_native.so', dest); print(dest)"
