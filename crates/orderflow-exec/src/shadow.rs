//! SOL and SUI shadow in parallel. Separate params, separate ledgers.

use orderflow_domain::{AppConfig, Mode};

use crate::ledger::LedgerSnap;
use crate::private::{parse_private_frame, symbol_from_inst, PrivateEvent};
use crate::ExecGateway;

pub struct ShadowPair {
    pub sol: ExecGateway,
    pub sui: ExecGateway,
    pub private_ok: bool,
    pub account_mismatch: bool,
}

impl ShadowPair {
    pub fn new(cfg: &AppConfig) -> Self {
        Self {
            sol: ExecGateway::new(Mode::Shadow, cfg),
            sui: ExecGateway::new(Mode::Shadow, cfg),
            private_ok: false,
            account_mismatch: false,
        }
    }

    pub fn lane(&mut self, symbol: &str) -> &mut ExecGateway {
        if symbol.eq_ignore_ascii_case("SUI") {
            &mut self.sui
        } else {
            &mut self.sol
        }
    }

    pub fn apply_private(&mut self, text: &str, cfg: &AppConfig) -> Result<usize, String> {
        let evs = parse_private_frame(text)?;
        let mut n = 0;
        for ev in evs {
            n += 1;
            match ev {
                PrivateEvent::Refused(_) => {}
                PrivateEvent::Control => {}
                PrivateEvent::Ack(_) => {
                    self.private_ok = true;
                }
                PrivateEvent::Order(o) => {
                    self.private_ok = true;
                    let gw = self.lane(&o.symbol);
                    gw.private_ok = true;
                    if o.acc_fill_sz > 0.0 {
                        if let Some(w) = gw.ledger.working.get_mut(&o.cl_ord_id) {
                            w.filled = o.acc_fill_sz.min(w.qty);
                        }
                    }
                    if matches!(
                        o.state,
                        crate::private::OrderState::Canceled
                            | crate::private::OrderState::Filled
                            | crate::private::OrderState::Rejected
                    ) {
                        gw.ledger.working.remove(&o.cl_ord_id);
                    }
                }
                PrivateEvent::Fill(f) => {
                    self.private_ok = true;
                    let gw = self.lane(&f.symbol);
                    gw.private_ok = true;
                    gw.ledger.apply_fill(f);
                }
                PrivateEvent::Position(p) => {
                    self.private_ok = true;
                    let gw = self.lane(&p.symbol);
                    gw.private_ok = true;
                    gw.risk.pos_qty = p.qty;
                    if let Some(m) = p.mark_px {
                        gw.set_mark(&p.symbol, m);
                    }
                    if let Some(liq) = p.liq_px {
                        gw.set_liq(liq);
                    }
                }
                PrivateEvent::Account(a) => {
                    self.private_ok = true;
                    if !a.pos_mode.is_empty() && a.pos_mode != cfg.runtime.exec.pos_mode {
                        self.account_mismatch = true;
                        self.sol.account_mismatch = true;
                        self.sui.account_mismatch = true;
                    }
                }
            }
        }
        let _ = symbol_from_inst("SOL-USDT-SWAP");
        Ok(n)
    }

    pub fn snap(&self) -> (LedgerSnap, LedgerSnap) {
        (self.sol.ledger.snapshot(), self.sui.ledger.snapshot())
    }
}
