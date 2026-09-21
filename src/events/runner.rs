use super::engine::{Engine, Limits, Metrics, Outcome, Policy, Propagation, State};
use super::problem::Problem;
use super::proof::{self, Certificate, Fact, Rule};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, clap::Args, Serialize)]
pub struct Config {
    #[arg(long)]
    pub maximum: usize,
    /// Common portfolio search deadline; root preparation is timed separately.
    #[arg(long, default_value_t = 30.0)]
    pub seconds: f64,
    /// Root preparation deadline, including table construction; zero is unlimited.
    #[arg(long, default_value_t = 120.0)]
    pub root_seconds: f64,
    /// Zero uses all logical processors.
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    /// Optional single-policy control; otherwise alternate the two policies.
    #[arg(long, value_enum)]
    pub policy: Option<Policy>,
    /// Use the stated universal membership lemma as an explicit external premise.
    #[arg(long)]
    pub universal_members: bool,
    /// Enable the 39 cross-group positive prime clauses.
    #[arg(long)]
    pub prime_clauses: bool,
    /// At least seven members in {2,4,...,384}.
    #[arg(long)]
    pub even_bound: bool,
    #[arg(long)]
    pub probes: bool,
    /// Saturate the shared root with all short-domain covers and renewed chains.
    #[arg(long)]
    pub root_strengthen: bool,
    /// Additional members allowed in a propagation-only probe; zero is unlimited.
    #[arg(long, default_value_t = 2000)]
    pub probe_members: usize,
    /// Largest complete factor cover examined during shared-root strengthening.
    #[arg(long, default_value_t = 8)]
    pub root_probe_width: usize,
    /// Probe all six overlapping seed alternatives; requires U and prime clauses.
    #[arg(long)]
    pub seed_probe: bool,
    #[arg(long, default_value_t = 1000000)]
    pub probe_case_events: usize,
    #[arg(long, default_value_t = 12000)]
    pub probe_node_events: usize,
    /// Total probe events: shared root first, then the remainder divided among lanes.
    #[arg(long, default_value_t = 200000000)]
    pub probe_events: usize,
    #[arg(long, default_value_t = 2)]
    pub probe_domains: usize,
    #[arg(long, default_value_t = 2)]
    pub absence_targets: usize,
    /// Pure prime-chain steps per root pass; zero disables this accelerator.
    #[arg(long, default_value_t = 50000000)]
    pub prime_chain_steps: usize,
    /// Per-lane DFS node limit; zero is unlimited.
    #[arg(long, default_value_t = 0)]
    pub node_limit: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            maximum: 113,
            seconds: 30.0,
            root_seconds: 120.0,
            threads: 1,
            policy: None,
            universal_members: false,
            prime_clauses: false,
            even_bound: false,
            probes: false,
            root_strengthen: false,
            probe_members: 2000,
            root_probe_width: 8,
            seed_probe: false,
            probe_case_events: 1000000,
            probe_node_events: 12000,
            probe_events: 200000000,
            probe_domains: 2,
            absence_targets: 2,
            prime_chain_steps: 50000000,
            node_limit: 0,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct LaneReport {
    pub lane: usize,
    pub policy: Policy,
    pub status: String,
    pub seconds: f64,
    pub metrics: Metrics,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub engine: &'static str,
    pub config: Config,
    pub status: String,
    pub evidence: String,
    pub scope: &'static str,
    pub reason: Option<String>,
    pub example: Vec<usize>,
    pub winner: Option<usize>,
    pub preprocess_time: f64,
    pub shared_root_time: f64,
    pub lane_search_time: f64,
    pub verification_time: f64,
    pub peak_memory: Option<u64>,
    pub root_metrics: Metrics,
    pub lanes: Vec<LaneReport>,
    pub proof_events: usize,
    pub compressed_proof_events: usize,
    #[serde(skip)]
    pub certificate: Option<Certificate>,
}
fn deadline(start: Instant, seconds: f64) -> Result<Option<Instant>, String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("seconds must be finite and non-negative".into());
    }
    if seconds == 0.0 {
        Ok(None)
    } else {
        start
            .checked_add(Duration::try_from_secs_f64(seconds).map_err(|e| e.to_string())?)
            .map(Some)
            .ok_or("deadline overflow".into())
    }
}
fn status(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Yes(_) => "YES",
        Outcome::No(_) => "NO",
        Outcome::Unknown => "UNKNOWN",
        Outcome::Error(_) => "ERROR",
    }
}
fn engine(
    p: Arc<Problem>,
    cfg: Config,
    lane: usize,
    limits: Limits,
    probe_remaining: usize,
) -> Engine {
    let policy = cfg.policy.unwrap_or(if lane.is_multiple_of(2) {
        Policy::Mrv
    } else {
        Policy::StrictMaximumRowFirst
    });
    Engine {
        p,
        cfg,
        policy,
        lane,
        limits,
        metrics: Metrics::default(),
        probe_remaining,
    }
}
fn accept(report: &mut Report, outcome: Outcome, cert: Option<Certificate>) {
    report.status = status(&outcome).into();
    match outcome {
        Outcome::Yes(values) => {
            if super::validate(report.config.maximum, &values) {
                report.example = values;
                report.evidence = "RAW_VALIDATED_YES".into();
            } else {
                report.status = "ERROR".into();
                report.reason = Some("coordinator raw YES validation failed".into());
            }
        }
        Outcome::No(_) => {
            let Some(cert) = cert else {
                report.status = "ERROR".into();
                report.reason = Some("missing complete-root certificate".into());
                return;
            };
            let started = Instant::now();
            let checked = proof::verify(&cert, true);
            report.verification_time = started.elapsed().as_secs_f64();
            report.compressed_proof_events = cert.nodes.len();
            match checked {
                Ok(external) => {
                    report.evidence = if external {
                        "SOLVER_NO_EXTERNAL_LEMMAS"
                    } else {
                        "VERIFIED_NO"
                    }
                    .into();
                    report.certificate = Some(cert);
                }
                Err(error) => {
                    report.status = "ERROR".into();
                    report.reason = Some(error);
                }
            }
        }
        Outcome::Error(error) => report.reason = Some(error),
        Outcome::Unknown => {
            report.reason = Some(
                "deadline, cancellation, or node budget reached before a complete result".into(),
            )
        }
    }
}

pub fn run(cfg: Config, interrupted: Arc<AtomicBool>) -> Result<Report, String> {
    if cfg.root_strengthen && cfg.root_probe_width < 2 {
        return Err("root-probe-width must be at least two".into());
    }
    if cfg.seed_probe && !(cfg.probes && cfg.universal_members && cfg.prime_clauses) {
        return Err("seed-probe requires probes, universal-members, and prime-clauses".into());
    }
    let started = Instant::now();
    let root_deadline = deadline(started, cfg.root_seconds)?;
    deadline(started, cfg.seconds)?;
    let p = Arc::new(Problem::new(cfg.maximum)?);
    let preprocess_time = started.elapsed().as_secs_f64();
    let root_started = Instant::now();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut root = State::new(&p);
    let mut prep = engine(
        Arc::clone(&p),
        cfg.clone(),
        0,
        Limits {
            deadline: root_deadline,
            cancel: Arc::clone(&cancel),
            interrupted: Arc::clone(&interrupted),
        },
        cfg.probe_events,
    );
    let mut report = Report {
        engine: "events-v2",
        config: cfg.clone(),
        status: "UNKNOWN".into(),
        evidence: "NONE".into(),
        scope: "complete_original_problem",
        reason: None,
        example: vec![],
        winner: None,
        preprocess_time,
        shared_root_time: 0.0,
        lane_search_time: 0.0,
        verification_time: 0.0,
        peak_memory: None,
        root_metrics: Metrics::default(),
        lanes: vec![],
        proof_events: 0,
        compressed_proof_events: 0,
        certificate: None,
    };
    let ready = if cfg.maximum == 1 {
        Err(Outcome::No(root.node(Fact::False, Rule::Boundary, vec![])))
    } else if cfg.maximum == 2 {
        Err(Outcome::Yes(vec![2]))
    } else {
        let result = match prep.initialize(&mut root) {
            Err(id) => Propagation::Conflict(id),
            Ok(()) => prep.quiesce(&mut root),
        };
        let result = if result == Propagation::Quiet {
            prep.prime_chains(&mut root)
        } else {
            result
        };
        let result = if result == Propagation::Quiet && cfg.root_strengthen {
            prep.strengthen_root(&mut root)
        } else {
            result
        };
        match result {
            Propagation::Quiet => Ok(()),
            Propagation::Paused => Err(Outcome::Unknown),
            Propagation::Conflict(id) => Err(Outcome::No(id)),
        }
    };
    report.shared_root_time = root_started.elapsed().as_secs_f64();
    prep.metrics.proof_events = root.proof.len();
    let search_probe_events = prep.probe_remaining;
    report.root_metrics = prep.metrics;
    if let Err(outcome) = ready {
        report.proof_events = root.proof.len();
        let cert = if let Outcome::No(id) = outcome {
            Some(root.proof.certificate(cfg.maximum, id))
        } else {
            None
        };
        accept(&mut report, outcome, cert);
        return Ok(report);
    }
    root.freeze();
    let threads = if cfg.threads == 0 {
        std::thread::available_parallelism().map_or(1, usize::from)
    } else {
        cfg.threads
    };
    let search_started = Instant::now();
    let search_deadline = deadline(search_started, cfg.seconds)?;
    let (tx, rx) = mpsc::channel();
    let mut handles = vec![];
    for lane in 0..threads {
        let mut state = root.clone();
        let tx = tx.clone();
        let allowance =
            search_probe_events / threads + usize::from(lane < search_probe_events % threads);
        let mut worker = engine(
            Arc::clone(&p),
            cfg.clone(),
            lane,
            Limits {
                deadline: search_deadline,
                cancel: Arc::clone(&cancel),
                interrupted: Arc::clone(&interrupted),
            },
            allowance,
        );
        let handle = std::thread::Builder::new()
            .name(format!("apa-events-{lane}"))
            .spawn(move || {
                let start = Instant::now();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker.dfs(&mut state)
                }))
                .unwrap_or_else(|_| {
                    worker.limits.cancel.store(true, Ordering::Release);
                    Outcome::Error("worker panicked".into())
                });
                let outcome = match outcome {
                    Outcome::No(id) if state.proof.get(id).scope != 0 => {
                        Outcome::Error("lane returned a conditional NO".into())
                    }
                    other => other,
                };
                worker.metrics.proof_events = state.proof.len();
                // Send the arena before compression so the coordinator can cancel
                // other lanes as soon as a full refutation has been found.
                let lane_report = LaneReport {
                    lane,
                    policy: worker.policy,
                    status: status(&outcome).into(),
                    seconds: start.elapsed().as_secs_f64(),
                    metrics: worker.metrics,
                };
                let _ = tx.send((outcome, state.proof, lane_report));
            });
        match handle {
            Ok(handle) => handles.push(handle),
            Err(error) => {
                cancel.store(true, Ordering::Release);
                for handle in handles {
                    let _ = handle.join();
                }
                return Err(format!("start lane {lane}: {error}"));
            }
        }
    }
    drop(tx);
    let mut winning = None;
    let mut failure = None;
    for (outcome, arena, lane_report) in rx {
        match &outcome {
            Outcome::Error(e) => {
                failure = Some(e.clone());
                cancel.store(true, Ordering::Release);
            }
            Outcome::Yes(v) if !super::validate(cfg.maximum, v) => {
                failure = Some("invalid lane YES".into());
                cancel.store(true, Ordering::Release);
            }
            Outcome::No(_) | Outcome::Yes(_) if winning.is_none() => {
                report.winner = Some(lane_report.lane);
                cancel.store(true, Ordering::Release);
                winning = Some((outcome, arena));
            }
            _ => {}
        }
        report.lanes.push(lane_report);
    }
    for handle in handles {
        if handle.join().is_err() {
            failure = Some("worker panicked".into());
        }
    }
    report.lane_search_time = search_started.elapsed().as_secs_f64();
    report.lanes.sort_by_key(|l| l.lane);
    if let Some(error) = failure {
        accept(&mut report, Outcome::Error(error), None);
    } else if let Some((outcome, arena)) = winning {
        report.proof_events = arena.len();
        let cert = if let Outcome::No(id) = outcome {
            Some(arena.certificate(cfg.maximum, id))
        } else {
            None
        };
        accept(&mut report, outcome, cert);
    } else {
        accept(&mut report, Outcome::Unknown, None);
    }
    Ok(report)
}
