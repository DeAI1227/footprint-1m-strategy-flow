//! Bucket keys, session clock, and the min-volume percentile.
//!
//! Price keys are integer bucket indices so SOL 0.01 and SUI 0.0001 never share a map.
//! Min volume is **session p25 of nonempty single-side cells**, never a hardcoded SOL lot.

use orderflow_domain::bar_open_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Session {
    Asia,
    Eu,
    Us,
    Thin,
}

impl Session {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asia => "asia",
            Self::Eu => "eu",
            Self::Us => "us",
            Self::Thin => "thin",
        }
    }
}

/// UTC hour of the bar open: Asia 00–08, EU 08–13, US 13–21, thin 21–24.
pub fn session_of(ts_ms: i64) -> Session {
    let hour = ((ts_ms.div_euclid(3_600_000)).rem_euclid(24)) as u32;
    if hour < 8 {
        Session::Asia
    } else if (13..21).contains(&hour) {
        Session::Us
    } else if hour >= 21 {
        Session::Thin
    } else {
        Session::Eu
    }
}

pub fn utc_day(ts_ms: i64) -> i64 {
    ts_ms.div_euclid(86_400_000)
}

pub fn session_key(ts_ms: i64) -> (i64, Session) {
    let open = bar_open_ms(ts_ms);
    (utc_day(open), session_of(open))
}

/// `floor(px / bucket + 1e-12)` — matches the research script.
pub fn bucket_key(px: f64, bucket: f64) -> i64 {
    debug_assert!(bucket > 0.0);
    (px / bucket + 1e-12).floor() as i64
}

pub fn key_to_price(key: i64, bucket: f64) -> f64 {
    key as f64 * bucket
}

/// Linear-interpolated percentile. `p` in 0..=100. Empty → 0.
pub fn percentile(xs: &[f64], p: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut ys = xs.to_vec();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if p <= 0.0 {
        return ys[0];
    }
    if p >= 100.0 {
        return ys[ys.len() - 1];
    }
    let k = (ys.len() - 1) as f64 * (p / 100.0);
    let f = k.floor() as usize;
    let c = k.ceil() as usize;
    if f == c {
        ys[f]
    } else {
        ys[f] * (c as f64 - k) + ys[c] * (k - f as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sol_and_sui_keys_do_not_collide_by_accident() {
        let sol = bucket_key(100.33, 0.01);
        let sui = bucket_key(3.2456, 0.0001);
        assert_eq!(sol, 10033);
        assert_eq!(sui, 32456);
        assert_ne!(bucket_key(100.00, 0.01), bucket_key(100.00, 0.0001));
    }

    #[test]
    fn session_hours_match_research_script() {
        let day = 1_700_000_000_000i64; // some UTC ms
        let midnight = day - day.rem_euclid(86_400_000);
        assert_eq!(session_of(midnight), Session::Asia);
        assert_eq!(session_of(midnight + 8 * 3_600_000), Session::Eu);
        assert_eq!(session_of(midnight + 13 * 3_600_000), Session::Us);
        assert_eq!(session_of(midnight + 21 * 3_600_000), Session::Thin);
    }

    #[test]
    fn p25_interpolates() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        let p = percentile(&xs, 25.0);
        assert!((p - 1.75).abs() < 1e-9);
        assert_eq!(percentile(&[], 25.0), 0.0);
    }
}
