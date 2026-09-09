//! Shadow stats on Rust frozen closed-1m snapshots. Never rebuild a matrix.
//! Never pick 300 vs 400.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

const STACK_MIN: u32 = 3;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionRow {
    pub session: String,
    pub bars: u64,
    pub dale_aligned: u64,
    pub valtos_aligned: u64,
    pub record_aligned: u64,
    pub dale_stack3: u64,
    pub valtos_stack3: u64,
    pub record_stack3: u64,
    pub unfinished: u64,
    pub chaos: u64,
    /// Batch p25 of frozen nonempty single-side cells in this journal slice.
    /// Not a frozen SOL lot. Engine aligned flags already used rolling session p25.
    pub nonempty_side_p25: Option<f64>,
    pub nonempty_side_cells: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShadowStats {
    pub event: &'static str,
    pub bars: u64,
    pub dale_aligned: u64,
    pub valtos_aligned: u64,
    pub record_aligned: u64,
    pub dale_stack3: u64,
    pub valtos_stack3: u64,
    pub record_stack3: u64,
    pub unfinished: u64,
    pub chaos: u64,
    pub sessions: Vec<SessionRow>,
    pub nonempty_side_p25: Option<f64>,
    pub chosen_armed_rate: Option<&'static str>,
    pub still_open: bool,
    pub out_of_sample_validated: bool,
    pub copied_price_onto_okx: bool,
    pub note: &'static str,
}

fn stack3(slice: &serde_json::Value) -> bool {
    let buy = slice.get("buy_stack").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let sell = slice.get("sell_stack").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    buy >= STACK_MIN || sell >= STACK_MIN
}

fn aligned(slice: &serde_json::Value) -> bool {
    slice.get("aligned").and_then(|x| x.as_bool()).unwrap_or(false)
}

fn percentile(xs: &[f64], p: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut ys = xs.to_vec();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if p <= 0.0 {
        return Some(ys[0]);
    }
    if p >= 100.0 {
        return Some(ys[ys.len() - 1]);
    }
    let k = (ys.len() - 1) as f64 * (p / 100.0);
    let f = k.floor() as usize;
    let c = k.ceil() as usize;
    if f == c {
        Some(ys[f])
    } else {
        Some(ys[f] * (c as f64 - k) + ys[c] * (k - f as f64))
    }
}

fn collect_side_vols(fp: &serde_json::Value, into: &mut Vec<f64>) {
    let Some(cells) = fp.get("cells").and_then(|x| x.as_array()) else {
        return;
    };
    for c in cells {
        if let Some(v) = c.get("bid").and_then(|x| x.as_f64()) {
            if v > 0.0 {
                into.push(v);
            }
        }
        if let Some(v) = c.get("ask").and_then(|x| x.as_f64()) {
            if v > 0.0 {
                into.push(v);
            }
        }
    }
}

fn session_name(fp: &serde_json::Value) -> String {
    fp.get("session")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn finish_row(name: String, mut row: SessionRow, vols: &[f64]) -> SessionRow {
    row.session = name;
    row.nonempty_side_cells = vols.len() as u64;
    row.nonempty_side_p25 = percentile(vols, 25.0);
    row
}

pub fn summarize_journal(path: &Path) -> Result<ShadowStats, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut s = ShadowStats {
        event: "calibrate_stats",
        bars: 0,
        dale_aligned: 0,
        valtos_aligned: 0,
        record_aligned: 0,
        dale_stack3: 0,
        valtos_stack3: 0,
        record_stack3: 0,
        unfinished: 0,
        chaos: 0,
        sessions: Vec::new(),
        nonempty_side_p25: None,
        chosen_armed_rate: None,
        still_open: true,
        out_of_sample_validated: false,
        copied_price_onto_okx: false,
        note: "stage 9: shadow stats only; do not select 300 vs 400; live still gated",
    };
    let mut by_sess: BTreeMap<String, SessionRow> = BTreeMap::new();
    let mut vols_all: Vec<f64> = Vec::new();
    let mut vols_sess: BTreeMap<String, Vec<f64>> = BTreeMap::new();

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
        if fp
            .get("unfinished_is_entry")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            return Err("unfinished_is_entry must stay false".into());
        }

        let sess = session_name(fp);
        let row = by_sess.entry(sess.clone()).or_default();
        s.bars += 1;
        row.bars += 1;

        let dale = fp.get("dale").cloned().unwrap_or(serde_json::Value::Null);
        let valtos = fp.get("valtos").cloned().unwrap_or(serde_json::Value::Null);
        let record = fp.get("record").cloned().unwrap_or(serde_json::Value::Null);

        if aligned(&dale) {
            s.dale_aligned += 1;
            row.dale_aligned += 1;
        }
        if aligned(&valtos) {
            s.valtos_aligned += 1;
            row.valtos_aligned += 1;
        }
        if aligned(&record) {
            s.record_aligned += 1;
            row.record_aligned += 1;
        }
        if stack3(&dale) {
            s.dale_stack3 += 1;
            row.dale_stack3 += 1;
        }
        if stack3(&valtos) {
            s.valtos_stack3 += 1;
            row.valtos_stack3 += 1;
        }
        if stack3(&record) {
            s.record_stack3 += 1;
            row.record_stack3 += 1;
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
            row.unfinished += 1;
        }
        if fp.get("chaos").and_then(|x| x.as_bool()).unwrap_or(false) {
            s.chaos += 1;
            row.chaos += 1;
        }

        collect_side_vols(fp, &mut vols_all);
        collect_side_vols(fp, vols_sess.entry(sess).or_default());
    }

    s.nonempty_side_p25 = percentile(&vols_all, 25.0);
    s.sessions = by_sess
        .into_iter()
        .map(|(name, row)| {
            let vols = vols_sess.get(&name).cloned().unwrap_or_default();
            finish_row(name, row, &vols)
        })
        .collect();
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
        assert!(!s.out_of_sample_validated);
        assert!(!s.copied_price_onto_okx);
        assert_eq!(s.sessions.len(), 1);
        assert_eq!(s.sessions[0].session, "asia");
        assert_eq!(s.sessions[0].bars, 2);
    }
}
