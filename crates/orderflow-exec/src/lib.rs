//! Stage 6: local sim matching + risk. OKX private I/O is still stage 7.
//! Live opens stay impossible even if someone flips toml flags.

mod intent;
mod ledger;
mod rest;
mod risk;
mod sim;

pub use intent::{IntentKind, OrderIntent, Side, Universe};
pub use ledger::{Ledger, LedgerSnap, Position};
pub use rest::{RestOp, RestPriority, RestQueue};
pub use risk::{Degrade, KillAction, KillState, RiskEngine};
pub use sim::{match_trade, taker_fill, BookLevel, Fill, SimBook, WorkingOrder};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use orderflow_domain::{
    live_open_allowed, AppConfig, LiveDenied, Mode, TakerSide, Trade, Venue,
};

/// Live path is not wired. Sim matching is.
pub const WIRED: bool = false;
pub const SIM_WIRED: bool = true;
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
}
