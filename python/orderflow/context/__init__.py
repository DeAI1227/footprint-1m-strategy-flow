# Near-window volume stacks / swings. Input must be Rust closed 1m bars.
# No VWAP, no session Market Profile / TPO, no Naked POC.

WIRED = True
SOURCE = "rust_closed_1m"
FORBIDDEN = ("vwap", "avwap", "tpo", "market_profile", "naked_poc")
