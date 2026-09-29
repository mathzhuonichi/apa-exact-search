//! Parallel propagation-only root probes against a single immutable snapshot.
//! Only parent-scoped conclusions and their proof dependencies cross workers.
use super::GROUPS;
use super::engine::{Engine, Metrics, Propagation, State};
use super::proof::{Arena, Cover, Fact, Id, Rule};
use std::collections::VecDeque;
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
impl Result {
    /// New root facts or a contradiction; either changes an unchanged root.
    fn productive(&self) -> bool {
        self.conflict || !self.roots.is_empty()
    }
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
    let fragment_started = Instant::now();
    let (proof, roots) = state.proof.fragment(&roots, snapshot.proof.scopes.len());
    worker.metrics.root_worker_fragment_cpu_time += fragment_started.elapsed().as_secs_f64();
    state.rollback(&worker.p, cp);
    state.proof.clone_from(&snapshot.proof);
    Result {
        job,
        proof,
        roots,
        conflict: conflict.is_some(),
        metrics: std::mem::take(&mut worker.metrics),
    }
}

/// Take the next jobs that are still useful on the current root. Allowances
/// are reserved greedily from the shared budget. If the first job alone needs
/// the entire remaining budget, successive batches would each run one job.
/// Such a batch is marked speculative: its successors run in parallel against
/// the same snapshot, and `batch` commits only results the serial order would
/// have produced.
fn plan(
    e: &Engine,
    s: &State,
    queue: &mut VecDeque<Job>,
    threads: usize,
    per_batch: usize,
) -> (Vec<(Job, usize)>, bool) {
    let mut jobs = vec![];
    let mut available = e.probe_remaining;
    let mut speculative = false;
    while available > 0
        && jobs.len() < if speculative { threads } else { per_batch }
        && !e.limits.stopped()
    {
        let Some(job) = queue.pop_front() else { break };
        let desired = match job {
            Job::Factor(sum) => {
                let live = s.live_count[sum];
                if s.product_count[sum] > 0 || !(2..=e.cfg.root_probe_width).contains(&live) {
                    continue;
                }
                e.cfg.probe_case_events.saturating_mul(live)
            }
            Job::Absence(v) => {
                if s.member[v].is_some() || s.banned[v].is_some() {
                    continue;
                }
                e.cfg.probe_case_events
            }
        };
        if jobs.is_empty() && desired >= available && threads > 1 && !e.cfg.no_root_speculation {
            speculative = true;
        }
        if speculative {
            jobs.push((job, desired.min(e.probe_remaining)));
        } else {
            let allowance = desired.min(available);
            available -= allowance;
            jobs.push((job, allowance));
        }
    }
    (jobs, speculative)
}

/// Install one result's parent-scoped conclusions; returns a contradiction.
fn commit(e: &mut Engine, s: &mut State, result: Result, shared_scopes: usize) -> Option<Id> {
    let before = s.revision;
    let roots = s
        .proof
        .append_fragment(result.proof, result.roots, shared_scopes);
    for id in roots {
        assert_eq!(s.proof.get(id).scope, 0);
        let fact = s.proof.get(id).fact.clone();
        if let Err(id) = e.apply(s, fact, id) {
            return Some(id);
        }
    }
    if s.revision != before {
        match result.job {
            Job::Factor(sum) => e.metrics.root_cover_sums.push(sum),
            Job::Absence(v) => e.metrics.root_forced_absences.push(v),
        }
    }
    None
}

/// Run one batch and return the propagation state with the number of leading
/// jobs whose results were committed. Uncommitted jobs have not run in the
/// serial order and must be offered again.
fn batch(
    e: &mut Engine,
    s: &mut State,
    pool: &mut Vec<State>,
    jobs: &[(Job, usize)],
    threads: usize,
    speculative: bool,
) -> std::result::Result<(Propagation, usize), String> {
    // Retained states release the shared prefix after each batch, so freezing
    // appends in place instead of copying every earlier proof node.
    debug_assert_eq!(Arc::strong_count(&s.proof.prefix), 1);
    let changes = s.freeze_changes();
    let shared_scopes = s.proof.scopes.len();
    e.metrics.root_parallel_batches += 1;
    e.metrics.root_speculative_batches += usize::from(speculative);
    let workers = threads.min(jobs.len());
    e.metrics.root_parallel_workers = e.metrics.root_parallel_workers.max(workers);
    let next = AtomicUsize::new(0);
    let first_productive = Arc::new(AtomicUsize::new(usize::MAX));
    let workers_started = Instant::now();
    // Retained worker states from earlier batches replay the root changes;
    // new workers clone the snapshot. Every retained state is synchronized,
    // even when it receives no job, so the pool stays one snapshot behind.
    let states = (0..workers.max(pool.len()))
        .map(|_| pool.pop())
        .collect::<Vec<_>>();
    let (results, states, prepare) = std::thread::scope(|scope| {
        let mut handles = vec![];
        let mut failure = None;
        for retained in states {
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
            let changes = &changes;
            let next = &next;
            let first_productive = &first_productive;
            match std::thread::Builder::new()
                .name("apa-root-probe".into())
                .spawn_scoped(scope, move || {
                    let cancel = Arc::clone(&worker.limits.cancel);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let prepare_started = Instant::now();
                        let mut prepare = Metrics::default();
                        let mut state = match retained {
                            Some(mut state) => {
                                state.sync_from(&worker.p, snapshot, changes);
                                prepare.root_worker_sync_cpu_time +=
                                    prepare_started.elapsed().as_secs_f64();
                                state
                            }
                            None => {
                                let state = snapshot.clone();
                                prepare.root_worker_clone_cpu_time +=
                                    prepare_started.elapsed().as_secs_f64();
                                state
                            }
                        };
                        let mut results = vec![];
                        loop {
                            worker.limits.horizon = None;
                            if worker.limits.stopped() {
                                break;
                            }
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(&(job, allowance)) = jobs.get(index) else {
                                break;
                            };
                            if speculative {
                                if first_productive.load(Ordering::Relaxed) < index {
                                    continue;
                                }
                                worker.limits.horizon = Some((Arc::clone(first_productive), index));
                            }
                            worker.probe_remaining = allowance;
                            let result = probe(&mut worker, &mut state, snapshot, job);
                            if speculative && result.productive() {
                                first_productive.fetch_min(index, Ordering::Relaxed);
                            }
                            results.push((index, result));
                        }
                        (results, state, prepare)
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
        let mut states = vec![];
        let mut prepare = Metrics::default();
        for handle in handles {
            match handle.join() {
                Ok(Ok((completed, state, times))) => {
                    results.extend(completed);
                    states.push(state);
                    prepare.add_probe_work(&times);
                }
                Ok(Err(error)) => failure = Some(error),
                Err(_) => failure = Some("root probe worker panicked".into()),
            }
        }
        results.sort_by_key(|(index, _)| *index);
        failure.map_or(Ok((results, states, prepare)), Err)
    })?;
    // Keep retained states' mutable arrays but not the snapshot's proof arena;
    // they receive the next snapshot's arena when synchronized.
    *pool = states
        .into_iter()
        .map(|mut state| {
            state.proof = Arena::new();
            state
        })
        .collect();
    e.metrics.root_worker_clone_cpu_time += prepare.root_worker_clone_cpu_time;
    e.metrics.root_worker_sync_cpu_time += prepare.root_worker_sync_cpu_time;
    e.metrics.root_batch_worker_wall_time += workers_started.elapsed().as_secs_f64();
    let merge_started = Instant::now();
    let mut results = results;
    // Import a complete contradiction before considering cancellation or
    // propagating other results, which may already have been interrupted. A
    // contradiction on an earlier snapshot still refutes the current root.
    if let Some(position) = results.iter().position(|(_, result)| result.conflict) {
        let (_, result) = results.swap_remove(position);
        // Ordinary reservations fit the budget together. Speculative jobs each
        // reserved the whole remainder, so only the refuting job is charged.
        let charged = std::iter::once(&result).chain(results.iter().map(|(_, r)| r));
        for (i, other) in charged.enumerate() {
            if speculative && i > 0 {
                e.metrics.root_discarded_events += other.metrics.probe_events;
                e.metrics.root_discarded_jobs += 1;
            } else {
                e.probe_remaining -= other.metrics.probe_events as usize;
                e.metrics.add_probe_work(&other.metrics);
            }
        }
        let roots = s
            .proof
            .append_fragment(result.proof, result.roots, shared_scopes);
        e.metrics.root_batch_merge_wall_time += merge_started.elapsed().as_secs_f64();
        return Ok((Propagation::Conflict(roots[0]), jobs.len()));
    }
    if !speculative {
        for (_, result) in &results {
            e.probe_remaining -= result.metrics.probe_events as usize;
            e.metrics.add_probe_work(&result.metrics);
        }
        for (_, result) in results {
            if let Some(id) = commit(e, s, result, shared_scopes) {
                e.metrics.root_batch_merge_wall_time += merge_started.elapsed().as_secs_f64();
                return Ok((Propagation::Conflict(id), jobs.len()));
            }
        }
        e.metrics.root_batch_merge_wall_time += merge_started.elapsed().as_secs_f64();
        let propagate_started = Instant::now();
        let result = e.quiesce(s);
        e.metrics.root_parent_propagation_wall_time += propagate_started.elapsed().as_secs_f64();
        return Ok((result, jobs.len()));
    }
    // Commit in job order while each result is exactly what one-job batches
    // would have produced: it ran on the unchanged root, the job would again
    // have reserved the entire remaining budget, and that possibly smaller
    // budget would not have stopped it. The first productive commit changes
    // the root and ends the batch; later results are discarded and do not
    // consume the budget.
    let base = s.revision;
    let mut consumed = 0;
    let mut outcome = Propagation::Quiet;
    let mut results = results.into_iter().peekable();
    while let Some((index, result)) = results.next_if(|(index, _)| *index == consumed) {
        let used = result.metrics.probe_events as usize;
        let (allowance, remaining) = (jobs[index].1, e.probe_remaining);
        let exact = allowance >= remaining && (allowance == remaining || used < remaining);
        // One-job batches would not have started this job after a stop.
        let stopped = index > 0 && e.limits.stopped();
        if s.revision != base || !exact || stopped {
            e.metrics.root_discarded_events += used as u64;
            break;
        }
        e.probe_remaining -= used;
        e.metrics.add_probe_work(&result.metrics);
        consumed += 1;
        if let Some(id) = commit(e, s, result, shared_scopes) {
            outcome = Propagation::Conflict(id);
            break;
        }
        let propagate_started = Instant::now();
        outcome = e.quiesce(s);
        e.metrics.root_parent_propagation_wall_time += propagate_started.elapsed().as_secs_f64();
        if outcome != Propagation::Quiet {
            break;
        }
    }
    e.metrics.root_discarded_events += results
        .map(|(_, result)| result.metrics.probe_events)
        .sum::<u64>();
    e.metrics.root_discarded_jobs += jobs.len() - consumed;
    e.metrics.root_batch_merge_wall_time += merge_started.elapsed().as_secs_f64();
    Ok((outcome, consumed))
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

/// Run a queue of jobs in batches until it is exhausted or members grow.
/// Returns early with any non-quiet propagation state.
fn drain(
    e: &mut Engine,
    s: &mut State,
    pool: &mut Vec<State>,
    mut queue: VecDeque<Job>,
    threads: usize,
    per_batch: usize,
    stop_on_new_members: Option<&mut usize>,
) -> std::result::Result<Propagation, String> {
    let mut last_chains_members = stop_on_new_members;
    loop {
        let (jobs, speculative) = plan(e, s, &mut queue, threads, per_batch);
        if jobs.is_empty() {
            return Ok(Propagation::Quiet);
        }
        let (result, consumed) = batch(e, s, pool, &jobs, threads, speculative)?;
        for &(job, _) in jobs[consumed..].iter().rev() {
            queue.push_front(job);
        }
        if result != Propagation::Quiet {
            return Ok(result);
        }
        if let Some(last) = last_chains_members.as_deref_mut()
            && s.members.len() > *last
        {
            let result = e.prime_chains(s);
            if result != Propagation::Quiet {
                return Ok(result);
            }
            *last = s.members.len();
            return Ok(Propagation::Quiet);
        }
    }
}

fn strengthen_inner(
    e: &mut Engine,
    s: &mut State,
    threads: usize,
) -> std::result::Result<Propagation, String> {
    assert_eq!(s.scope, 0);
    assert!(threads > 0);
    let mut pool = vec![];
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
        // Queue several covers per worker so short jobs can immediately
        // hand their CPU to another cover instead of waiting at a barrier.
        let queue = sums.into_iter().map(Job::Factor).collect();
        let result = drain(
            e,
            s,
            &mut pool,
            queue,
            threads,
            threads.saturating_mul(2),
            Some(&mut last_chains_members),
        )?;
        if result != Propagation::Quiet {
            return Ok(result);
        }
        let targets = std::iter::once(4)
            .chain(GROUPS.iter().flat_map(|g| g.iter().copied()))
            .filter(|&v| v <= e.p.n && s.member[v].is_none() && s.banned[v].is_none())
            .map(Job::Absence)
            .collect();
        let result = drain(e, s, &mut pool, targets, threads, threads, None)?;
        if result != Propagation::Quiet {
            return Ok(result);
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
