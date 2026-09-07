use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use orderflow_clock::{BarCutter, CutEvent};
use orderflow_domain::{boot_decision, default_config_dir, json_log, AppConfig, Mode, Venue};
use orderflow_exec::submit_live_open;
use orderflow_ingest::journal::JsonlJournal;
use orderflow_ingest::load_dump_sorted;

struct Args {
    mode: Option<Mode>,
    config_dir: PathBuf,
    once: bool,
    replay: Option<PathBuf>,
    venue: Venue,
    replay_okx: Option<PathBuf>,
    replay_binance: Option<PathBuf>,
    replay_bybit: Option<PathBuf>,
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
    let mut venue = Venue::Okx;
    let mut replay_okx = None;
    let mut replay_binance = None;
    let mut replay_bybit = None;
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
            "--venue" => {
                let v = it.next().ok_or("--venue needs a value")?;
                venue = Venue::parse(&v)?;
            }
            "--replay-okx" => {
                let v = it.next().ok_or("--replay-okx needs a path")?;
                replay_okx = Some(v.into());
            }
            "--replay-binance" => {
                let v = it.next().ok_or("--replay-binance needs a path")?;
                replay_binance = Some(v.into());
            }
            "--replay-bybit" => {
                let v = it.next().ok_or("--replay-bybit needs a path")?;
                replay_bybit = Some(v.into());
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
                     \t[--replay PATH] [--venue okx|binance|bybit]\n\
                     \t[--replay-okx PATH] [--replay-binance PATH] [--replay-bybit PATH]\n\
                     \t[--journal out.jsonl] [--symbol SOL] [--max-trades N]\n\
                     Resonance stays off. Replay venue is not the execution venue."
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
        venue,
        replay_okx,
        replay_binance,
        replay_bybit,
        journal,
        symbol,
        max_trades,
    })
}

fn collect_replay_jobs(args: &Args) -> Vec<(Venue, PathBuf)> {
    let mut jobs = Vec::new();
    if let Some(p) = &args.replay {
        jobs.push((args.venue, p.clone()));
    }
    if let Some(p) = &args.replay_okx {
        jobs.push((Venue::Okx, p.clone()));
    }
    if let Some(p) = &args.replay_binance {
        jobs.push((Venue::Binance, p.clone()));
    }
    if let Some(p) = &args.replay_bybit {
        jobs.push((Venue::Bybit, p.clone()));
    }
    jobs
}

fn journal_for(
    base: Option<&Path>,
    venue: Venue,
    multi: bool,
) -> Result<Option<JsonlJournal>, String> {
    let Some(base) = base else {
        return Ok(None);
    };
    let path = if multi {
        let mut name = base
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("bars.jsonl"))
            .to_os_string();
        name.push(".");
        name.push(venue.as_str());
        base.with_file_name(name)
    } else {
        base.to_path_buf()
    };
    Ok(Some(JsonlJournal::create(path)?))
}

fn run_one_replay(
    venue: Venue,
    path: &Path,
    args: &Args,
    cfg: &AppConfig,
    journal: Option<JsonlJournal>,
) -> Result<(), String> {
    let mut trades = load_dump_sorted(venue, path, &args.symbol)?;
    if args.max_trades > 0 && trades.len() > args.max_trades {
        trades.truncate(args.max_trades);
    }
    let mut cutter = BarCutter::new(venue, args.symbol.clone());
    let mut closed_n = 0u64;
    for t in &trades {
        debug_assert_eq!(t.venue, venue);
        for ev in cutter.push(t) {
            if let CutEvent::Closed(bar) = ev {
                closed_n += 1;
                if let Some(j) = &journal {
                    j.append_closed(&bar, cutter.quality())?;
                }
            }
        }
    }
    // Replay ends: do not flush forming as closed (matches live).
    let q = cutter.quality();
    println!(
        "{}",
        serde_json::json!({
            "level": "info",
            "event": "replay_done",
            "venue": venue.as_str(),
            "symbol": args.symbol,
            "execution_venue": "okx",
            "resonance": format!("{:?}", cfg.runtime.resonance).to_ascii_lowercase(),
            "copied_price_onto_okx": false,
            "trades": q.trades_seen,
            "bars_closed": q.bars_closed,
            "closed_emitted": closed_n,
            "late_trade": q.late_trade,
            "out_of_order": q.out_of_order,
            "gap_minutes": q.gap_minutes,
            "reconnect": q.reconnect,
            "forming_open_ms": cutter.forming().map(|b| b.open_ms),
            "journal": journal.as_ref().map(|j| j.path().display().to_string()),
            "note": "stage 1b: three-venue event-time 1m bars; closed never rewritten; resonance off; live still gated",
        })
    );
    Ok(())
}

fn run_replays(args: &Args, cfg: &AppConfig) -> Result<(), String> {
    let jobs = collect_replay_jobs(args);
    if jobs.is_empty() {
        return Err("no replay path".into());
    }
    let multi = jobs.len() > 1;
    for (venue, path) in &jobs {
        let journal = journal_for(args.journal.as_deref(), *venue, multi)?;
        run_one_replay(*venue, path, args, cfg, journal)?;
    }
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

    if !collect_replay_jobs(&args).is_empty() {
        if let Err(e) = run_replays(&args, &cfg) {
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
                "note": "stage 1b: OKX/Binance/Bybit public trade adapters + bounded lanes. Use --replay PATH [--venue okx|binance|bybit]. Resonance off. Live still gated. TCP WS long-connect is not required for replay.",
            })
        );
    }
    ExitCode::SUCCESS
}
