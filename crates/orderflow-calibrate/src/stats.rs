//! Shadow stats on Rust frozen closed-1m snapshots. Never rebuild a matrix.
//! Never pick 300 vs 400.

use std::fs;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ShadowStats {
    pub event: &'static str,
    pub bars: u64,
    pub dale_aligned: u64,
    pub valtos_aligned: u64,
    pub record_aligned: u64,
    pub unfinished: u64,
    pub chaos: u64,
    pub chosen_armed_rate: Option<&'static str>,
    pub still_open: bool,
    pub copied_price_onto_okx: bool,
    pub note: &'static str,
}

pub fn summarize_journal(path: &Path) -> Result<ShadowStats, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut s = ShadowStats {
        event: "calibrate_stats",
        bars: 0,
        dale_aligned: 0,
        valtos_aligned: 0,
        record_aligned: 0,
        unfinished: 0,
        chaos: 0,
        chosen_armed_rate: None,
        still_open: true,
        copied_price_onto_okx: false,
        note: "stage 9: shadow stats only; do not select 300 vs 400; live still gated",
    };
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        if v.get("event").and_then(|x| x.as_str()) != Some("footprint_closed") {
            continue;
        }
        let fp = v.get("footprint").unwrap_or(&v);
        s.bars += 1;
        if fp
            .pointer("/dale/aligned")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            s.dale_aligned += 1;
        }
        if fp
            .pointer("/valtos/aligned")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            s.valtos_aligned += 1;
        }
        if fp
            .pointer("/record/aligned")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            s.record_aligned += 1;
        }
        if fp
            .get("unfinished_high")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
            || fp
                .get("unfinished_low")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        {
            s.unfinished += 1;
        }
        if fp.get("chaos").and_then(|x| x.as_bool()).unwrap_or(false) {
            s.chaos += 1;
        }
        if fp
            .get("unfinished_is_entry")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            return Err("unfinished_is_entry must stay false".into());
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fixture_counts_both_rates_and_does_not_choose() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shadow_stats.jsonl");
        let s = summarize_journal(&path).unwrap();
        assert_eq!(s.bars, 2);
        assert_eq!(s.dale_aligned, 1);
        assert_eq!(s.valtos_aligned, 1);
        assert_eq!(s.unfinished, 1);
        assert!(s.chosen_armed_rate.is_none());
        assert!(s.still_open);
        assert!(!s.copied_price_onto_okx);
    }
}
