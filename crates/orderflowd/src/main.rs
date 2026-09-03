use std::env;
use std::process::ExitCode;

use orderflow_domain::{boot_decision, default_config_dir, json_log, AppConfig, Mode};
use orderflow_exec::submit_live_open;

struct Args {
    mode: Option<Mode>,
    config_dir: std::path::PathBuf,
    once: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut mode = None;
    let mut config_dir = default_config_dir();
    let mut once = false;
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
            "-h" | "--help" => {
                eprintln!(
                    "orderflowd --mode shadow|sim|live_small|live [--config-dir params] [--once]"
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
    })
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

    if !args.once {
        println!(
            "{}",
            serde_json::json!({
                "level": "info",
                "event": "idle",
                "note": "stage 0: no WS, no footprint, no orders. next: stage 1 OKX 1m bars",
            })
        );
    }
    ExitCode::SUCCESS
}
