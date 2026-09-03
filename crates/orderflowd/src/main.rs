use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use orderflow_clock::{BarCutter, CutEvent};
use orderflow_domain::{
    boot_decision, default_config_dir, json_log, AppConfig, Mode, Venue,
};
use orderflow_exec::submit_live_open;
use orderflow_ingest::journal::JsonlJournal;
use orderflow_ingest::okx;

struct Args {
    mode: Option<Mode>,
    config_dir: PathBuf,
    once: bool,
    replay: Option<PathBuf>,
    journal: Option<PathBuf>,
    symbol: String,
    /// Cap trades for smoke (0 = all).
    max_trades: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut mode = None;
    let mut config_dir = default_config_dir();
    let mut once = false;
    let mut replay = None;
    let mut journal = None;
    let mut symbol = "SOL".to_string();
    let mut max_trades = 0usize;
    let mut it = env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mode" => {
                let v = it.next().ok_or("--mode needs a value")?;
                mode = Some(Mode::parse(&v)?);
            }
            "--config-dir" => {
                let v = it.next().ok_or("--config-dir needs a value")?;
                config_dir = v.into();
            }
            "--once" => once = true,
            "--replay" => {
                let v = it.next().ok_or("--replay needs a path")?;
                replay = Some(v.into());
            }
            "--journal" => {
                let v = it.next().ok_or("--journal needs a path")?;
                journal = Some(v.into());
            }
            "--symbol" => {
                symbol = it.next().ok_or("--symbol needs a value")?;
            }
            "--max-trades" => {
                let v = it.next().ok_or("--max-trades needs a value")?;
                max_trades = v.parse().map_err(|_| "bad --max-trades")?;
            }
            "-h" | "--help" => {
                eprintln!(
                    "orderflowd --mode shadow|sim|live_small|live [--config-dir params] [--once]\n\
                     \t[--replay OKX_TRADES.jsonl] [--journal out.jsonl] [--symbol SOL] [--max-trades N]"
                );
                return Err("help".into());
            }
            other => return Err(format!("unknown arg {other}")),
        }
    }
    Ok(Args {
        mode,
        config_dir,
        once,
        replay,
        journal,
        symbol,
        max_trades,
    })
}

fn run_replay(args: &Args, cfg: &AppConfig) -> Result<(), String> {
    let path = args.replay.as_ref().unwrap();
    let mut trades = okx::load_jsonl_sorted(path, &args.symbol)?;
    if args.max_trades > 0 && trades.len() > args.max_trades {
        // Keep the earliest max_trades after sort (stable window), not a random head.
        trades.truncate(args.max_trades);
    }
    let mut cutter = BarCutter::new(Venue::Okx, args.symbol.clone());
    let journal = match &args.journal {
        Some(p) => Some(JsonlJournal::create(p)?),
        None => None,
    };
    let mut closed_n = 0u64;
    for t in &trades {
        for ev in cutter.push(t) {
            if let CutEvent::Closed(bar) = ev {
                closed_n += 1;
                if let Some(j) = &journal {
                    j.append_closed(&bar, cutter.quality())?;
                }
            }
        }
    }
    // Replay ends: do not flush forming as closed (that minute is still forming in live).
    // For offline completeness tests, callers may want flush — keep forming dropped here
    // to match live contract (last incomplete minute is not a decision bar).
    let q = cutter.quality();
    println!(
        "{}",
        serde_json::json!({
            "level": "info",
            "event": "replay_done",
            "venue": "okx",
            "symbol": args.symbol,
            "execution_venue": "okx",
            "resonance": format!("{:?}", cfg.runtime.resonance).to_ascii_lowercase(),
            "trades": q.trades_seen,
            "bars_closed": q.bars_closed,
            "closed_emitted": closed_n,
            "late_trade": q.late_trade,
            "out_of_order": q.out_of_order,
            "gap_minutes": q.gap_minutes,
            "reconnect": q.reconnect,
            "forming_open_ms": cutter.forming().map(|b| b.open_ms),
            "journal": journal.as_ref().map(|j| j.path().display().to_string()),
            "note": "stage 1: OKX event-time 1m bars; closed never rewritten; live still gated",
        })
    );
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) if e == "help" => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let cfg = match AppConfig::load(&args.config_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}",
                serde_json::json!({"level":"error","event":"config_error","error":e})
            );
            return ExitCode::from(2);
        }
    };
    let mode = args.mode.unwrap_or(cfg.runtime.mode_default);
    let decision = boot_decision(mode, &cfg);
    let level = if decision.ok { "info" } else { "error" };
    println!("{}", json_log(level, &decision));

    if mode.is_live() {
        let _ = submit_live_open(mode, &cfg);
        return ExitCode::from(2);
    }

    if args.replay.is_some() {
        if let Err(e) = run_replay(&args, &cfg) {
            eprintln!(
                "{}",
                serde_json::json!({"level":"error","event":"replay_error","error":e})
            );
            return ExitCode::from(2);
        }
        return ExitCode::SUCCESS;
    }

    if !args.once {
        println!(
            "{}",
            serde_json::json!({
                "level": "info",
                "event": "idle",
                "note": "stage 1: OKX 1m cutter + JSONL replay ready; use --replay PATH. WS live subscribe is next. Binance/Bybit still stubs.",
            })
        );
    }
    ExitCode::SUCCESS
}
