//! Per-venue L2. Unhealthy book → that venue's DOM scripts `not_evaluated`.
//! Never fill OKX from another book. Script F stays `not_evaluated` until the book is Ok.

mod engine;
mod ladder;
mod parse;

pub use engine::{BookConfig, BookEngine, BookHealth, BookRead, ThreeBooks, WallRead};
pub use parse::{load_jsonl, parse_frame, subscribe_text, BookParseError};

pub const WIRED: bool = true;

pub fn unfinished_book_is_not_an_entry() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ladder::okx_checksum;
    use orderflow_domain::{QualityVector, TakerSide, Trade, Venue, VenueRole};

    fn trade(px: f64, sz: f64, side: TakerSide) -> Trade {
        Trade {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            trade_id: Some("1".into()),
            event_ts_ms: 1_000,
            recv_ts_ms: 1,
            processed_ts_ms: 1,
            price: px,
            size: sz,
            taker_side: side,
        }
    }

    fn ladder_json(seq: i64, prev: i64, extra_bid: Option<(&str, &str)>) -> String {
        let mut bids = vec![
            ("100.00", "1"),
            ("99.99", "1"),
            ("99.98", "1"),
            ("99.97", "1"),
            ("99.96", "1"),
        ];
        if let Some((p, s)) = extra_bid {
            if let Some(slot) = bids.iter_mut().find(|(bp, _)| *bp == p) {
                *slot = (p, s);
            } else {
                bids.insert(0, (p, s));
            }
        }
        let asks = [
            ("100.01", "1"),
            ("100.02", "1"),
            ("100.03", "1"),
            ("100.04", "1"),
            ("100.05", "1"),
        ];
        let bids_j: Vec<String> = bids
            .iter()
            .map(|(p, s)| format!("[\"{p}\",\"{s}\",\"0\",\"1\"]"))
            .collect();
        let asks_j: Vec<String> = asks
            .iter()
            .map(|(p, s)| format!("[\"{p}\",\"{s}\",\"0\",\"1\"]"))
            .collect();
        format!(
            r#"{{"arg":{{"channel":"books","instId":"SOL-USDT-SWAP"}},"action":"{}","data":[{{"bids":[{}],"asks":[{}],"ts":"1000","checksum":0,"seqId":{seq},"prevSeqId":{prev}}}]}}"#,
            if prev == -1 { "snapshot" } else { "update" },
            bids_j.join(","),
            asks_j.join(","),
        )
    }

    #[test]
    fn wired() {
        assert!(WIRED);
    }

    #[test]
    fn okx_seq_gap_poisons_only_okx() {
        let mut books = ThreeBooks::sol();
        books.okx.apply_frame(&ladder_json(10, -1, None)).unwrap();
        assert_eq!(books.okx.health(), BookHealth::Ok);
        // gap: prevSeqId 99 != 10
        books.okx.apply_frame(&ladder_json(11, 99, None)).unwrap();
        assert_eq!(books.okx.health(), BookHealth::Bad);
        let mut q = QualityVector::default();
        books.apply_quality(&mut q);
        assert!(!q.okx_book_ok);
        assert!(!q.binance_book_ok, "Binance never got a snapshot");
        // Binance still rebuilding, not marked from OKX gap
        books
            .binance
            .apply_frame(
                r#"{"lastUpdateId":5,"bids":[["100.00","1"],["99.99","1"],["99.98","1"],["99.97","1"],["99.96","1"]],"asks":[["100.01","1"],["100.02","1"],["100.03","1"],["100.04","1"],["100.05","1"]]}"#,
            )
            .unwrap();
        assert_eq!(books.binance.health(), BookHealth::Ok);
        let mut q = QualityVector::default();
        books.apply_quality(&mut q);
        assert!(!q.okx_book_ok);
        assert!(q.binance_book_ok);
        assert!(!q.bybit_book_ok);
        let r = books.okx.freeze_1m(None, &[]);
        assert!(!r.dom_entries_allowed);
        assert_eq!(r.script_f, "not_evaluated");
        assert_eq!(r.read, WallRead::NotEvaluated);
        let rb = books.binance.freeze_1m(None, &[]);
        assert!(
            rb.dom_entries_allowed,
            "resonance book ok does not block OKX"
        );
        assert_eq!(rb.resonance_read, "evaluated");
    }

    #[test]
    fn binance_pu_gap_requires_rebuild() {
        let mut eng = BookEngine::new(Venue::Binance, VenueRole::Resonance, BookConfig::sol());
        eng.apply_frame(
            r#"{"lastUpdateId":100,"bids":[["100.00","1"],["99.99","1"],["99.98","1"],["99.97","1"],["99.96","1"]],"asks":[["100.01","1"],["100.02","1"],["100.03","1"],["100.04","1"],["100.05","1"]]}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Ok);
        // pu=50 != 100
        eng.apply_frame(
            r#"{"e":"depthUpdate","T":1,"s":"SOLUSDT","U":101,"u":102,"pu":50,"b":[["100.00","2"]],"a":[]}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Bad);
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.resonance_read, "not_evaluated");
        assert!(r.dom_entries_allowed);
    }

    #[test]
    fn bybit_snapshot_then_delta_delete() {
        let mut eng = BookEngine::new(Venue::Bybit, VenueRole::Resonance, BookConfig::sol());
        eng.apply_frame(
            r#"{"topic":"orderbook.50.SOLUSDT","type":"snapshot","ts":1,"data":{"s":"SOLUSDT","b":[["100.00","1"],["99.99","1"],["99.98","1"],["99.97","1"],["99.96","8"],["99.95","1"]],"a":[["100.01","1"],["100.02","1"],["100.03","1"],["100.04","1"],["100.05","1"],["100.06","1"]],"u":10,"seq":1},"cts":1}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Ok);
        eng.apply_frame(
            r#"{"topic":"orderbook.50.SOLUSDT","type":"delta","ts":2,"data":{"s":"SOLUSDT","b":[["99.96","0"]],"a":[],"u":11,"seq":2},"cts":2}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Ok);
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.levels_bid, 5);
        // gap u=20
        eng.apply_frame(
            r#"{"topic":"orderbook.50.SOLUSDT","type":"delta","ts":3,"data":{"s":"SOLUSDT","b":[],"a":[["100.06","1"]],"u":20,"seq":3},"cts":3}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Bad);
    }

    #[test]
    fn bybit_u1_overwrites() {
        let mut eng = BookEngine::new(Venue::Bybit, VenueRole::Resonance, BookConfig::sol());
        eng.apply_frame(
            r#"{"type":"snapshot","ts":1,"data":{"b":[["100.00","1"],["99.99","1"],["99.98","1"],["99.97","1"],["99.96","1"]],"a":[["100.01","1"],["100.02","1"],["100.03","1"],["100.04","1"],["100.05","1"]],"u":9,"seq":1},"cts":1}"#,
        )
        .unwrap();
        eng.apply_frame(
            r#"{"type":"delta","ts":2,"data":{"b":[["101.00","1"],["100.99","1"],["100.98","1"],["100.97","1"],["100.96","1"]],"a":[["101.01","1"],["101.02","1"],["101.03","1"],["101.04","1"],["101.05","1"]],"u":1,"seq":2},"cts":2}"#,
        )
        .unwrap();
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.bid1, Some(101.00));
        assert_eq!(eng.health(), BookHealth::Ok);
    }

    #[test]
    fn checksum_zero_is_ignored_nonzero_mismatch_rebuilds() {
        let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        eng.apply_frame(&ladder_json(1, -1, None)).unwrap();
        assert_eq!(eng.health(), BookHealth::Ok);
        let bids = vec![("100.00".into(), "1".into()), ("99.99".into(), "1".into())];
        let asks = vec![("100.01".into(), "1".into())];
        assert_ne!(okx_checksum(&bids, &asks), 0);
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","2"]],"asks":[],"ts":"2","checksum":0,"seqId":2,"prevSeqId":1}]}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Ok);
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","3"]],"asks":[],"ts":"3","checksum":12345,"seqId":3,"prevSeqId":2}]}"#,
        )
        .unwrap();
        assert_eq!(eng.health(), BookHealth::Bad);
        assert!(eng.freeze_1m(None, &[]).checksum_fail >= 1);
    }

    #[test]
    fn execution_book_bad_blocks_dom_not_other_venues() {
        let eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.health, BookHealth::Rebuilding);
        assert!(!r.dom_entries_allowed);
        assert_eq!(r.script_f, "not_evaluated");
    }

    #[test]
    fn absorb_when_trades_hit_wall_and_price_holds() {
        let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        eng.apply_frame(&ladder_json(1, -1, Some(("100.00", "20"))))
            .unwrap();
        let r0 = eng.freeze_1m(Some(100.00), &[100.00]);
        assert!(r0.wall_on_poc);
        assert!(r0.wall_on_stack);
        eng.apply_trade(&trade(100.00, 3.0, TakerSide::Sell));
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","17"]],"asks":[],"ts":"2","checksum":0,"seqId":2,"prevSeqId":1}]}"#,
        )
        .unwrap();
        let r = eng.freeze_1m(Some(100.00), &[]);
        assert_eq!(r.read, WallRead::Absorb);
        assert_eq!(r.script_f, "computed");
        assert!(r.dom_entries_allowed);
    }

    #[test]
    fn yield_when_wall_pulls_without_trades() {
        let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        eng.apply_frame(&ladder_json(1, -1, Some(("100.00", "20"))))
            .unwrap();
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","0"]],"asks":[],"ts":"2","checksum":0,"seqId":2,"prevSeqId":1}]}"#,
        )
        .unwrap();
        // last trade through the old wall
        eng.apply_trade(&trade(99.95, 1.0, TakerSide::Sell));
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.read, WallRead::Yield);
    }

    #[test]
    fn eat_through_when_trades_consume_wall_and_price_crosses() {
        let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        eng.apply_frame(&ladder_json(1, -1, Some(("100.00", "20"))))
            .unwrap();
        eng.apply_trade(&trade(100.00, 20.0, TakerSide::Sell));
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","0"]],"asks":[],"ts":"2","checksum":0,"seqId":2,"prevSeqId":1}]}"#,
        )
        .unwrap();
        eng.apply_trade(&trade(99.95, 1.0, TakerSide::Sell));
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.read, WallRead::EatThrough);
    }

    #[test]
    fn fake_wall_when_pull_then_restore() {
        let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        eng.apply_frame(&ladder_json(1, -1, Some(("100.00", "20"))))
            .unwrap();
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","1"]],"asks":[],"ts":"2","checksum":0,"seqId":2,"prevSeqId":1}]}"#,
        )
        .unwrap();
        eng.apply_frame(
            r#"{"action":"update","data":[{"bids":[["100.00","20"]],"asks":[],"ts":"3","checksum":0,"seqId":3,"prevSeqId":2}]}"#,
        )
        .unwrap();
        let r = eng.freeze_1m(None, &[]);
        assert_eq!(r.read, WallRead::FakeWall);
    }

    #[test]
    fn control_frames_are_noop() {
        let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
        eng.apply_frame(r#"{"event":"subscribe","arg":{"channel":"books"}}"#)
            .unwrap();
        assert_eq!(eng.health(), BookHealth::Rebuilding);
        let mut bn = BookEngine::new(Venue::Binance, VenueRole::Resonance, BookConfig::sol());
        bn.apply_frame(r#"{"result":null,"id":2}"#).unwrap();
        assert_eq!(bn.health(), BookHealth::Rebuilding);
        let mut by = BookEngine::new(Venue::Bybit, VenueRole::Resonance, BookConfig::sol());
        by.apply_frame(r#"{"op":"subscribe","success":true}"#)
            .unwrap();
        assert_eq!(by.health(), BookHealth::Rebuilding);
    }

    #[test]
    fn sui_tick_is_not_copied_from_sol() {
        assert!((BookConfig::sol().tick_sz - 0.01).abs() < 1e-12);
        assert!((BookConfig::sui().tick_sz - 0.0001).abs() < 1e-12);
        assert_ne!(BookConfig::sol().tick_sz, BookConfig::sui().tick_sz);
    }

    #[test]
    fn subscribe_payloads_are_venue_native() {
        assert!(subscribe_text(Venue::Okx, "SOL-USDT-SWAP").contains("books"));
        assert!(subscribe_text(Venue::Binance, "SOLUSDT").contains("solusdt@depth"));
        assert!(subscribe_text(Venue::Bybit, "SOLUSDT").contains("orderbook.50.SOLUSDT"));
    }

    #[test]
    fn never_copies_binance_touch_onto_okx() {
        let mut books = ThreeBooks::sol();
        books.okx.apply_frame(&ladder_json(1, -1, None)).unwrap();
        books
            .binance
            .apply_frame(
                r#"{"lastUpdateId":1,"bids":[["200.00","1"],["199.99","1"],["199.98","1"],["199.97","1"],["199.96","1"]],"asks":[["200.01","1"],["200.02","1"],["200.03","1"],["200.04","1"],["200.05","1"]]}"#,
            )
            .unwrap();
        let okx = books.okx.freeze_1m(None, &[]);
        let bn = books.binance.freeze_1m(None, &[]);
        assert_eq!(okx.bid1, Some(100.00));
        assert_eq!(bn.bid1, Some(200.00));
        assert_ne!(okx.bid1, bn.bid1);
    }
}
