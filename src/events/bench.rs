//! Synthetic performance benchmarks, ignored by default. They never make the
//! maximum a member, so they decide nothing about any maximum; they exercise
//! the event engine on large factor tables. Run them with
//! `cargo test --release bench_ -- --ignored --nocapture`.
use super::engine::{Engine, Limits, Metrics, Policy, Propagation, State};
use super::problem::Problem;
use super::proof::{Fact, Rule};
use super::*;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;

fn engine(p: Arc<Problem>) -> Engine {
    let n = p.n;
    Engine {
        p,
        cfg: Config {
            maximum: n,
            probe_members: std::env::var("BENCH_MEMBERS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            ..Config::default()
        },
        policy: Policy::Mrv,
        lane: 0,
        limits: Limits {
            deadline: None,
            cancel: Arc::new(AtomicBool::new(false)),
            interrupted: Arc::new(AtomicBool::new(false)),
            horizon: None,
        },
        metrics: Metrics::default(),
        probe_remaining: usize::MAX,
    }
}

fn ban_sixteenths() -> u64 {
    std::env::var("BENCH_BAN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4)
}

#[test]
#[ignore]
fn bench_synthetic_propagation() {
    let n: usize = std::env::var("BENCH_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150000);
    let started = Instant::now();
    let p = Arc::new(Problem::new(n).unwrap());
    let table = started.elapsed().as_secs_f64();
    let mut e = engine(Arc::clone(&p));
    let mut s = State::new(&p);
    let t = Instant::now();
    // Members of the published maximum-113 example, but not n itself.
    let seed = [
        1usize, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37,
        39, 41, 43, 45, 47, 49, 53, 55, 59, 61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 99, 101, 103,
        107, 109, 113,
    ];
    for v in seed {
        let r = s.node(Fact::Member(v), Rule::Assumption, vec![]);
        e.force(&mut s, v, r).unwrap();
    }
    // Ban a deterministic sparse pattern of larger values.
    let mut rng = 0xbad5eed_u64;
    for v in 114..=n {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        if (rng >> 60) < ban_sixteenths() {
            let r = s.node(Fact::Ban(v), Rule::Assumption, vec![]);
            if e.ban(&mut s, v, r).is_err() {
                break;
            }
        }
    }
    // Sums above n with an empty factor domain are impossible products.
    for sum in 2..=p.limit {
        if s.live_count[sum] == 0 {
            let r = s.node(Fact::Bad(sum), Rule::Assumption, vec![]);
            e.bad(&mut s, sum, r).unwrap();
        }
    }
    let root = e.quiesce(&mut s);
    let root_time = t.elapsed().as_secs_f64();
    let members = s.members.len();
    let t = Instant::now();
    let undecided = (3..=n)
        .filter(|&v| s.member[v].is_none() && s.banned[v].is_none())
        .collect::<Vec<_>>();
    let stride = (undecided.len() / 300).max(1);
    let stride = if undecided.len() > 3000 {
        undecided.len() / 300
    } else {
        stride
    };
    let (mut refuted, mut survived) = (0, 0);
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    for (i, &v) in undecided.iter().step_by(stride).enumerate() {
        let f = if i % 2 == 0 {
            Fact::Member(v)
        } else {
            Fact::Ban(v)
        };
        let r = e.probe(&mut s, vec![f], usize::MAX);
        if r.conflict.is_some() {
            refuted += 1
        } else {
            survived += 1
        }
        r.conflict.is_some().hash(&mut digest);
        for fact in r.delta.keys() {
            fact.hash(&mut digest);
        }
    }
    let probe_time = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let chains = e.prime_chains(&mut s);
    let chain_time = t.elapsed().as_secs_f64();
    s.freeze();
    let t = Instant::now();
    let clone = s.clone();
    let clone_time = t.elapsed().as_secs_f64();
    drop(clone);
    println!(
        "BENCH n={n} table={table:.3} root={root_time:.3} ({root:?}, members {members}, undecided {}) probes={probe_time:.3} (refuted {refuted}, survived {survived}, events {}, kills {}) chains={chain_time:.3} ({chains:?}, steps {}) clone={clone_time:.3} arena={} digest={:016x}",
        undecided.len(),
        e.metrics.probe_events,
        e.metrics.witness_kills,
        e.metrics.prime_chain_steps,
        s.proof.len(),
        digest.finish()
    );
}

#[test]
#[ignore]
fn bench_chain_search() {
    use std::collections::VecDeque;
    let n: usize = std::env::var("BENCH_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150000);
    let every: usize = std::env::var("BENCH_EVERY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let p = Arc::new(Problem::new(n).unwrap());
    let mut e = engine(Arc::clone(&p));
    let mut s = State::new(&p);
    // Members and bans are installed without propagation: only the chain
    // search, which reads these arrays, is timed.
    let cap: usize = std::env::var("BENCH_CAP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(n);
    let banmod: usize = std::env::var("BENCH_BANMOD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    for v in (1..=cap).filter(|v| *v <= 2 || (v % 2 == 1 && v % every == 1)) {
        let r = s.node(Fact::Member(v), Rule::Assumption, vec![]);
        e.force(&mut s, v, r).unwrap();
    }
    for v in 3..=n {
        if banmod == 0 || v % banmod != 3 || s.member[v].is_some() {
            continue;
        }
        let r = s.node(Fact::Ban(v), Rule::Assumption, vec![]);
        e.ban(&mut s, v, r).unwrap();
    }
    let odd = s.members.iter().filter(|a| *a % 2 == 1).count();
    let candidates = (2..=n)
        .step_by(2)
        .filter(|&c| s.member[c].is_none() && s.banned[c].is_none())
        .collect::<Vec<_>>();
    // Previous algorithm: BFS from every odd member with full reason lists.
    let t = Instant::now();
    let mut known = vec![0u32; p.limit + 1];
    let mut generation = 0;
    let mut queue = VecDeque::new();
    let mut old_found = 0;
    for &c in &candidates {
        generation += 1;
        queue.clear();
        let mut reasons = vec![];
        for &a in &s.members {
            if a % 2 == 1 {
                known[a] = generation;
                queue.push_back(a);
                reasons.push(s.member[a].unwrap());
            }
        }
        let mut steps = vec![];
        'chain: while let Some(a) = queue.pop_front() {
            for h in [2, c] {
                let q = a + h;
                if !p.prime[q] || known[q] == generation {
                    continue;
                }
                steps.push((a, h, q));
                known[q] = generation;
                if q > n || s.banned[q].is_some() {
                    old_found += 1;
                    break 'chain;
                }
                queue.push_back(q);
            }
        }
        std::hint::black_box((&steps, &reasons));
    }
    let old_time = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut search = super::engine::ChainSearch::new(&p);
    let mut new_found = 0;
    for &c in &candidates {
        let mut left = usize::MAX;
        new_found += usize::from(e.prime_chain(&s, &mut search, c, &mut left).is_some());
    }
    let new_time = t.elapsed().as_secs_f64();
    println!(
        "CHAINS n={n} odd_members={odd} candidates={} old={old_time:.3}s ({old_found}) new={new_time:.3}s ({new_found})",
        candidates.len()
    );
    assert_eq!(old_found, new_found);
}

#[test]
#[ignore]
fn bench_parallel_root() {
    let n: usize = std::env::var("BENCH_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20000);
    let threads: usize = std::env::var("BENCH_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let p = Arc::new(Problem::new(n).unwrap());
    let mut e = engine(Arc::clone(&p));
    e.cfg.probe_case_events = usize::MAX;
    e.cfg.root_probe_width = 8;
    e.cfg.no_root_speculation = std::env::var("BENCH_NOSPEC").is_ok();
    let mut s = State::new(&p);
    let seed = [
        1usize, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37,
        39, 41, 43, 45, 47, 49, 53, 55, 59, 61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 99, 101, 103,
        107, 109, 113,
    ];
    let k: usize = std::env::var("BENCH_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    for &v in &seed[..k] {
        let r = s.node(Fact::Member(v), Rule::Assumption, vec![]);
        e.force(&mut s, v, r).unwrap();
    }
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    let t = Instant::now();
    let result = super::parallel_root::strengthen(&mut e, &mut s, threads).unwrap();
    let m = &e.metrics;
    println!(
        "ROOT n={n} threads={threads} time={:.3} result={result:?} members={} bans={} rounds={} batches={} workers={} probe_events={} cover_sums={:?} absences={:?} clone={:.3} chains={:.3}",
        t.elapsed().as_secs_f64(),
        s.members.len(),
        (1..=n).filter(|&v| s.banned[v].is_some()).count(),
        m.root_strengthen_rounds,
        m.root_parallel_batches,
        m.root_parallel_workers,
        m.probe_events,
        m.root_cover_sums.len(),
        m.root_forced_absences,
        m.root_worker_clone_cpu_time,
        m.prime_chain_wall_time
    );
}
