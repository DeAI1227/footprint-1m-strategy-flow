//! Encode OKX place/cancel. Never HTTP. Live send is double-locked.

use orderflow_domain::{live_open_allowed, AppConfig, LiveDenied, Mode, Venue};
use serde_json::{json, Value};

use crate::intent::{IntentKind, OrderIntent};
use crate::private::{cl_ord_id, parse_private_frame, Ack, PrivateEvent};

/// Private decode is on. Actual HTTP/WS write stays off.
pub const LIVE_SEND_WIRED: bool = false;

pub fn inst_id_for(symbol: &str, cfg: &AppConfig) -> String {
    if symbol.eq_ignore_ascii_case("SUI") {
        cfg.sui.okx_inst_id.clone()
    } else {
        cfg.sol.okx_inst_id.clone()
    }
}

pub fn tick_for(symbol: &str, cfg: &AppConfig) -> f64 {
    if symbol.eq_ignore_ascii_case("SUI") {
        cfg.sui.tick_sz
    } else {
        cfg.sol.tick_sz
    }
}

/// Both locks: calibration **and** live flags. Then live_send toml. Then this crate.
pub fn live_send_allowed(mode: Mode, cfg: &AppConfig) -> Result<(), LiveDenied> {
    live_open_allowed(mode, cfg)?;
    if !cfg.runtime.exec.live_send || !LIVE_SEND_WIRED {
        return Err(LiveDenied::ExecNotWired);
    }
    Err(LiveDenied::ExecNotWired)
}

pub fn format_px(px: f64, tick: f64) -> String {
    let t = if tick > 0.0 { tick } else { 0.01 };
    let n = if t >= 1.0 {
        0
    } else {
        let s = format!("{t:.10}");
        s.trim_end_matches('0')
            .split('.')
            .nth(1)
            .map(|x| x.len())
            .unwrap_or(2)
    };
    format!("{px:.n$}")
}

/// What would be written to OKX. Must never include a peer-venue price or secrets.
pub fn encode_place(
    intent: &OrderIntent,
    cfg: &AppConfig,
    seq: u64,
) -> Result<Value, &'static str> {
    if intent.copied_price_onto_okx {
        return Err("copied_price_onto_okx");
    }
    if intent.venue != Venue::Okx {
        return Err("not_okx");
    }
    let Some(px) = intent.limit_px else {
        return Err("no_limit");
    };
    let qty = intent.qty.unwrap_or(0.0);
    if qty <= 0.0 {
        return Err("qty_zero");
    }
    let tick = tick_for(&intent.symbol, cfg);
    let inst = inst_id_for(&intent.symbol, cfg);
    if inst.contains("BINANCE") || inst.contains("BYBIT") {
        return Err("not_okx");
    }
    let script = intent.script.as_deref().unwrap_or("X");
    let cl = intent
        .client_id
        .clone()
        .unwrap_or_else(|| cl_ord_id(&intent.symbol, script, seq));
    if cl.len() > 32 || !cl.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("bad_cl_ord_id");
    }
    let ord_type = match intent.kind {
        IntentKind::Flatten => "market",
        _ => "post_only",
    };
    let mut arg = json!({
        "instId": inst,
        "tdMode": cfg.runtime.exec.td_mode,
        "side": intent.side.as_str(),
        "ordType": ord_type,
        "sz": format_px(qty, tick),
        "clOrdId": cl,
    });
    if ord_type != "market" {
        arg["px"] = json!(format_px(px, tick));
    }
    Ok(json!({
        "id": format!("req{seq}"),
        "op": "order",
        "args": [arg],
        "copied_price_onto_okx": false,
    }))
}

pub fn encode_cancel(cl: &str, inst_id: &str, seq: u64) -> Value {
    json!({
        "id": format!("cx{seq}"),
        "op": "cancel-order",
        "args": [{"instId": inst_id, "clOrdId": cl}],
    })
}

pub fn parse_ack_text(text: &str) -> Option<Ack> {
    let evs = parse_private_frame(text).ok()?;
    evs.into_iter().find_map(|e| match e {
        PrivateEvent::Ack(a) => Some(a),
        _ => None,
    })
}

pub fn signal_stale(snapshot_px: f64, bid1: f64, ask1: f64, tick: f64, max_ticks: u32) -> bool {
    if tick <= 0.0 {
        return false;
    }
    let mid = (bid1 + ask1) / 2.0;
    (mid - snapshot_px).abs() > f64::from(max_ticks) * tick
}

/// Transport that encodes but never writes bytes. Tests prove this.
pub fn dispatch_live(mode: Mode, cfg: &AppConfig, payload: &Value) -> Result<(), LiveDenied> {
    let _ = payload;
    live_send_allowed(mode, cfg)
}
