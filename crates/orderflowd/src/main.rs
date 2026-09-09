use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use orderflow_book::{load_jsonl, BookConfig, BookEngine};
use orderflow_context::{
    BarIn, ContextConfig, ContextEngine, RegimeInputs, ResonanceBook, VenueDir,
};
use orderflow_domain::{
    boot_decision, default_config_dir, json_log, AppConfig, Mode, Venue, VenueRole,
};
use orderflow_exec::{run_private_replay, run_sim_fixture, submit_live_open};
use orderflow_footprint::{FootprintConfig, FootprintEngine};
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
    book_replay: Option<PathBuf>,
    regime_replay: Option<PathBuf>,
    sim_fixture: Option<PathBuf>,
    kill_switch: Option<PathBuf>,
    private_replay: Option<PathBuf>,
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
    let mut book_replay = None;
    let mut regime_replay = None;
    let mut sim_fixture = None;
    let mut kill_switch = None;
    let mut private_replay = None;
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
            "--book-replay" => {
                let v = it.next().ok_or("--book-replay needs a path")?;
                book_replay = Some(v.into());
            }
            "--regime-replay" => {
                let v = it.next().ok_or("--regime-replay needs a path")?;
                regime_replay = Some(v.into());
            }
            "--sim-fixture" => {
                let v = it.next().ok_or("--sim-fixture needs a path")?;
                sim_fixture = Some(v.into());
            }
            "--kill-switch" => {
                let v = it.next().ok_or("--kill-switch needs a path")?;
                kill_switch = Some(v.into());
            }
            "--private-replay" => {
                let v = it.next().ok_or("--private-replay needs a path")?;
                private_replay = Some(v.into());
            }
            "-h" | "--help" => {
                eprintln!(
                    "orderflowd --mode shadow|sim|live_small|live [--config-dir params] [--once]\n\
                     \t[--replay PATH] [--venue okx|binance|bybit]\n\
                     \t[--replay-okx PATH] [--replay-binance PATH] [--replay-bybit PATH]\n\
                     \t[--journal out.jsonl] [--symbol SOL] [--max-trades N]\n\
                     \t[--book-replay PATH] [--regime-replay PATH]\n\
                     \t[--sim-fixture PATH] [--kill-switch PATH] [--private-replay PATH]\n\
                     Resonance stays off (still recorded). Replay venue is not the execution venue.\n\
                     --book-replay applies to --venue (default okx). Toxic books never copy prices onto OKX.\n\
                     --sim-fixture matches on the OKX book only. --private-replay decodes OKX private frames.\n\
                     No API keys. Live still double-locked. Default mode is shadow."
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
        book_replay,
        regime_replay,
        sim_fixture,
        kill_switch,
        private_replay,
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

fn footprint_config(cfg: &AppConfig, symbol: &str) -> Result<FootprintConfig, String> {
    match symbol.to_ascii_uppercase().as_str() {
        "SOL" => FootprintConfig::from_symbol(&cfg.sol),
        "SUI" => FootprintConfig::from_symbol(&cfg.sui),
        other => Err(format!("unknown symbol {other}; expected SOL|SUI")),
    }
}

fn context_config(cfg: &AppConfig, symbol: &str) -> Result<ContextConfig, String> {
    let p = match symbol.to_ascii_uppercase().as_str() {
        "SOL" => &cfg.sol,
        "SUI" => &cfg.sui,
        other => return Err(format!("unknown symbol {other}; expected SOL|SUI")),
    };
    ContextConfig::from_symbol(p, cfg.runtime.resonance_k)
}

fn book_engine(venue: Venue, symbol: &str) -> Result<BookEngine, String> {
    let cfg = match symbol.to_ascii_uppercase().as_str() {
        "SOL" => BookConfig::sol(),
        "SUI" => BookConfig::sui(),
        other => return Err(format!("unknown symbol {other}; expected SOL|SUI")),
    };
    let role = match venue {
        Venue::Okx => VenueRole::Execution,
        Venue::Binance | Venue::Bybit => VenueRole::Resonance,
    };
    Ok(BookEngine::new(venue, role, cfg))
}

enum ReplayEv {
    Trade(orderflow_domain::Trade),
    Book(String),
}

fn merge_events(trades: Vec<orderflow_domain::Trade>, books: Vec<(i64, String)>) -> Vec<ReplayEv> {
    let mut tagged: Vec<(i64, u8, ReplayEv)> = Vec::new();
    for t in trades {
        tagged.push((t.event_ts_ms, 0, ReplayEv::Trade(t)));
    }
    for (ts, line) in books {
        tagged.push((ts, 1, ReplayEv::Book(line)));
    }
    tagged.sort_by_key(|(ts, k, _)| (*ts, *k));
    tagged.into_iter().map(|(_, _, ev)| ev).collect()
}

fn stack_prices(fp: &orderflow_footprint::FootprintBar) -> Vec<f64> {
    let mut v = fp.dale.buy_imb_prices.clone();
    v.extend(fp.dale.sell_imb_prices.iter().copied());
    v
}

fn push_context(
    ctx_eng: &mut ContextEngine,
    res_book: &mut ResonanceBook,
    fp: &orderflow_footprint::FootprintBar,
    regime: &RegimeInputs,
    journal: Option<&JsonlJournal>,
) -> Result<u64, String> {
    let bar = BarIn::from_footprint(fp);
    res_book.insert(VenueDir::from_delta(
        fp.venue,
        fp.open_ms,
        fp.delta,
        bar.stack_sign(),
        true,
    ));
    let snap = ctx_eng.push(bar, regime);
    if let Some(j) = journal {
        j.append_json(&serde_json::json!({
            "event": "context_closed",
            "context": snap,
        }))?;
    }
    Ok(1)
}

fn run_one_replay(
    venue: Venue,
    trade_path: Option<&Path>,
    book_path: Option<&Path>,
    args: &Args,
    cfg: &AppConfig,
    journal: Option<JsonlJournal>,
    regime: &RegimeInputs,
    res_book: &mut ResonanceBook,
) -> Result<u64, String> {
    let mut trades = if let Some(path) = trade_path {
        load_dump_sorted(venue, path, &args.symbol)?
    } else {
        Vec::new()
    };
    if args.max_trades > 0 && trades.len() > args.max_trades {
        trades.truncate(args.max_trades);
    }
    let books = if let Some(path) = book_path {
        load_jsonl(venue, path)?
    } else {
        Vec::new()
    };
    if trades.is_empty() && books.is_empty() {
        return Err("no trades and no book frames".into());
    }
    let mut ctx_eng = ContextEngine::new(context_config(cfg, &args.symbol)?);
    let mut context_n = 0u64;

    let mut book_eng = if book_path.is_some() {
        Some(book_engine(venue, &args.symbol)?)
    } else {
        None
    };
    let mut closed_n = 0u64;
    let mut stack_dale = 0u64;
    let mut stack_valtos = 0u64;
    let mut book_closed_n = 0u64;
    let mut last_quality = orderflow_domain::QualityVector::default();

    if trades.is_empty() {
        if let Some(book) = book_eng.as_mut() {
            for (_, line) in &books {
                book.apply_frame(line).map_err(|e| e.to_string())?;
            }
            let snap = book.freeze_bar(None, &[]);
            book.apply_quality(&mut last_quality);
            book_closed_n += 1;
            if let Some(j) = &journal {
                j.append_json(&serde_json::json!({
                    "event": "book_closed",
                    "book": snap,
                }))?;
            }
        }
    } else {
        let fp_cfg = footprint_config(cfg, &args.symbol)?;
        let mut eng = FootprintEngine::new(venue, args.symbol.clone(), fp_cfg);
        let events = merge_events(trades, books);
        for ev in events {
            match ev {
                ReplayEv::Book(line) => {
                    if let Some(book) = book_eng.as_mut() {
                        book.apply_frame(&line).map_err(|e| e.to_string())?;
                    }
                }
                ReplayEv::Trade(t) => {
                    debug_assert_eq!(t.venue, venue);
                    if let Some(book) = book_eng.as_mut() {
                        book.apply_trade(&t);
                    }
                    for closed in eng.push(&t) {
                        closed_n += 1;
                        if closed.footprint.dale.aligned {
                            stack_dale += 1;
                        }
                        if closed.footprint.valtos.aligned {
                            stack_valtos += 1;
                        }
                        last_quality = eng.cutter().quality().clone();
                        context_n += push_context(
                            &mut ctx_eng,
                            res_book,
                            &closed.footprint,
                            regime,
                            journal.as_ref(),
                        )?;
                        if let Some(book) = book_eng.as_mut() {
                            book.apply_quality(&mut last_quality);
                            let stacks = stack_prices(&closed.footprint);
                            let snap = book.freeze_bar(closed.footprint.poc, &stacks);
                            book_closed_n += 1;
                            if let Some(j) = &journal {
                                j.append_closed(&closed.bar, &last_quality)?;
                                j.append_json(&serde_json::json!({
                                    "event": "footprint_closed",
                                    "footprint": closed.footprint,
                                }))?;
                                j.append_json(&serde_json::json!({
                                    "event": "book_closed",
                                    "book": snap,
                                }))?;
                            }
                        } else if let Some(j) = &journal {
                            j.append_closed(&closed.bar, eng.cutter().quality())?;
                            j.append_json(&serde_json::json!({
                                "event": "footprint_closed",
                                "footprint": closed.footprint,
                            }))?;
                        }
                    }
                }
            }
        }
        last_quality = eng.cutter().quality().clone();
        if let Some(book) = &book_eng {
            book.apply_quality(&mut last_quality);
        }
        let q = last_quality.clone();
        let book_ok = book_eng.as_ref().map(|b| b.health().is_ok());
        let dom = book_eng
            .as_ref()
            .map(|b| b.freeze_1m(None, &[]).dom_entries_allowed);
        let book_health = book_eng.as_ref().map(|b| b.health());
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
                "forming_open_ms": eng.cutter().forming().map(|b| b.open_ms),
                "footprint_wired": true,
                "book_wired": book_eng.is_some(),
                "book_ok": book_ok,
                "book_health": book_health,
                "dom_entries_allowed": dom,
                "okx_book_ok": q.okx_book_ok,
                "binance_book_ok": q.binance_book_ok,
                "bybit_book_ok": q.bybit_book_ok,
                "book_closed_emitted": book_closed_n,
                "dale_aligned_stacks": stack_dale,
                "valtos_aligned_stacks": stack_valtos,
                "context_closed_emitted": context_n,
                "context_wired": true,
                "journal": journal.as_ref().map(|j| j.path().display().to_string()),
                "note": "stage 4: context/regime/resonance fields; 300∥400 parallel; unfinished not entry; resonance off; live still gated",
            })
        );
        return Ok(context_n);
    }

    let book_ok = book_eng.as_ref().map(|b| b.health().is_ok());
    let dom = book_eng
        .as_ref()
        .map(|b| b.freeze_1m(None, &[]).dom_entries_allowed);
    let book_health = book_eng.as_ref().map(|b| b.health());
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
            "trades": last_quality.trades_seen,
            "bars_closed": last_quality.bars_closed,
            "closed_emitted": closed_n,
            "footprint_wired": false,
            "book_wired": book_eng.is_some(),
            "book_ok": book_ok,
            "book_health": book_health,
            "dom_entries_allowed": dom,
            "okx_book_ok": last_quality.okx_book_ok,
            "binance_book_ok": last_quality.binance_book_ok,
            "bybit_book_ok": last_quality.bybit_book_ok,
            "book_closed_emitted": book_closed_n,
            "context_closed_emitted": context_n,
            "context_wired": true,
            "journal": journal.as_ref().map(|j| j.path().display().to_string()),
            "note": "stage 4: book-only replay; live still gated; resonance off",
        })
    );
    Ok(context_n)
}

fn run_replays(args: &Args, cfg: &AppConfig) -> Result<(), String> {
    let mut jobs = collect_replay_jobs(args);
    if jobs.is_empty() {
        if args.book_replay.is_some() {
            jobs.push((args.venue, PathBuf::new()));
        } else {
            return Err("no replay path".into());
        }
    }
    let regime = if let Some(p) = &args.regime_replay {
        RegimeInputs::load_jsonl(p)?
    } else {
        RegimeInputs::default()
    };
    let mut res_book = ResonanceBook::default();
    let multi = jobs.len() > 1;
    let mut okx_journal_path: Option<PathBuf> = None;
    for (venue, path) in &jobs {
        let journal = journal_for(args.journal.as_deref(), *venue, multi)?;
        if *venue == Venue::Okx {
            okx_journal_path = journal.as_ref().map(|j| j.path().to_path_buf());
        }
        let trades = if path.as_os_str().is_empty() {
            None
        } else {
            Some(path.as_path())
        };
        let book = if *venue == args.venue {
            args.book_replay.as_deref()
        } else {
            None
        };
        run_one_replay(
            *venue,
            trades,
            book,
            args,
            cfg,
            journal,
            &regime,
            &mut res_book,
        )?;
    }
    let minutes = res_book.okx_minutes();
    let mut resonance_n = 0u64;
    if let Some(path) = okx_journal_path.or_else(|| args.journal.clone()) {
        if !minutes.is_empty() {
            let j = JsonlJournal::open_append(path)?;
            for t in &minutes {
                let snap = res_book.snapshot(*t, cfg.runtime.resonance, cfg.runtime.resonance_k);
                j.append_json(&serde_json::json!({
                    "event": "resonance_closed",
                    "resonance": snap,
                }))?;
                resonance_n += 1;
            }
        }
    } else {
        resonance_n = minutes.len() as u64;
    }
    if !minutes.is_empty() {
        let sample = res_book.snapshot(minutes[0], cfg.runtime.resonance, cfg.runtime.resonance_k);
        println!(
            "{}",
            serde_json::json!({
                "level": "info",
                "event": "resonance_done",
                "minutes": minutes.len(),
                "resonance_closed_emitted": resonance_n,
                "mode": format!("{:?}", cfg.runtime.resonance).to_ascii_lowercase(),
                "k": cfg.runtime.resonance_k,
                "used_for_entry": sample.used_for_entry,
                "copied_price_onto_okx": false,
                "note": "stage 4: resonance recorded, mode off, never copies peer prices onto OKX",
            })
        );
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

    if let Some(path) = &args.sim_fixture {
        match run_sim_fixture(path, &cfg, args.kill_switch.as_deref()) {
            Ok(v) => {
                println!("{v}");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    serde_json::json!({"level":"error","event":"sim_error","error":e})
                );
                return ExitCode::from(2);
            }
        }
    }

    if let Some(path) = &args.private_replay {
        match run_private_replay(path, &cfg) {
            Ok(v) => {
                println!("{v}");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    serde_json::json!({"level":"error","event":"private_replay_error","error":e})
                );
                return ExitCode::from(2);
            }
        }
    }

    if !collect_replay_jobs(&args).is_empty() || args.book_replay.is_some() {
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
                "note": "stage 7: OKX private decode, shadow default. --private-replay PATH. Live double-locked.",
            })
        );
    }
    ExitCode::SUCCESS
}
