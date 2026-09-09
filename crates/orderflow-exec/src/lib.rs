//! Stage 7: OKX private decode + order encode. Live send stays double-locked.
//! Default mode is shadow. SUI shadow runs on its own param table.

mod intent;
mod ledger;
mod order;
mod private;
mod rest;
mod risk;
mod shadow;
mod sim;

pub use intent::{IntentKind, OrderIntent, Side, Universe};
pub use ledger::{Ledger, LedgerSnap, Position};
pub use order::{
    dispatch_live, encode_cancel, encode_place, format_px, inst_id_for, live_send_allowed,
    parse_ack_text, signal_stale, tick_for, LIVE_SEND_WIRED,
};
pub use private::{
    cl_ord_id, parse_private_frame, private_subscribe_okx, Ack, OrderState, PrivateEvent,
    PrivateOrder, RejectClass,
};
pub use rest::{RestOp, RestPriority, RestQueue};
pub use risk::{Degrade, KillAction, KillState, RiskEngine};
pub use shadow::ShadowPair;
pub use sim::{match_trade, taker_fill, BookLevel, Fill, SimBook, WorkingOrder};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use orderflow_domain::{
    live_open_allowed, AppConfig, LiveDenied, Mode, TakerSide, Trade, Venue,
};

/// Live HTTP write is not wired. Private decode and sim matching are.
pub const WIRED: bool = false;
pub const SIM_WIRED: bool = true;
pub const PRIVATE_WIRED: bool = true;
pub const LIVE_WIRED: bool = false;

pub fn submit_live_open(mode: Mode, cfg: &AppConfig) -> Result<(), LiveDenied> {
    live_open_allowed(mode, cfg)?;
    Err(LiveDenied::ExecNotWired)
}

#[derive(Debug, Clone)]
pub struct SubmitResult {
    pub accepted: bool,
    pub reason: &'static str,
    pub client_id: Option<String>,
    pub qty: f64,
}

pub struct ExecGateway {
    pub mode: Mode,
    pub risk: RiskEngine,
    pub ledger: Ledger,
    pub book: Option<SimBook>,
    pub marks: BTreeMap<String, f64>,
    pub rest: RestQueue,
    pub private_ok: bool,
    pub account_mismatch: bool,
    seq_clock: i64,
}

impl ExecGateway {
    pub fn new(mode: Mode, cfg: &AppConfig) -> Self {
        Self {
            mode,
            risk: RiskEngine::new(cfg.runtime.risk.clone()),
            ledger: Ledger::default(),
            book: None,
            marks: BTreeMap::new(),
            rest: RestQueue::default(),
            private_ok: false,
            account_mismatch: false,
            seq_clock: 1,
        }
    }

    pub fn set_book(&mut self, book: SimBook) {
        if book.venue != Venue::Okx {
            return;
        }
        if let Some(px) = book.best_bid() {
            // mark mid if both sides exist
            if let Some(ask) = book.best_ask() {
                self.marks.insert("SOL".into(), (px + ask) / 2.0);
                self.marks.insert("SUI".into(), (px + ask) / 2.0);
            }
        }
        self.book = Some(book);
    }

    pub fn set_mark(&mut self, symbol: &str, px: f64) {
        self.marks.insert(symbol.to_string(), px);
        self.risk.mark = px;
    }

    pub fn set_liq(&mut self, px: f64) {
        self.risk.liq_px = Some(px);
    }

    fn now_ms(&mut self) -> i64 {
        self.seq_clock += 1;
        self.seq_clock
    }

    fn unrealized(&self) -> f64 {
        self.ledger.unrealized(&self.marks)
    }

    pub fn submit(&mut self, mut intent: OrderIntent, cfg: &AppConfig) -> SubmitResult {
        if intent.copied_price_onto_okx {
            return SubmitResult {
                accepted: false,
                reason: "copied_price_onto_okx",
                client_id: None,
                qty: 0.0,
            };
        }
        if intent.venue != Venue::Okx {
            return SubmitResult {
                accepted: false,
                reason: "not_okx",
                client_id: None,
                qty: 0.0,
            };
        }
        if intent.live || self.mode.is_live() {
            let reason = match submit_live_open(self.mode, cfg) {
                Err(e) => e.as_str(),
                Ok(()) => "exec_not_wired",
            };
            return SubmitResult {
                accepted: false,
                reason,
                client_id: None,
                qty: 0.0,
            };
        }

        match intent.kind {
            IntentKind::ShadowSignal => SubmitResult {
                accepted: true,
                reason: "shadow_only",
                client_id: None,
                qty: 0.0,
            },
            IntentKind::Cancel => {
                if let Some(id) = &intent.client_id {
                    self.ledger.working.remove(id);
                }
                self.rest.push(RestPriority::Cancel, "cancel");
                SubmitResult {
                    accepted: true,
                    reason: "canceled",
                    client_id: intent.client_id,
                    qty: 0.0,
                }
            }
            IntentKind::Flatten => self.submit_flatten(intent, cfg),
            IntentKind::SimOpen => {
                if self.mode != Mode::Sim {
                    return SubmitResult {
                        accepted: false,
                        reason: "not_sim",
                        client_id: None,
                        qty: 0.0,
                    };
                }
                self.submit_open(&mut intent, cfg)
            }
        }
    }

    fn submit_open(&mut self, intent: &mut OrderIntent, cfg: &AppConfig) -> SubmitResult {
        if self.account_mismatch {
            return SubmitResult {
                accepted: false,
                reason: "account_mode_mismatch",
                client_id: None,
                qty: 0.0,
            };
        }
        if let Err(r) = self.risk.allow_open(intent, self.unrealized()) {
            return SubmitResult {
                accepted: false,
                reason: r,
                client_id: None,
                qty: 0.0,
            };
        }
        let Some(limit) = intent.limit_px else {
            return SubmitResult {
                accepted: false,
                reason: "no_limit",
                client_id: None,
                qty: 0.0,
            };
        };
        let Some(inv) = intent.invalidation_px else {
            return SubmitResult {
                accepted: false,
                reason: "no_invalidation",
                client_id: None,
                qty: 0.0,
            };
        };
        let tick = if intent.symbol.eq_ignore_ascii_case("SUI") {
            cfg.sui.tick_sz
        } else {
            cfg.sol.tick_sz
        };
        let ct = if intent.symbol.eq_ignore_ascii_case("SUI") {
            cfg.sui.ct_val
        } else {
            cfg.sol.ct_val
        };
        if RiskEngine::stop_too_tight(limit, inv, tick) {
            // Still size, but cap will clip. Record the tight stop.
        }
        let lot = tick;
        let qty = intent
            .qty
            .filter(|q| *q > 0.0)
            .unwrap_or_else(|| self.risk.size_qty(limit, inv, tick, ct, lot));
        if qty <= 0.0 {
            return SubmitResult {
                accepted: false,
                reason: "qty_zero",
                client_id: None,
                qty: 0.0,
            };
        }
        if let Some(id) = &intent.client_id {
            if self.ledger.working.contains_key(id) {
                return SubmitResult {
                    accepted: true,
                    reason: "idempotent",
                    client_id: Some(id.clone()),
                    qty,
                };
            }
        }
        let now = self.now_ms();
        let script = intent.script.clone().unwrap_or_else(|| "X".into());
        let id = intent
            .client_id
            .clone()
            .unwrap_or_else(|| self.ledger.next_client_id(&intent.symbol, &script, now));
        let queue_ahead = self
            .book
            .as_ref()
            .filter(|b| b.venue == Venue::Okx)
            .map(|b| b.size_at(intent.side, limit))
            .unwrap_or(0.0);
        let working = WorkingOrder {
            client_id: id.clone(),
            symbol: intent.symbol.clone(),
            side: intent.side,
            limit_px: limit,
            qty,
            filled: 0.0,
            queue_ahead,
            script: intent.script.clone(),
        };
        self.ledger.working.insert(id.clone(), working);
        self.risk.day_trades += 1;
        self.rest.push(RestPriority::Open, "sim_open");
        SubmitResult {
            accepted: true,
            reason: "accepted",
            client_id: Some(id),
            qty,
        }
    }

    fn submit_flatten(&mut self, intent: OrderIntent, _cfg: &AppConfig) -> SubmitResult {
        if let Err(r) = self.risk.allow_flatten() {
            return SubmitResult {
                accepted: false,
                reason: r,
                client_id: None,
                qty: 0.0,
            };
        }
        let pos = self.ledger.position(&intent.symbol);
        if pos.abs() <= 1e-12 {
            return SubmitResult {
                accepted: true,
                reason: "flat",
                client_id: None,
                qty: 0.0,
            };
        }
        let side = if pos > 0.0 { Side::Sell } else { Side::Buy };
        let Some(book) = self.book.clone() else {
            return SubmitResult {
                accepted: false,
                reason: "no_book",
                client_id: None,
                qty: 0.0,
            };
        };
        if book.venue != Venue::Okx {
            return SubmitResult {
                accepted: false,
                reason: "not_okx",
                client_id: None,
                qty: 0.0,
            };
        }
        let now = self.now_ms();
        let id = self.ledger.next_client_id(&intent.symbol, "flat", now);
        let working = WorkingOrder {
            client_id: id.clone(),
            symbol: intent.symbol.clone(),
            side,
            limit_px: book.opposite_best(side).unwrap_or(0.0),
            qty: pos.abs(),
            filled: 0.0,
            queue_ahead: 0.0,
            script: Some("risk".into()),
        };
        if let Some(fill) = taker_fill(&working, &book) {
            self.ledger.apply_fill(fill);
            self.risk.realized_pnl = self.ledger.realized_pnl();
            self.risk.pos_qty = self.ledger.position(&intent.symbol);
        }
        self.rest.push(RestPriority::Flatten, "flatten");
        SubmitResult {
            accepted: true,
            reason: "flattened",
            client_id: Some(id),
            qty: pos.abs(),
        }
    }

    pub fn on_trade(&mut self, trade: &Trade) -> Vec<Fill> {
        if trade.venue != Venue::Okx {
            return Vec::new();
        }
        self.marks
            .insert(trade.symbol.clone(), trade.price);
        self.risk.mark = trade.price;
        let ids: Vec<String> = self.ledger.working.keys().cloned().collect();
        let mut out = Vec::new();
        for id in ids {
            let Some(mut order) = self.ledger.working.remove(&id) else {
                continue;
            };
            if let Some(fill) = match_trade(&mut order, trade) {
                out.push(fill.clone());
                self.ledger.apply_fill(fill);
            }
            if !order.done() {
                self.ledger.working.insert(id, order);
            }
        }
        self.risk.realized_pnl = self.ledger.realized_pnl();
        if let Some(sym) = out.first().map(|f| f.symbol.clone()) {
            self.risk.pos_qty = self.ledger.position(&sym);
        }
        if self.risk.daily_loss_tripped(self.unrealized())
            && self.risk.kill.action == KillAction::Off
        {
            self.risk.trip(KillAction::ReduceOnly, "daily_loss", trade.event_ts_ms);
        }
        if self.risk.liq_buffer_breached() && self.risk.kill.action == KillAction::Off {
            self.risk.trip(KillAction::FlattenAll, "liq_buffer", trade.event_ts_ms);
        }
        out
    }

    pub fn trip_kill(&mut self, action: KillAction, reason: &str) {
        let now = self.now_ms();
        self.risk.trip(action, reason, now);
        self.rest.push(RestPriority::Risk, "kill");
        match action {
            KillAction::CancelAll | KillAction::Halt => {
                self.ledger.working.clear();
            }
            KillAction::FlattenAll => {
                self.ledger.working.clear();
            }
            KillAction::ReduceOnly | KillAction::Off => {}
        }
    }

    pub fn set_degrade(&mut self, d: Degrade) {
        self.risk.degrade = d;
    }
}

/// JSONL fixture: book / intent / trade / kill. No API keys.
pub fn run_sim_fixture(
    path: &Path,
    cfg: &AppConfig,
    kill_path: Option<&Path>,
) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut gw = ExecGateway::new(Mode::Sim, cfg);
    if let Some(p) = kill_path {
        gw.risk.load_persist(p)?;
    }
    let mut intents = 0u32;
    let mut denied = 0u32;
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        match v.get("event").and_then(|x| x.as_str()).unwrap_or("") {
            "book" => {
                let book: SimBook = serde_json::from_value(v).map_err(|e| e.to_string())?;
                gw.set_book(book);
            }
            "intent" => {
                let intent: OrderIntent = serde_json::from_value(v).map_err(|e| e.to_string())?;
                intents += 1;
                let r = gw.submit(intent, cfg);
                if !r.accepted {
                    denied += 1;
                }
            }
            "trade" => {
                let venue = v
                    .get("venue")
                    .and_then(|x| x.as_str())
                    .unwrap_or("okx");
                let trade = Trade {
                    venue: Venue::parse(venue)?,
                    symbol: v
                        .get("symbol")
                        .and_then(|x| x.as_str())
                        .unwrap_or("SOL")
                        .to_string(),
                    trade_id: None,
                    event_ts_ms: v.get("event_ts_ms").and_then(|x| x.as_i64()).unwrap_or(0),
                    recv_ts_ms: 0,
                    processed_ts_ms: 0,
                    price: v.get("price").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    size: v.get("size").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    taker_side: match v.get("taker_side").and_then(|x| x.as_str()).unwrap_or("sell")
                    {
                        "buy" => TakerSide::Buy,
                        _ => TakerSide::Sell,
                    },
                };
                gw.on_trade(&trade);
            }
            "kill" => {
                let action = match v.get("action").and_then(|x| x.as_str()).unwrap_or("") {
                    "halt" => KillAction::Halt,
                    "flatten_all" => KillAction::FlattenAll,
                    "reduce_only" => KillAction::ReduceOnly,
                    "cancel_all" => KillAction::CancelAll,
                    _ => KillAction::Off,
                };
                gw.trip_kill(action, "fixture");
            }
            _ => {}
        }
    }
    if let Some(p) = kill_path {
        gw.risk.persist(p)?;
    }
    let snap = gw.ledger.snapshot();
    Ok(serde_json::json!({
        "event": "sim_done",
        "sim_wired": true,
        "live_wired": false,
        "copied_price_onto_okx": false,
        "intents": intents,
        "denied": denied,
        "fills": snap.fill_n,
        "working": snap.working.len(),
        "realized_pnl": snap.realized_pnl,
        "positions": snap.positions,
        "kill": gw.risk.kill.action,
        "note": "stage 6: local OKX book matching; no API keys; live still gated",
    }))
}

/// Replay OKX private JSONL onto SOL+SUI shadow ledgers. Never sends live.
pub fn run_private_replay(path: &Path, cfg: &AppConfig) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut pair = ShadowPair::new(cfg);
    let mut n = 0u32;
    let mut refused = 0u32;
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_private_frame(line) {
            Ok(evs) => {
                for ev in &evs {
                    if matches!(ev, PrivateEvent::Refused(_)) {
                        refused += 1;
                    }
                }
                pair.apply_private(line, cfg)
                    .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
                n += 1;
            }
            Err(e) => return Err(format!("{}:{}: {e}", path.display(), i + 1)),
        }
    }
    let (sol, sui) = pair.snap();
    Ok(serde_json::json!({
        "event": "private_replay_done",
        "private_wired": true,
        "live_wired": false,
        "live_send": cfg.runtime.exec.live_send,
        "copied_price_onto_okx": false,
        "frames": n,
        "refused": refused,
        "private_ok": pair.private_ok,
        "account_mismatch": pair.account_mismatch,
        "sol_fills": sol.fill_n,
        "sui_fills": sui.fill_n,
        "sol_qty": sol.positions.iter().map(|p| p.qty).sum::<f64>(),
        "sui_qty": sui.positions.iter().map(|p| p.qty).sum::<f64>(),
        "note": "stage 7: OKX private decode; shadow default; live double-locked; no API keys",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> AppConfig {
        AppConfig::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("params"),
        )
        .unwrap()
    }

    fn okx_trade(px: f64, sz: f64, side: TakerSide) -> Trade {
        Trade {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            trade_id: None,
            event_ts_ms: 1,
            recv_ts_ms: 1,
            processed_ts_ms: 1,
            price: px,
            size: sz,
            taker_side: side,
        }
    }

    #[test]
    fn submit_live_open_never_succeeds_on_repo_params() {
        let cfg = cfg();
        assert_eq!(
            submit_live_open(Mode::Live, &cfg).unwrap_err(),
            LiveDenied::ParamsNotCalibrated
        );
        assert_eq!(
            submit_live_open(Mode::Shadow, &cfg).unwrap_err(),
            LiveDenied::ExecNotWired
        );
        assert!(!WIRED);
        assert!(!LIVE_WIRED);
        assert!(SIM_WIRED);
        assert!(PRIVATE_WIRED);
        assert!(!LIVE_SEND_WIRED);
        assert!(!cfg.runtime.exec.live_send);
        assert_eq!(cfg.runtime.mode_default, Mode::Shadow);
    }

    #[test]
    fn queue_then_partial_then_fill_on_okx_only() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.set_book(SimBook::okx(0.01, &[(100.00, 5.0)], &[(100.01, 4.0)]));
        let r = gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 2.0), &cfg);
        assert!(r.accepted, "{}", r.reason);
        let fills = gw.on_trade(&okx_trade(100.00, 5.0, TakerSide::Sell));
        assert!(fills.is_empty(), "queue ahead must eat the first 5");
        let fills = gw.on_trade(&okx_trade(100.00, 1.0, TakerSide::Sell));
        assert_eq!(fills.len(), 1);
        assert!((fills[0].qty - 1.0).abs() < 1e-9);
        assert!(fills[0].maker);
        assert_eq!(gw.ledger.position("SOL"), 1.0);
        let fills = gw.on_trade(&okx_trade(100.00, 2.0, TakerSide::Sell));
        assert_eq!(fills[0].qty, 1.0);
        assert_eq!(gw.ledger.position("SOL"), 2.0);
        assert!(gw.ledger.working.is_empty());
    }

    #[test]
    fn binance_trade_does_not_fill_okx_order() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.set_book(SimBook::okx(0.01, &[(100.00, 0.0)], &[(100.01, 1.0)]));
        gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0), &cfg);
        let mut t = okx_trade(100.00, 10.0, TakerSide::Sell);
        t.venue = Venue::Binance;
        assert!(gw.on_trade(&t).is_empty());
        assert_eq!(gw.ledger.position("SOL"), 0.0);
    }

    #[test]
    fn copied_price_is_rejected() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        let mut intent = OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0);
        intent.copied_price_onto_okx = true;
        let r = gw.submit(intent, &cfg);
        assert!(!r.accepted);
        assert_eq!(r.reason, "copied_price_onto_okx");
    }

    #[test]
    fn kill_switch_survives_restart() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.trip_kill(KillAction::Halt, "test");
        let path = std::env::temp_dir().join(format!("of-kill-{}.json", std::process::id()));
        gw.risk.persist(&path).unwrap();
        let mut gw2 = ExecGateway::new(Mode::Sim, &cfg);
        gw2.risk.load_persist(&path).unwrap();
        let r = gw2.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0), &cfg);
        assert_eq!(r.reason, "kill_halt");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn daily_loss_blocks_new_opens() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.risk.realized_pnl = -80.0;
        let r = gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0), &cfg);
        assert_eq!(r.reason, "daily_loss");
    }

    #[test]
    fn liq_buffer_blocks_new_opens() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.risk.pos_qty = 1.0;
        gw.risk.mark = 100.0;
        gw.risk.liq_px = Some(99.0); // 1% < 5%
        let r = gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0), &cfg);
        assert_eq!(r.reason, "liq_buffer");
    }

    #[test]
    fn degrade_stops_opens_but_flatten_still_runs() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.set_book(SimBook::okx(0.01, &[(100.00, 0.0)], &[(100.01, 2.0)]));
        gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0), &cfg);
        gw.on_trade(&okx_trade(100.00, 1.0, TakerSide::Sell));
        assert_eq!(gw.ledger.position("SOL"), 1.0);
        gw.set_degrade(Degrade::StopOpens);
        let r = gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0), &cfg);
        assert_eq!(r.reason, "degrade_stop_opens");
        let mut flat = OrderIntent::sim_open("SOL", Side::Sell, 100.01, 1.0);
        flat.kind = IntentKind::Flatten;
        let r = gw.submit(flat, &cfg);
        assert!(r.accepted, "{}", r.reason);
        assert_eq!(gw.ledger.position("SOL"), 0.0);
    }

    #[test]
    fn research_shed_before_core() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.set_degrade(Degrade::StopResearch);
        let mut research = OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0);
        research.universe = Universe::Research;
        assert_eq!(gw.submit(research, &cfg).reason, "degrade_research");
        let core = OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0);
        assert_eq!(gw.submit(core, &cfg).reason, "accepted");
    }

    #[test]
    fn tiny_stop_is_clipped_by_symbol_cap() {
        let cfg = cfg();
        let risk = RiskEngine::new(cfg.runtime.risk.clone());
        // 0.01 stop on 10000 * 0.002 = 20 risk → 2000 coin, notional 200000, clip to 1000
        let qty = risk.size_qty(100.0, 99.99, 0.01, 1.0, 0.01);
        assert!(qty * 100.0 <= 1000.0 + 1e-6, "qty={qty}");
        assert!(qty > 0.0);
    }

    #[test]
    fn sui_tick_is_not_sol() {
        let cfg = cfg();
        assert_eq!(cfg.sui.tick_sz, 0.0001);
        assert_eq!(cfg.sol.tick_sz, 0.01);
        let risk = RiskEngine::new(cfg.runtime.risk.clone());
        let sol = risk.size_qty(100.0, 99.0, cfg.sol.tick_sz, cfg.sol.ct_val, cfg.sol.tick_sz);
        let sui = risk.size_qty(1.0, 0.9, cfg.sui.tick_sz, cfg.sui.ct_val, cfg.sui.tick_sz);
        assert_ne!(sol, sui);
    }

    #[test]
    fn shadow_does_not_create_working() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Shadow, &cfg);
        let mut intent = OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0);
        intent.kind = IntentKind::ShadowSignal;
        let r = gw.submit(intent, &cfg);
        assert_eq!(r.reason, "shadow_only");
        assert!(gw.ledger.working.is_empty());
    }

    #[test]
    fn rest_priority_is_risk_first() {
        let mut q = RestQueue::default();
        q.push(RestPriority::Query, "q");
        q.push(RestPriority::Open, "o");
        q.push(RestPriority::Risk, "r");
        q.push(RestPriority::Flatten, "f");
        q.push(RestPriority::Cancel, "c");
        assert_eq!(q.pop().unwrap().name, "r");
        assert_eq!(q.pop().unwrap().name, "c");
        assert_eq!(q.pop().unwrap().name, "f");
        assert_eq!(q.pop().unwrap().name, "o");
        assert_eq!(q.pop().unwrap().name, "q");
    }

    #[test]
    fn fixture_file_runs_without_keys() {
        let cfg = cfg();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/okx_sim.jsonl");
        let out = run_sim_fixture(&path, &cfg, None).unwrap();
        assert_eq!(out["sim_wired"], true);
        assert_eq!(out["live_wired"], false);
        assert_eq!(out["copied_price_onto_okx"], false);
        assert!(out["fills"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn idempotent_client_id() {
        let cfg = cfg();
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.set_book(SimBook::okx(0.01, &[(100.00, 0.0)], &[(100.01, 1.0)]));
        let mut a = OrderIntent::sim_open("SOL", Side::Buy, 100.00, 1.0);
        a.client_id = Some("of-dup".into());
        let b = a.clone();
        assert_eq!(gw.submit(a, &cfg).reason, "accepted");
        assert_eq!(gw.submit(b, &cfg).reason, "idempotent");
        assert_eq!(gw.ledger.working.len(), 1);
    }

    #[test]
    fn private_orders_and_fills_update_sol_not_sui() {
        let cfg = cfg();
        let mut pair = ShadowPair::new(&cfg);
        let orders = r#"{"arg":{"channel":"orders","instType":"SWAP"},"data":[{"instId":"SOL-USDT-SWAP","clOrdId":"ofsolA00000001","ordId":"1","side":"buy","px":"100.00","sz":"2","accFillSz":"0","state":"live"}]}"#;
        pair.apply_private(orders, &cfg).unwrap();
        let fills = r#"{"arg":{"channel":"fills","instType":"SWAP"},"data":[{"instId":"SOL-USDT-SWAP","clOrdId":"ofsolA00000001","fillPx":"100.00","fillSz":"2","side":"buy","execType":"M"}]}"#;
        pair.apply_private(fills, &cfg).unwrap();
        let (sol, sui) = pair.snap();
        assert_eq!(sol.fill_n, 1);
        assert!((sol.positions[0].qty - 2.0).abs() < 1e-9);
        assert!(sui.positions.is_empty());
        assert!(pair.private_ok);
    }

    #[test]
    fn sui_fill_uses_native_inst_not_sol_tick() {
        let cfg = cfg();
        assert_eq!(tick_for("SUI", &cfg), 0.0001);
        assert_eq!(tick_for("SOL", &cfg), 0.01);
        assert_eq!(inst_id_for("SUI", &cfg), "SUI-USDT-SWAP");
        let mut pair = ShadowPair::new(&cfg);
        let fills = r#"{"arg":{"channel":"fills","instType":"SWAP"},"data":[{"instId":"SUI-USDT-SWAP","clOrdId":"ofsuiA00000001","fillPx":"1.2345","fillSz":"3","side":"buy","execType":"M"}]}"#;
        pair.apply_private(fills, &cfg).unwrap();
        let (sol, sui) = pair.snap();
        assert!(sol.positions.is_empty());
        assert!((sui.positions[0].qty - 3.0).abs() < 1e-9);
        let intent = OrderIntent::sim_open("SUI", Side::Buy, 1.2345, 1.0);
        let payload = encode_place(&intent, &cfg, 1).unwrap();
        let px = payload["args"][0]["px"].as_str().unwrap();
        assert!(px.starts_with("1.234"), "{px}");
        assert_eq!(payload["args"][0]["instId"], "SUI-USDT-SWAP");
        assert_eq!(payload["copied_price_onto_okx"], false);
    }

    #[test]
    fn cl_ord_id_is_okx_safe() {
        let id = cl_ord_id("SOL", "A", 12);
        assert!(id.len() <= 32);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(id.starts_with("ofsol"));
        let sui = cl_ord_id("SUI", "C", 3);
        assert!(sui.starts_with("ofsui"));
        assert_ne!(id, sui);
    }

    #[test]
    fn ack_precision_and_ok() {
        let ok = parse_ack_text(r#"{"op":"order","id":"req1","code":"0","data":[{"clOrdId":"ofsolA00000001","ordId":"9","sCode":"0","sMsg":""}]}"#).unwrap();
        assert!(ok.ok);
        assert_eq!(ok.class, RejectClass::Ok);
        let bad = parse_ack_text(r#"{"op":"order","id":"req1","code":"1","data":[{"clOrdId":"ofsolA00000001","sCode":"51000","sMsg":"Tick size precision error"}]}"#).unwrap();
        assert!(!bad.ok);
        assert_eq!(bad.class, RejectClass::Precision);
    }

    #[test]
    fn binance_private_is_refused() {
        let evs = parse_private_frame(r#"{"venue":"binance","stream":"binance.user","data":[]}"#).unwrap();
        assert!(matches!(evs[0], PrivateEvent::Refused("not_okx_private")));
    }

    #[test]
    fn copied_price_cannot_encode() {
        let cfg = cfg();
        let mut intent = OrderIntent::sim_open("SOL", Side::Buy, 100.0, 1.0);
        intent.copied_price_onto_okx = true;
        assert_eq!(encode_place(&intent, &cfg, 1).unwrap_err(), "copied_price_onto_okx");
    }

    #[test]
    fn dispatch_live_never_sends_even_if_flags_flip() {
        let mut cfg = cfg();
        cfg.runtime.calibration.status = orderflow_domain::CalibrationStatus::OutOfSampleValidated;
        cfg.runtime.calibration.out_of_sample_validated = true;
        cfg.runtime.calibration.calibration_complete = true;
        cfg.runtime.calibration.live_authorized = true;
        cfg.runtime.exec.live_send = true;
        cfg.sol.live_enabled = true;
        cfg.sol.calibration_complete = true;
        cfg.sol.out_of_sample_validated = true;
        cfg.sol.armed_rate_policy = orderflow_domain::ArmedRatePolicy::Dale300;
        let payload = serde_json::json!({"op":"order"});
        assert_eq!(
            dispatch_live(Mode::Live, &cfg, &payload).unwrap_err(),
            LiveDenied::ExecNotWired
        );
        assert_eq!(
            live_send_allowed(Mode::Live, &cfg).unwrap_err(),
            LiveDenied::ExecNotWired
        );
    }

    #[test]
    fn account_mode_mismatch_blocks_opens() {
        let cfg = cfg();
        let mut pair = ShadowPair::new(&cfg);
        pair.apply_private(
            r#"{"arg":{"channel":"account"},"data":[{"posMode":"long_short_mode"}]}"#,
            &cfg,
        )
        .unwrap();
        assert!(pair.account_mismatch);
        let mut gw = ExecGateway::new(Mode::Sim, &cfg);
        gw.account_mismatch = true;
        let r = gw.submit(OrderIntent::sim_open("SOL", Side::Buy, 100.0, 1.0), &cfg);
        assert_eq!(r.reason, "account_mode_mismatch");
    }

    #[test]
    fn signal_stale_uses_okx_book_not_peer() {
        let cfg = cfg();
        assert!(signal_stale(100.0, 100.10, 100.12, cfg.sol.tick_sz, 4));
        assert!(!signal_stale(100.0, 100.01, 100.02, cfg.sol.tick_sz, 4));
        assert!(!signal_stale(1.2345, 1.2344, 1.2346, cfg.sui.tick_sz, 4));
    }

    #[test]
    fn private_fixture_replay_no_keys() {
        let cfg = cfg();
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/okx_private.jsonl");
        let out = run_private_replay(&path, &cfg).unwrap();
        assert_eq!(out["private_wired"], true);
        assert_eq!(out["live_wired"], false);
        assert_eq!(out["live_send"], false);
        assert!(out["sol_fills"].as_u64().unwrap() >= 1);
        assert!(out["sui_fills"].as_u64().unwrap() >= 1);
        let line = out.to_string().to_ascii_lowercase();
        assert!(!line.contains("secret"));
        assert!(!line.contains("apikey"));
        assert!(!line.contains("passphrase"));
    }

    #[test]
    fn subscribe_payload_is_okx_only() {
        let s = private_subscribe_okx();
        assert!(s.contains("orders"));
        assert!(!s.to_ascii_lowercase().contains("binance"));
        assert!(!s.to_ascii_lowercase().contains("bybit"));
    }
}
