.PHONY: test shadow live-deny native fmt replay

shadow:
	cargo run -p orderflowd -- --mode shadow --once
	PYTHONPATH=python python3 -m orderflow --mode shadow --once

live-deny:
	- cargo run -p orderflowd -- --mode live --once
	- PYTHONPATH=python python3 -m orderflow --mode live --once

replay:
	cargo run -p orderflowd -- --mode shadow --replay /tmp/sol_okx_trades.jsonl --max-trades 5000 --journal /tmp/sol_bars_closed.jsonl

test:
	cargo test --workspace --exclude orderflow-py
	PYTHONPATH=python python3 -m unittest tests.test_live_gate

fmt:
	cargo fmt --all

native:
	cargo build -p orderflow-py --features extension-module
	python3 -c "import pathlib,shutil,sysconfig; dest=pathlib.Path('python')/'orderflow_native'+sysconfig.get_config_var('EXT_SUFFIX'); shutil.copy('target/debug/liborderflow_native.so', dest); print(dest)"
