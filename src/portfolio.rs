use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use serde::Serialize;

use crate::solver::{
    Compatibility, DemandOrder, LaneConfig, LaneOutcome, LaneReport, Problem, Solver, State,
};

#[derive(Debug, Serialize)]
pub struct PortfolioReport {
    pub maximum: usize,
    pub status: String,
    pub seconds: f64,
    pub threads: usize,
    pub winner: Option<usize>,
    pub example: Vec<usize>,
    pub lanes: Vec<LaneReport>,
}

fn strategy(lane: usize) -> LaneConfig {
    const WINDOWS: [usize; 6] = [32, 64, 64, 32, 128, 16];
    let compatibility = if lane % 24 == 11 {
        Compatibility::Full
    } else if lane % 12 == 5 {
        Compatibility::Adaptive
    } else {
        Compatibility::None
    };
    LaneConfig {
        lane,
        lookahead: WINDOWS[(lane / 2) % WINDOWS.len()],
        order: if lane.is_multiple_of(2) {
            DemandOrder::Insertion
        } else {
            DemandOrder::MaximumFirst
        },
        compatibility,
        reverse_witnesses: lane % 4 >= 2,
        learn_nogoods: true,
        early_prime_filter: true,
        rotation: lane,
    }
}

fn validate_example(problem: &Problem, example: &[usize]) -> bool {
    if example.is_empty() || example.last() != Some(&problem.maximum) {
        return false;
    }
    let mut member = vec![false; problem.maximum + 1];
    for &v in example {
        if v == 0 || v > problem.maximum || member[v] {
            return false;
        }
        member[v] = true;
    }
    for &a in example {
        for &b in example {
            let sum = a + b;
            let mut witnessed = false;
            let mut d = 1;
            while d * d <= sum {
                if sum % d == 0 {
                    let e = sum / d;
                    if e <= problem.maximum && member[d] && member[e] {
                        witnessed = true;
                        break;
                    }
                }
                d += 1;
            }
            if !witnessed {
                return false;
            }
        }
    }
    true
}

pub fn run_portfolio(
    problem: Arc<Problem>,
    root: State,
    threads: usize,
    seconds: f64,
    interrupted: Arc<AtomicBool>,
) -> Result<PortfolioReport, String> {
    if threads == 0 {
        return Err("threads must be positive".into());
    }
    root.validate(problem.maximum)?;
    let started = Instant::now();
    let cancel = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::with_capacity(threads);

    for lane in 0..threads {
        let tx = sender.clone();
        let lane_problem = Arc::clone(&problem);
        let lane_root = root.clone();
        let lane_cancel = Arc::clone(&cancel);
        let lane_interrupted = Arc::clone(&interrupted);
        handles.push(std::thread::spawn(move || {
            let config = strategy(lane);
            let report = Solver::new(lane_problem, config, seconds, lane_cancel, lane_interrupted)
                .run(lane_root);
            let _ = tx.send(report);
        }));
    }
    drop(sender);

    let mut reports = Vec::with_capacity(threads);
    let mut winner = None;
    let mut winning_status = None;
    let mut example = Vec::new();
    while reports.len() < threads {
        let report = receiver
            .recv()
            .map_err(|_| "worker channel closed unexpectedly".to_string())?;
        match report.outcome {
            LaneOutcome::No if winner.is_none() => {
                winner = Some(report.config.lane);
                winning_status = Some("NO");
                cancel.store(true, Ordering::Release);
            }
            LaneOutcome::Yes if winner.is_none() => {
                if !validate_example(&problem, &report.example) {
                    cancel.store(true, Ordering::Release);
                    return Err(format!(
                        "lane {} produced an invalid example",
                        report.config.lane
                    ));
                }
                example = report.example.clone();
                winner = Some(report.config.lane);
                winning_status = Some("YES");
                cancel.store(true, Ordering::Release);
            }
            _ => {}
        }
        reports.push(report);
    }
    for handle in handles {
        handle
            .join()
            .map_err(|_| "worker thread panicked".to_string())?;
    }
    reports.sort_by_key(|r| r.config.lane);
    let status = if interrupted.load(Ordering::Acquire) {
        "CANCELLED"
    } else {
        winning_status.unwrap_or("UNKNOWN")
    };
    Ok(PortfolioReport {
        maximum: problem.maximum,
        status: status.into(),
        seconds: started.elapsed().as_secs_f64(),
        threads,
        winner,
        example,
        lanes: reports,
    })
}
