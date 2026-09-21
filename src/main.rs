mod events;
mod portfolio;
mod solver;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};
use portfolio::{PortfolioReport, run_portfolio};
use solver::{Problem, State};

#[derive(Parser)]
#[command(
    name = "apa-exact",
    about = "Exact parallel search for A+A contained in A*A"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Opt-in event-driven engine with scoped arithmetic certificates.
    SolveEvents {
        #[command(flatten)]
        config: events::Config,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        proof: Option<PathBuf>,
    },
    /// Independently replay an event-engine root certificate.
    VerifyEvents {
        #[arg(long)]
        proof: PathBuf,
        /// Explicitly trust named universal lemmas absent their proof bundles.
        #[arg(long)]
        allow_external_lemmas: bool,
    },
    /// Solve one fixed maximum with a strategy-diverse thread portfolio.
    Solve {
        #[arg(long)]
        maximum: usize,
        #[arg(long)]
        state: Option<PathBuf>,
        /// Wall-clock limit in seconds; zero means no time limit.
        #[arg(long, default_value_t = 0.0)]
        seconds: f64,
        /// Worker threads; zero uses every logical processor.
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long)]
        output: Option<PathBuf>,
        /// Enable the maximum-deletion lemma using a completely excluded prefix.
        #[arg(long)]
        complete_prefix_base: Option<usize>,
    },
    /// Process candidates in order, stopping at the first non-NO result.
    Campaign {
        #[arg(long)]
        candidates: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Optional directory containing <n>/root.state.txt files.
        #[arg(long)]
        roots: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        start_after: usize,
        #[arg(long, default_value_t = 0.0)]
        seconds: f64,
        #[arg(long, default_value_t = 0)]
        threads: usize,
        /// Enable the maximum-deletion lemma using a completely excluded prefix.
        #[arg(long)]
        complete_prefix_base: Option<usize>,
    },
}

fn worker_count(requested: usize) -> usize {
    if requested != 0 {
        requested
    } else {
        std::thread::available_parallelism().map_or(1, usize::from)
    }
}

fn load_state(problem: &Problem, path: Option<&Path>) -> Result<State, String> {
    match path {
        Some(path) => State::from_file(problem.maximum, path),
        None => State::prepared(problem),
    }
}

fn solve_one(
    maximum: usize,
    state_path: Option<&Path>,
    seconds: f64,
    threads: usize,
    complete_prefix_base: Option<usize>,
    interrupted: Arc<AtomicBool>,
) -> Result<PortfolioReport, String> {
    if !(2..=1_000_000).contains(&maximum) {
        return Err("maximum must be in [2, 1000000]".into());
    }
    if seconds < 0.0 || !seconds.is_finite() {
        return Err("seconds must be finite and non-negative".into());
    }
    let problem = Arc::new(Problem::new(maximum));
    let mut state = load_state(&problem, state_path)?;
    if let Some(base) = complete_prefix_base {
        state.enable_maximum_deletion(&problem, base)?;
    }
    run_portfolio(problem, state, worker_count(threads), seconds, interrupted)
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n";
    fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let interrupted = Arc::new(AtomicBool::new(false));
    let handler_flag = Arc::clone(&interrupted);
    ctrlc::set_handler(move || handler_flag.store(true, Ordering::Release))
        .map_err(|e| format!("install Ctrl-C handler: {e}"))?;

    match cli.command {
        Command::SolveEvents {
            config,
            output,
            proof,
        } => {
            let report = events::run(config, Arc::clone(&interrupted))?;
            if let (Some(path), Some(certificate)) = (proof, &report.certificate) {
                write_json(&path, certificate)?;
            }
            if let Some(path) = output {
                write_json(&path, &report)?;
            }
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|e| e.to_string())?
            );
            if report.status == "ERROR" {
                return Err(report.reason.unwrap_or_default());
            }
        }
        Command::VerifyEvents {
            proof,
            allow_external_lemmas,
        } => {
            let certificate: events::proof::Certificate =
                serde_json::from_slice(&fs::read(&proof).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let external = events::proof::verify(&certificate, allow_external_lemmas)?;
            println!(
                "{}",
                if external {
                    "VALID_DERIVATION_WITH_EXTERNAL_LEMMAS"
                } else {
                    "VERIFIED_NO"
                }
            );
        }
        Command::Solve {
            maximum,
            state,
            seconds,
            threads,
            output,
            complete_prefix_base,
        } => {
            let report = solve_one(
                maximum,
                state.as_deref(),
                seconds,
                threads,
                complete_prefix_base,
                Arc::clone(&interrupted),
            )?;
            if let Some(path) = output {
                write_json(&path, &report)?;
            }
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|e| e.to_string())?
            );
        }
        Command::Campaign {
            candidates,
            output,
            roots,
            start_after,
            seconds,
            threads,
            complete_prefix_base,
        } => {
            let source = fs::read_to_string(&candidates)
                .map_err(|e| format!("read {}: {e}", candidates.display()))?;
            let mut values = Vec::new();
            for token in source.split_whitespace() {
                values.push(
                    token
                        .parse::<usize>()
                        .map_err(|_| format!("bad candidate: {token}"))?,
                );
            }
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err("candidates must be strictly increasing".into());
            }
            fs::create_dir_all(&output).map_err(|e| format!("create {}: {e}", output.display()))?;
            let mut reports = Vec::new();
            for maximum in values.into_iter().filter(|n| *n > start_after) {
                if interrupted.load(Ordering::Acquire) {
                    break;
                }
                let root_path = roots
                    .as_ref()
                    .map(|base| base.join(maximum.to_string()).join("root.state.txt"))
                    .filter(|path| path.exists());
                let report = solve_one(
                    maximum,
                    root_path.as_deref(),
                    seconds,
                    threads,
                    complete_prefix_base,
                    Arc::clone(&interrupted),
                )?;
                write_json(&output.join(format!("{maximum}.json")), &report)?;
                println!(
                    "{}",
                    serde_json::to_string(&report).map_err(|e| e.to_string())?
                );
                let complete_no = report.status == "NO";
                reports.push(report);
                write_json(&output.join("campaign.json"), &reports)?;
                if !complete_no {
                    break;
                }
            }
        }
    }
    Ok(())
}
