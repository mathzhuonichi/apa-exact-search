//! Parallel propagation-only root probes against a single immutable snapshot.
//! Only parent-scoped conclusions and their proof dependencies cross workers.
use super::GROUPS;
use super::engine::{Engine, Metrics, Propagation, State};
use super::proof::{Arena, Cover, Fact, Id, Rule};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;

#[derive(Clone, Copy)]
enum Job {
    Factor(usize),
    Absence(usize),
}

struct Result {
    job: Job,
    proof: Arena,
    roots: Vec<Id>,
    conflict: bool,
    metrics: Metrics,
}

fn probe(worker: &mut Engine, state: &mut State, snapshot: &State, job: Job) -> Result {
    let cp = state.checkpoint();
    let conflict = match job {
        Job::Factor(sum) => {
            let (cases, context) = worker.cover_context(state, sum);
            let budget = worker.probe_remaining;
            worker
                .cover(state, Cover::Factor(sum), cases, context, budget)
                .err()
        }
        Job::Absence(v) => {
            let budget = worker.probe_remaining;
            let result = worker.probe(state, vec![Fact::Ban(v)], budget);
            result.conflict.and_then(|id| {
                let proof = state.node(Fact::Member(v), Rule::Discharge(result.scope), vec![id]);
                worker.force(state, v, proof).err()
            })
        }
    };
    // A cover conflict is a complete root refutation, never a single-case NO.
    if conflict.is_some() {
        worker.limits.cancel.store(true, Ordering::Release);
    }
    let roots = conflict.map_or_else(
        || state.delta(&cp).into_values().collect::<Vec<_>>(),
        |id| vec![id],
    );
    let (proof, roots) = state.proof.fragment(&roots, snapshot.proof.scopes.len());
    state.rollback(&worker.p, cp);
    state.proof = snapshot.proof.clone();
    Result {
        job,
        proof,
        roots,
        conflict: conflict.is_some(),
        metrics: std::mem::take(&mut worker.metrics),
    }
}

fn batch(
    e: &mut Engine,
    s: &mut State,
    jobs: &[(Job, usize)],
    threads: usize,
) -> std::result::Result<Propagation, String> {
    s.freeze();
    let shared_scopes = s.proof.scopes.len();
    e.metrics.root_parallel_batches += 1;
    let workers = threads.min(jobs.len());
    e.metrics.root_parallel_workers = e.metrics.root_parallel_workers.max(workers);
    let next = AtomicUsize::new(0);
    let results = std::thread::scope(|scope| {
        let mut handles = vec![];
        let mut failure = None;
        for _ in 0..workers {
            let mut worker = Engine {
                p: Arc::clone(&e.p),
                cfg: e.cfg.clone(),
                policy: e.policy,
                lane: 0,
                limits: e.limits.clone(),
                metrics: Metrics::default(),
                probe_remaining: 0,
            };
            let snapshot = &*s;
            let next = &next;
            match std::thread::Builder::new()
                .name("apa-root-probe".into())
                .spawn_scoped(scope, move || {
                    let cancel = Arc::clone(&worker.limits.cancel);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut state = snapshot.clone();
                        let mut results = vec![];
                        while !worker.limits.stopped() {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(&(job, allowance)) = jobs.get(index) else {
                                break;
                            };
                            worker.probe_remaining = allowance;
                            results.push((index, probe(&mut worker, &mut state, snapshot, job)));
                        }
                        results
                    }))
                    .map_err(|_| {
                        cancel.store(true, Ordering::Release);
                        "root probe worker panicked".to_owned()
                    })
                }) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    e.limits.cancel.store(true, Ordering::Release);
                    failure = Some(format!("start root probe worker: {error}"));
                    break;
                }
            }
        }
        let mut results = vec![];
        for handle in handles {
            match handle.join() {
                Ok(Ok(completed)) => results.extend(completed),
                Ok(Err(error)) => failure = Some(error),
                Err(_) => failure = Some("root probe worker panicked".into()),
            }
        }
        results.sort_by_key(|(index, _)| *index);
        failure.map_or(
            Ok(results
                .into_iter()
                .map(|(_, result)| result)
                .collect::<Vec<_>>()),
            Err,
        )
    })?;
    for result in &results {
        e.probe_remaining -= result.metrics.probe_events as usize;
        e.metrics.add_probe_work(&result.metrics);
    }
    // Import the complete contradiction before considering cancellation or
    // propagating other results, which may already have been interrupted.
    let mut results = results;
    if let Some(index) = results.iter().position(|result| result.conflict) {
        let result = results.swap_remove(index);
        let roots = s
            .proof
            .append_fragment(result.proof, result.roots, shared_scopes);
        return Ok(Propagation::Conflict(roots[0]));
    }
    for result in results {
        let before = s.revision;
        let roots = s
            .proof
            .append_fragment(result.proof, result.roots, shared_scopes);
        for id in roots {
            assert_eq!(s.proof.get(id).scope, 0);
            let fact = s.proof.get(id).fact.clone();
            if let Err(id) = e.apply(s, fact, id) {
                return Ok(Propagation::Conflict(id));
            }
        }
        if s.revision != before {
            match result.job {
                Job::Factor(sum) => e.metrics.root_cover_sums.push(sum),
                Job::Absence(v) => e.metrics.root_forced_absences.push(v),
            }
        }
    }
    Ok(e.quiesce(s))
}

pub(super) fn strengthen(
    e: &mut Engine,
    s: &mut State,
    threads: usize,
) -> std::result::Result<Propagation, String> {
    let started = Instant::now();
    let result = strengthen_inner(e, s, threads);
    e.metrics.probe_wall_time += started.elapsed().as_secs_f64();
    result
}

fn strengthen_inner(
    e: &mut Engine,
    s: &mut State,
    threads: usize,
) -> std::result::Result<Propagation, String> {
    assert_eq!(s.scope, 0);
    assert!(threads > 0);
    let mut last_chains_members = s.members.len();
    loop {
        if e.limits.stopped() {
            return Ok(Propagation::Paused);
        }
        if e.probe_remaining == 0 {
            return Ok(e.quiesce(s));
        }
        e.metrics.root_strengthen_rounds += 1;
        let revision = s.revision;
        let mut sums = s
            .unresolved
            .iter()
            .copied()
            .filter(|&sum| (2..=e.cfg.root_probe_width).contains(&s.live_count[sum]))
            .collect::<Vec<_>>();
        sums.sort_unstable();
        let mut sums = sums.into_iter();
        loop {
            let mut jobs = vec![];
            let mut available = e.probe_remaining;
            // Queue several covers per worker so short jobs can immediately
            // hand their CPU to another cover instead of waiting at a barrier.
            while jobs.len() < threads.saturating_mul(2) && available > 0 && !e.limits.stopped() {
                let Some(sum) = sums.next() else { break };
                if s.product_count[sum] > 0
                    || !(2..=e.cfg.root_probe_width).contains(&s.live_count[sum])
                {
                    continue;
                }
                let allowance = e
                    .cfg
                    .probe_case_events
                    .saturating_mul(s.live_count[sum])
                    .min(available);
                available -= allowance;
                jobs.push((Job::Factor(sum), allowance));
            }
            if jobs.is_empty() {
                break;
            }
            let result = batch(e, s, &jobs, threads)?;
            if result != Propagation::Quiet {
                return Ok(result);
            }
            if s.members.len() > last_chains_members {
                let result = e.prime_chains(s);
                if result != Propagation::Quiet {
                    return Ok(result);
                }
                last_chains_members = s.members.len();
                break;
            }
        }
        let targets = std::iter::once(4)
            .chain(GROUPS.iter().flat_map(|g| g.iter().copied()))
            .filter(|&v| v <= e.p.n && s.member[v].is_none() && s.banned[v].is_none())
            .collect::<Vec<_>>();
        let mut targets = targets.into_iter();
        loop {
            let mut jobs = vec![];
            let mut available = e.probe_remaining;
            while jobs.len() < threads && available > 0 && !e.limits.stopped() {
                let Some(v) = targets.next() else { break };
                if s.member[v].is_some() || s.banned[v].is_some() {
                    continue;
                }
                let allowance = e.cfg.probe_case_events.min(available);
                available -= allowance;
                jobs.push((Job::Absence(v), allowance));
            }
            if jobs.is_empty() {
                break;
            }
            let result = batch(e, s, &jobs, threads)?;
            if result != Propagation::Quiet {
                return Ok(result);
            }
        }
        if s.members.len() > last_chains_members {
            let result = e.prime_chains(s);
            if result != Propagation::Quiet {
                return Ok(result);
            }
            last_chains_members = s.members.len();
        }
        if e.limits.stopped() {
            return Ok(Propagation::Paused);
        }
        if s.revision == revision {
            return Ok(Propagation::Quiet);
        }
    }
}
