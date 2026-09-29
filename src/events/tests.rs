use super::engine::{Engine, Limits, Metrics, Outcome, Policy, Propagation, State};
use super::problem::Problem;
use super::proof::{Fact, Rule};
use super::*;
use std::sync::{Arc, atomic::AtomicBool};

fn engine(n: usize) -> Engine {
    Engine {
        p: Arc::new(Problem::new(n).unwrap()),
        cfg: Config {
            maximum: n,
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
        probe_remaining: 240000,
    }
}
fn assume(e: &mut Engine, s: &mut State, f: Fact) -> Result<(), usize> {
    let id = s.node(f.clone(), Rule::Assumption, vec![]);
    match f {
        Fact::Member(v) => e.force(s, v, id),
        Fact::Ban(v) => e.ban(s, v, id),
        Fact::Bad(v) => e.bad(s, v, id),
        Fact::Pair(d, b) => e.pair(s, d, b, id),
        _ => panic!(),
    }
}
fn counts(e: &Engine, s: &State) {
    for sum in 2..=e.p.limit {
        let live =
            e.p.domain(sum)
                .filter(|&id| s.dead[id].is_none())
                .collect::<Vec<_>>();
        assert_eq!(s.live_count[sum], live.len());
        assert_eq!(s.xor_live_id[sum], live.iter().fold(0, |a, b| a ^ b));
        let products =
            e.p.domain(sum)
                .filter(|&id| {
                    let w = e.p.witness[id];
                    s.member[w.d].is_some() && s.member[w.e].is_some()
                })
                .count();
        assert_eq!(s.product_count[sum], products);
    }
}
fn fingerprint(s: &State) -> String {
    let mut pairs = s.pairs.iter().collect::<Vec<_>>();
    pairs.sort();
    format!(
        "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{}{}{}",
        s.member,
        s.banned,
        s.bad,
        s.active,
        s.dead,
        s.members,
        s.live_count,
        s.xor_live_id,
        s.product_count,
        s.unresolved,
        pairs,
        s.revision,
        s.scope,
        s.fingerprint_extensions()
    )
}

#[test]
fn complete_tables_and_reverse_indices() {
    for n in 1..=100 {
        let p = Problem::new(n).unwrap();
        for sum in 2..=p.limit {
            let expected = (1..=n)
                .flat_map(|d| (d..=n).filter(move |e| d * e == sum).map(move |e| (d, e)))
                .collect::<Vec<_>>();
            let actual = p
                .domain(sum)
                .map(|id| {
                    let w = p.witness[id];
                    (w.d, w.e)
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
        for v in 1..=n {
            assert_eq!(
                p.endpoint.get(v),
                p.witness
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| w.d == v || w.e == v)
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            );
        }
        for sum in 2..=p.limit {
            assert_eq!(
                p.endpoint_sum.get(sum),
                p.witness
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| w.d + w.e == sum)
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn boundary_and_exhaustive_small_sets() {
    for n in 1..=14 {
        let exists = (1u64..1 << n).any(|mask| {
            let a = (1..=n)
                .filter(|v| mask & (1 << (v - 1)) != 0)
                .collect::<Vec<_>>();
            validate(n, &a)
        });
        for probes in [false, true] {
            let r = run(
                Config {
                    maximum: n,
                    probes,
                    ..Config::default()
                },
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            assert_eq!(
                r.status,
                if exists { "YES" } else { "NO" },
                "n={n}: {:?}",
                r.reason
            );
            if let Some(c) = r.certificate {
                assert!(!proof::verify(&c, false).unwrap());
            }
        }
    }
}
#[test]
fn rollback_paused_queue_and_sibling_marks() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    let before = fingerprint(&s);
    let cp = s.checkpoint();
    assume(&mut e, &mut s, Fact::Ban(4)).unwrap();
    assert_eq!(e.propagate(&mut s, &mut 1), Propagation::Paused);
    s.rollback(&e.p, cp);
    assert!(s.quiet());
    assert_eq!(before, fingerprint(&s));
    counts(&e, &s);
    assume(&mut e, &mut s, Fact::Ban(4)).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    for &id in e.p.endpoint.get(4) {
        assert!(s.dead[id].is_some());
    }
    counts(&e, &s);
}
#[test]
#[should_panic(expected = "checkpoint requires empty propagation queues")]
fn checkpoint_rejects_pending_work() {
    let mut e = engine(20);
    let mut s = State::new(&e.p);
    assume(&mut e, &mut s, Fact::Member(2)).unwrap();
    s.checkpoint();
}
#[test]
fn bad_product_both_event_orders_and_independent_deletion() {
    let mut e = engine(113);
    for bad_first in [true, false] {
        let mut s = State::new(&e.p);
        let facts = if bad_first {
            [Fact::Bad(13), Fact::Member(2)]
        } else {
            [Fact::Member(2), Fact::Bad(13)]
        };
        for f in facts {
            assume(&mut e, &mut s, f).unwrap();
            assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
        }
        assert!(s.banned[11].is_some());
        assert!(s.bad[13].is_some());
        assert!(e.p.domain(13).all(|id| s.dead[id].is_some()));
        counts(&e, &s);
    }
}
#[test]
fn no_double_kill_square_count_and_zero_xor_id() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    assert_eq!(e.p.domain(2).collect::<Vec<_>>(), vec![0]);
    assert_eq!(s.xor_live_id[2], 0);
    assume(&mut e, &mut s, Fact::Member(2)).unwrap();
    assert_eq!(s.product_count[4], 1);
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    assume(&mut e, &mut s, Fact::Ban(7)).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    let id = e.p.pair_id(7, 7).unwrap();
    let count = s.live_count[49];
    let r = s.dead[id].unwrap();
    e.kill(&mut s, id, r).unwrap();
    assert_eq!(s.live_count[49], count);
    counts(&e, &s);
}
#[test]
fn policies_are_actually_different() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    s.member[7] = Some(0);
    s.unresolved = vec![120, 30];
    s.live_count[120] = 3;
    s.live_count[30] = 2;
    assert_eq!(e.select(&s), Some(30));
    e.policy = Policy::StrictMaximumRowFirst;
    assert_eq!(e.select(&s), Some(120));
}
#[test]
fn six_seeds_and_39_clauses_keep_full_cover() {
    let cases = seed_cases();
    assert_eq!(cases.len(), 6);
    let q = GROUPS
        .iter()
        .flat_map(|g| g.iter().copied())
        .collect::<Vec<_>>();
    let mut clauses = vec![];
    for (i, g) in GROUPS.iter().enumerate() {
        for h in &GROUPS[i + 1..] {
            for p in *g {
                for b in *h {
                    clauses.push((*p, *b));
                }
            }
        }
    }
    assert_eq!(clauses.len(), 39);
    for mask in 0..1 << q.len() {
        let member = |v| mask & (1 << q.iter().position(|q| *q == v).unwrap()) != 0;
        let positive = clauses.iter().all(|&(a, b)| member(a) || member(b));
        let covered = cases
            .iter()
            .any(|c| c.iter().all(|f| matches!(f,Fact::Member(v) if member(*v))));
        assert_eq!(positive, covered);
    }
    assert!(!U.contains(&4));
}
#[test]
fn random_atomic_operations_restore_parent() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    let original = fingerprint(&s);
    let mut rng = 123456789u64;
    for _ in 0..80 {
        let cp = s.checkpoint();
        for _ in 0..12 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = 1 + (rng as usize % 113);
            let f = if rng & 1 == 0 {
                Fact::Ban(v)
            } else {
                Fact::Member(v)
            };
            if assume(&mut e, &mut s, f).is_err() {
                break;
            }
            counts(&e, &s);
            if e.propagate(&mut s, &mut 3) != Propagation::Quiet {
                break;
            }
        }
        s.rollback(&e.p, cp);
        assert_eq!(fingerprint(&s), original);
        counts(&e, &s);
    }
}
#[test]
fn forged_root_empty_domain_and_scope_are_rejected() {
    let r = run(
        Config {
            maximum: 3,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let cert = r.certificate.unwrap();
    assert!(proof::verify(&cert, false).is_ok());
    let mut forged = cert.clone();
    for node in &mut forged.nodes {
        if matches!(node.rule, Rule::Initial) {
            node.fact = Fact::Member(3);
        }
    }
    assert!(proof::verify(&forged, false).is_err());
    let mut forged = cert.clone();
    let id = forged.nodes.len();
    forged.nodes.push(proof::Node {
        scope: 0,
        fact: Fact::Bad(2),
        rule: Rule::Empty,
        premises: vec![],
    });
    forged.root = id;
    assert!(proof::verify(&forged, false).is_err());
    let mut forged = cert;
    forged.scopes.push(proof::Scope {
        parent: Some(0),
        assumptions: vec![Fact::Member(3)],
    });
    forged.nodes[0].scope = 1;
    assert!(proof::verify(&forged, false).is_err());
}

#[test]
fn unstarted_and_paused_probes_do_not_become_refutations() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    e.initialize(&mut s).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    let before = fingerprint(&s);
    let v = (1..=113)
        .find(|&v| s.member[v].is_none() && s.banned[v].is_none())
        .unwrap();
    let r = e.probe(&mut s, vec![Fact::Ban(v)], 0);
    assert!(r.conflict.is_none());
    assert!(r.delta.contains_key(&Fact::Ban(v)));
    assert_eq!(before, fingerprint(&s));
    let sum = e.select(&s).unwrap();
    let (cases, ctx) = e.cover_context(&mut s, sum);
    // The context may cache materialized deletion proofs; facts are unchanged.
    let before = fingerprint(&s);
    let count = cases.len();
    e.cover(&mut s, proof::Cover::Factor(sum), cases, ctx, 0)
        .unwrap();
    assert_eq!(e.metrics.probe_unstarted_cases, count as u64);
    assert_eq!(before, fingerprint(&s));
    assert!(s.pairs.is_empty());
    assert_eq!(e.metrics.probe_refutations, 0);
}

#[test]
fn github_positive_examples_survive_all_modules() {
    // mathzhuonichi/research @ 3867395440384a07fae542a85035f7bb423809a1,
    // apa/paper/RESULTS.md, Q1. Check the actual arithmetic, not the prose claim.
    let minimal = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37, 39, 41,
        43, 45, 47, 49, 53, 55, 59, 61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 99, 101, 103, 107, 109,
        113,
    ];
    let maximal = (1..=13)
        .chain([15, 16, 17])
        .chain((19..=109).step_by(2))
        .chain([113])
        .collect::<Vec<_>>();
    for a in [&minimal, &maximal] {
        assert!(validate(113, a));
        assert!(U.iter().all(|v| a.contains(v)));
        assert!(a.iter().filter(|v| **v <= 384 && **v % 2 == 0).count() >= 7);
    }
    for policy in [Policy::Mrv, Policy::StrictMaximumRowFirst] {
        for probes in [false, true] {
            for lemmas in [false, true] {
                let cfg = Config {
                    maximum: 113,
                    policy: Some(policy),
                    probes,
                    universal_members: lemmas,
                    prime_clauses: lemmas,
                    even_bound: lemmas,
                    seed_probe: probes && lemmas,
                    ..Config::default()
                };
                let r = run(cfg, Arc::new(AtomicBool::new(false))).unwrap();
                assert_eq!(r.status, "YES", "{:?}", r.reason);
                assert!(validate(113, &r.example));
            }
        }
    }
}

#[test]
fn external_lemmas_are_never_silently_verified() {
    let r = run(
        Config {
            maximum: 3,
            universal_members: true,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(r.evidence, "SOLVER_NO_EXTERNAL_LEMMAS");
    let cert = r.certificate.unwrap();
    assert!(proof::verify(&cert, false).is_err());
    assert_eq!(proof::verify(&cert, true), Ok(true));
}

#[test]
fn root_deadline_and_parallel_completion() {
    let r = run(
        Config {
            maximum: 113,
            root_seconds: f64::MIN_POSITIVE,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(r.status, "UNKNOWN");
    assert!(r.lanes.is_empty());
    let r = run(
        Config {
            maximum: 113,
            threads: 4,
            probes: true,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(r.status, "YES");
    assert_eq!(r.lanes.len(), 4);
    let r = run(
        Config {
            maximum: 113,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(true)),
    )
    .unwrap();
    assert_eq!(r.status, "UNKNOWN");
    assert!(r.certificate.is_none());
}

#[test]
fn checker_rejects_missing_cover_case_and_partial_common_fact() {
    let mut a = proof::Arena::new();
    let one = a.add(0, Fact::Member(1), Rule::Initial, vec![]);
    let two = a.add(0, Fact::Member(2), Rule::Initial, vec![]);
    let sum3 = a.add(0, Fact::Active(3), Rule::Sum, vec![one, two]);
    let three = a.add(0, Fact::Member(3), Rule::Unique(1, 3), vec![sum3]);
    let sum5 = a.add(0, Fact::Active(5), Rule::Sum, vec![two, three]);
    let five = a.add(0, Fact::Member(5), Rule::Unique(1, 5), vec![sum5]);
    let sum8 = a.add(0, Fact::Active(8), Rule::Sum, vec![three, five]);
    let scopes = vec![
        a.enter(0, vec![Fact::Member(1), Fact::Member(8)]),
        a.enter(0, vec![Fact::Member(2), Fact::Member(4)]),
    ];
    let per_case = scopes
        .iter()
        .map(|&s| a.add(s, Fact::Member(1), Rule::Unique(1, 3), vec![sum3]))
        .collect::<Vec<_>>();
    let join = a.add(
        0,
        Fact::Member(1),
        Rule::Join {
            cover: proof::Cover::Factor(8),
            scopes: scopes.clone(),
            context: 1,
        },
        vec![sum8, per_case[0], per_case[1]],
    );
    let cert = a.certificate(113, join);
    assert_eq!(
        proof::verify(&cert, false),
        Err("not a complete root refutation".into())
    );
    let mut omitted = cert.clone();
    let node = omitted.nodes.last_mut().unwrap();
    node.premises.pop();
    if let Rule::Join { scopes, .. } = &mut node.rule {
        scopes.pop();
    }
    assert!(
        proof::verify(&omitted, false)
            .unwrap_err()
            .starts_with("invalid inference")
    );
    let mut partial = cert;
    let node = partial.nodes.last_mut().unwrap();
    node.fact = Fact::Member(4);
    assert!(
        proof::verify(&partial, false)
            .unwrap_err()
            .starts_with("invalid inference")
    );
}

#[test]
fn checker_rejects_modified_prime_chain() {
    let mut a = proof::Arena::new();
    let two = a.add(0, Fact::Member(2), Rule::Initial, vec![]);
    let three = a.add(0, Fact::Member(3), Rule::Initial, vec![]);
    let ban = a.add(
        0,
        Fact::Ban(2),
        Rule::PrimeChain {
            candidate: 2,
            steps: vec![(3, 2, 5)],
        },
        vec![two, three],
    );
    let end = a.add(0, Fact::False, Rule::Conflict, vec![two, ban]);
    let mut cert = a.certificate(3, end);
    assert_eq!(proof::verify(&cert, false), Ok(false));
    if let Rule::PrimeChain { steps, .. } = &mut cert.nodes[ban].rule {
        steps[0].2 = 7;
    }
    assert!(proof::verify(&cert, false).is_err());
}

#[test]
fn pair_nogood_above_limit_and_rollback_integrity() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    let before = fingerprint(&s);
    let cp = s.checkpoint();
    assume(&mut e, &mut s, Fact::Pair(100, 101)).unwrap();
    assume(&mut e, &mut s, Fact::Member(100)).unwrap();
    // Only process the member event; its adjacency must still imply the ban.
    e.propagate(&mut s, &mut 1);
    assert!(s.banned[101].is_some());
    s.rollback(&e.p, cp);
    assert_eq!(fingerprint(&s), before);
    // Deliberate missing-trail mutation is detectable by the same rollback oracle.
    let cp = s.checkpoint();
    s.banned[42] = Some(0);
    s.rollback(&e.p, cp);
    assert_ne!(fingerprint(&s), before);
}

#[test]
fn conditional_dfs_refutations_discharge_without_root_assumptions() {
    use super::engine::Outcome;
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    e.initialize(&mut s).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    // These belong to the published 45-element core, not merely one example.
    let targets = [
        8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37, 41, 43, 47, 49, 53, 55, 59,
        61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 101, 103, 107, 109,
    ];
    let mut checked = 0;
    for v in targets {
        if s.member[v].is_some() {
            continue;
        }
        let before = fingerprint(&s);
        let cp = s.checkpoint();
        let child = s.proof.enter(0, vec![Fact::Ban(v)]);
        s.scope = child;
        assume(&mut e, &mut s, Fact::Ban(v)).unwrap();
        let result = e.dfs(&mut s);
        let Outcome::No(id) = result else {
            panic!("core absence {v} was not refuted: {result:?}");
        };
        assert_eq!(s.proof.get(id).scope, child);
        s.rollback(&e.p, cp);
        assert_eq!(before, fingerprint(&s));
        let member = s.node(Fact::Member(v), Rule::Discharge(child), vec![id]);
        let cert = s.proof.certificate(113, member);
        // Every inference must validate, but a proved member is not a root NO.
        assert_eq!(
            proof::verify(&cert, false),
            Err("not a complete root refutation".into())
        );
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn satisfiable_extensions_keep_the_known_solution() {
    use super::engine::Outcome;
    let example = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37, 39, 41,
        43, 45, 47, 49, 53, 55, 59, 61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 99, 101, 103, 107, 109,
        113,
    ];
    let mut rng = 20260921u64;
    for trial in 0..24 {
        let mut e = engine(113);
        e.cfg.probes = trial % 2 == 0;
        e.policy = if trial % 3 == 0 {
            Policy::StrictMaximumRowFirst
        } else {
            Policy::Mrv
        };
        let mut s = State::new(&e.p);
        e.initialize(&mut s).unwrap();
        let mut forced = vec![];
        let mut bans = vec![];
        for v in 1..=113 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            if rng >> 62 != 0 {
                continue;
            }
            if example.contains(&v) {
                forced.push(v);
                assume(&mut e, &mut s, Fact::Member(v)).unwrap();
            } else {
                bans.push(v);
                assume(&mut e, &mut s, Fact::Ban(v)).unwrap();
            }
        }
        let Outcome::Yes(a) = e.dfs(&mut s) else {
            panic!("valid extension lost");
        };
        assert!(validate(113, &a));
        assert!(forced.iter().all(|v| a.contains(v)));
        assert!(bans.iter().all(|v| !a.contains(v)));
    }
}

#[test]
fn all_search_and_probe_proof_nodes_are_scope_valid() {
    use super::engine::Outcome;
    for probes in [false, true] {
        let mut e = engine(113);
        e.cfg.probes = probes;
        let mut s = State::new(&e.p);
        e.initialize(&mut s).unwrap();
        assert!(matches!(e.dfs(&mut s), Outcome::Yes(_)));
        let cert = proof::Certificate {
            maximum: 113,
            nodes: s.proof.nodes.clone(),
            scopes: s.proof.scopes.clone(),
            root: 0,
        };
        assert_eq!(
            proof::verify(&cert, false),
            Err("not a complete root refutation".into())
        );
    }
}

#[test]
fn shared_root_strengthening_preserves_boundary_and_positive_controls() {
    for n in [1, 2, 3, 31, 97, 112, 113, 114, 257] {
        let report = run(
            Config {
                maximum: n,
                root_strengthen: true,
                threads: 2,
                ..Config::default()
            },
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(
            report.status,
            if n == 2 || n == 113 { "YES" } else { "NO" },
            "{:?}",
            report.reason
        );
        if let Some(cert) = report.certificate {
            assert_eq!(proof::verify(&cert, false), Ok(false));
        }
        let used = report.root_metrics.probe_events
            + report
                .lanes
                .iter()
                .map(|l| l.metrics.probe_events)
                .sum::<u64>();
        assert!(used <= report.config.probe_events as u64);
    }
}

#[test]
fn member_limited_probe_keeps_parent_queues_and_facts() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    e.initialize(&mut s).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    e.cfg.probe_members = 1;
    let before = fingerprint(&s);
    let v = (1..=113)
        .find(|&v| s.member[v].is_none() && s.banned[v].is_none())
        .unwrap();
    let r = e.probe(&mut s, vec![Fact::Member(v)], 1000000);
    assert_eq!(fingerprint(&s), before);
    assert!(s.quiet());
    counts(&e, &s);
    if r.conflict.is_none() {
        assert!(r.delta.contains_key(&Fact::Member(v)));
    }
}

#[test]
fn root_probes_cover_small_sums_and_repeat_only_on_progress() {
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    e.probe_remaining = 2000000;
    e.initialize(&mut s).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    assert_eq!(e.strengthen_root(&mut s), Propagation::Quiet);
    assert!(s.quiet());
    counts(&e, &s);
    assert!(e.metrics.root_strengthen_rounds > 0);
    assert!(e.metrics.root_cover_sums.iter().any(|&sum| sum < 113));
    let cert = proof::Certificate {
        maximum: 113,
        nodes: s.proof.nodes.clone(),
        scopes: s.proof.scopes.clone(),
        root: 0,
    };
    assert_eq!(
        proof::verify(&cert, false),
        Err("not a complete root refutation".into())
    );
}

#[test]
fn parallel_root_merges_scopes_and_retains_positive_controls() {
    // One worker exercises repeated task reuse deterministically as well.
    for threads in [1, 2, 4, 24] {
        let mut e = engine(113);
        let mut s = State::new(&e.p);
        e.probe_remaining = 2000000;
        e.cfg.probe_case_events = 2000;
        e.initialize(&mut s).unwrap();
        assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
        assert_eq!(
            parallel_root::strengthen(&mut e, &mut s, threads).unwrap(),
            Propagation::Quiet
        );
        assert!(s.quiet());
        counts(&e, &s);
        assert!(e.metrics.root_parallel_batches > 1);
        assert!(e.metrics.root_parallel_workers >= threads.min(2));
        assert!(e.metrics.root_parallel_workers <= threads);
        assert!(e.metrics.probe_events <= 2000000);
        assert_eq!(e.probe_remaining + e.metrics.probe_events as usize, 2000000);
        // Include every imported node, not just a winning dependency closure.
        let cert = proof::Certificate {
            maximum: 113,
            nodes: (0..s.proof.len())
                .map(|id| s.proof.get(id).clone())
                .collect(),
            scopes: s.proof.scopes.clone(),
            root: 0,
        };
        assert_eq!(
            proof::verify(&cert, false),
            Err("not a complete root refutation".into())
        );
        e.cfg.probes = false;
        let engine::Outcome::Yes(values) = e.dfs(&mut s) else {
            panic!("lost positive control")
        };
        assert!(validate(113, &values));
    }
}

#[test]
fn parallel_root_respects_shared_budget_and_interruption() {
    for budget in [0, 1, 13, 1000] {
        let mut e = engine(113);
        let mut s = State::new(&e.p);
        e.initialize(&mut s).unwrap();
        assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
        e.probe_remaining = budget;
        e.cfg.probe_case_events = 1;
        e.cfg.probe_members = 1;
        assert_eq!(
            parallel_root::strengthen(&mut e, &mut s, 24).unwrap(),
            Propagation::Quiet
        );
        counts(&e, &s);
        assert!(s.quiet());
        assert_eq!(e.probe_remaining + e.metrics.probe_events as usize, budget);
        let engine::Outcome::Yes(values) = e.dfs(&mut s) else {
            panic!("paused probe refuted positive control")
        };
        assert!(validate(113, &values));
    }
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    e.initialize(&mut s).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    let before = fingerprint(&s);
    e.limits
        .interrupted
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        parallel_root::strengthen(&mut e, &mut s, 24).unwrap(),
        Propagation::Paused
    );
    assert_eq!(fingerprint(&s), before);
    assert_eq!(e.metrics.root_parallel_batches, 0);
}

#[test]
fn complete_prefix_mode_preserves_positive_control_and_marks_dependencies() {
    let yes = run(
        Config {
            maximum: 113,
            complete_prefix_base: Some(2),
            root_strengthen: true,
            threads: 2,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(yes.status, "YES");
    assert!(validate(113, &yes.example));

    let no = run(
        Config {
            maximum: 4,
            complete_prefix_base: Some(2),
            threads: 1,
            ..Config::default()
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(no.status, "NO");
    assert_eq!(no.evidence, "SOLVER_NO_EXTERNAL_LEMMAS");
    let certificate = no.certificate.unwrap();
    assert_eq!(proof::verify(&certificate, true), Ok(true));
    assert!(proof::verify(&certificate, false).is_err());
    assert!(
        run(
            Config {
                maximum: 113,
                complete_prefix_base: Some(113),
                ..Config::default()
            },
            Arc::new(AtomicBool::new(false)),
        )
        .is_err()
    );
}

#[test]
fn additive_maximum_unique_requires_every_other_pair_excluded() {
    let certificate = proof::Certificate {
        maximum: 5,
        scopes: vec![
            proof::Scope {
                parent: None,
                assumptions: vec![],
            },
            proof::Scope {
                parent: Some(0),
                assumptions: vec![Fact::Ban(2)],
            },
        ],
        nodes: vec![
            proof::Node {
                scope: 0,
                fact: Fact::AdditiveMaximum,
                rule: Rule::PrefixAdditive { base: 2 },
                premises: vec![],
            },
            proof::Node {
                scope: 1,
                fact: Fact::Ban(2),
                rule: Rule::Assumption,
                premises: vec![],
            },
            proof::Node {
                scope: 1,
                fact: Fact::Member(4),
                rule: Rule::AdditiveUnique { a: 1 },
                premises: vec![0, 1],
            },
        ],
        root: 2,
    };
    assert_eq!(
        proof::verify(&certificate, true),
        Err("not a complete root refutation".into())
    );
    let mut forged = certificate;
    forged.nodes[2].premises.pop();
    assert!(
        proof::verify(&forged, true)
            .unwrap_err()
            .contains("invalid inference")
    );
}

#[test]
fn additive_maximum_conflict_rolls_back_pair_and_ban_deletions() {
    let mut e = engine(31);
    let mut s = State::new(&e.p);
    e.enable_maximum_deletion(&mut s, 3).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    let parent = fingerprint(&s);
    let cp = s.checkpoint();

    // A pair excludes the first additive witness; bans exclude the rest.
    assume(&mut e, &mut s, Fact::Pair(1, 30)).unwrap();
    for v in 2..=15 {
        assume(&mut e, &mut s, Fact::Ban(v)).unwrap();
    }
    assert!(matches!(e.quiesce(&mut s), Propagation::Conflict(_)));
    s.rollback(&e.p, cp);
    assert_eq!(fingerprint(&s), parent);
    counts(&e, &s);

    // After rollback, the first witness is live again, so banning the other
    // values forces 1 before the ordinary sum/product rules find a conflict.
    for v in 2..=15 {
        assume(&mut e, &mut s, Fact::Ban(v)).unwrap();
    }
    assert!(matches!(e.quiesce(&mut s), Propagation::Conflict(_)));
    assert!(s.member[1].is_some());
}

#[test]
fn additive_maximum_is_a_dfs_branch_when_product_demands_are_absent() {
    let mut e = engine(31);
    let mut s = State::new(&e.p);
    e.enable_maximum_deletion(&mut s, 3).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    assert_eq!(e.select(&s), None);
    e.cfg.node_limit = 1;
    assert!(matches!(e.dfs(&mut s), Outcome::Unknown));
    assert_eq!(e.metrics.yes_checks, 0);
    assert_eq!(e.metrics.full_domain_enumerations, 1);
    assert_eq!(e.metrics.additive_branches, 1);
}

// Independent audit: each fact is checked against an explicit arithmetic model,
// rather than against the engine's own proof rules or counters.
#[test]
fn audit_known_models_preserved_by_mixed_events_and_rollback() {
    let smaller = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37, 39, 41,
        43, 45, 47, 49, 53, 55, 59, 61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 99, 101, 103, 107, 109,
        113,
    ];
    let larger = (1..=13)
        .chain([15, 16, 17])
        .chain((19..=109).step_by(2))
        .chain([113])
        .collect::<Vec<_>>();
    for model in [smaller, larger] {
        assert!(validate(113, &model));
        let mut member = [false; 114];
        let mut product = [false; 227];
        let mut sum = [false; 227];
        for &a in &model {
            member[a] = true;
            for &b in &model {
                if a * b <= 226 {
                    product[a * b] = true;
                }
                sum[a + b] = true;
            }
        }
        let check = |e: &Engine, s: &State| {
            for (v, &present) in member.iter().enumerate().skip(1) {
                assert!(s.member[v].is_none() || present, "unsound member {v}");
                assert!(s.banned[v].is_none() || !present, "unsound ban {v}");
            }
            for v in 2..=226 {
                assert!(s.bad[v].is_none() || !product[v], "unsound bad product {v}");
                assert!(s.active[v].is_none() || sum[v], "unsound active sum {v}");
            }
            for &(d, f) in s.pairs.keys() {
                assert!(!member[d] || !member[f], "unsound nogood {d},{f}");
            }
            for (id, w) in e.p.witness.iter().enumerate() {
                assert!(
                    s.dead[id].is_none() || !member[w.d] || !member[w.e],
                    "unsound deletion {},{}",
                    w.d,
                    w.e
                );
            }
        };
        let mut rng = 0x20260926u64;
        let mut rand = || {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            rng
        };
        for trial in 0..120 {
            let mut e = engine(113);
            e.cfg.probe_members = 1 + (trial % 7);
            e.cfg.probe_case_events = 1 + (trial % 31);
            e.cfg.prime_chain_steps = 3000;
            let mut s = State::new(&e.p);
            e.initialize(&mut s).unwrap();
            if trial % 2 == 0 {
                e.enable_maximum_deletion(&mut s, 2).unwrap();
            }
            assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
            check(&e, &s);
            let parent = fingerprint(&s);
            let cp = s.checkpoint();
            for step in 0..24 {
                let v = 1 + (rand() as usize % 113);
                let w = 1 + (rand() as usize % 113);
                let t = 2 + (rand() as usize % 225);
                let f = match step % 4 {
                    0 => {
                        if member[v] {
                            Fact::Member(v)
                        } else {
                            Fact::Ban(v)
                        }
                    }
                    1 if !member[v] || !member[w] => Fact::Pair(v, w),
                    2 if !product[t] => Fact::Bad(t),
                    _ => {
                        if member[w] {
                            Fact::Member(w)
                        } else {
                            Fact::Ban(w)
                        }
                    }
                };
                assert!(assume(&mut e, &mut s, f).is_ok());
                let mut budget = rand() as usize % 13;
                assert!(!matches!(
                    e.propagate(&mut s, &mut budget),
                    Propagation::Conflict(_)
                ));
                check(&e, &s);
            }
            s.rollback(&e.p, cp);
            assert_eq!(fingerprint(&s), parent);
            counts(&e, &s);
            assert_eq!(e.prime_chains(&mut s), Propagation::Quiet);
            check(&e, &s);
            if trial % 20 == 0 {
                e.probe_remaining = 3000;
                assert_eq!(
                    parallel_root::strengthen(&mut e, &mut s, 4).unwrap(),
                    Propagation::Quiet
                );
                check(&e, &s);
                counts(&e, &s);
            }
        }
    }
}

// The word-parallel chain search must find a terminal exactly when the plain
// closure of odd members under steps 2 and e does, and return a valid path.
#[test]
fn prime_chain_search_matches_naive_closure_and_returns_a_valid_path() {
    use std::collections::{HashSet, VecDeque};
    fn naive(e: &Engine, s: &State, candidate: usize) -> bool {
        let n = e.p.n;
        let mut known = s
            .members
            .iter()
            .copied()
            .filter(|a| a % 2 == 1)
            .collect::<HashSet<_>>();
        let mut queue = known.iter().copied().collect::<VecDeque<_>>();
        while let Some(a) = queue.pop_front() {
            for h in [2, candidate] {
                let p = a + h;
                if !e.p.prime[p] || !known.insert(p) {
                    continue;
                }
                if p > n || s.banned[p].is_some() {
                    return true;
                }
                queue.push_back(p);
            }
        }
        false
    }
    let mut states = vec![];
    let mut e = engine(113);
    let mut s = State::new(&e.p);
    e.initialize(&mut s).unwrap();
    assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
    states.push((e, s));
    let mut rng = 0x7e57_c4a1u64;
    for n in [60, 200, 1000, 5000] {
        for _ in 0..6 {
            let mut e = engine(n);
            let mut s = State::new(&e.p);
            let mut ok = true;
            for v in [1, 2] {
                ok &= assume(&mut e, &mut s, Fact::Member(v)).is_ok();
            }
            for _ in 0..(n / 20).max(4) {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let v = 3 + (rng >> 33) as usize % (n - 2);
                let f = if rng & (1 << 20) == 0 {
                    Fact::Member(v)
                } else {
                    Fact::Ban(v)
                };
                if s.member[v].is_some() || s.banned[v].is_some() {
                    continue;
                }
                ok &= assume(&mut e, &mut s, f).is_ok();
            }
            if ok && e.quiesce(&mut s) == Propagation::Quiet {
                states.push((e, s));
            }
        }
    }
    let mut found = 0;
    let mut tested = 0;
    for (mut e, s) in states {
        let n = e.p.n;
        let mut search = engine::ChainSearch::new(&e.p);
        for candidate in (2..=n).step_by(2) {
            if s.member[candidate].is_some() || s.banned[candidate].is_some() {
                continue;
            }
            tested += 1;
            let mut left = usize::MAX;
            let chain = e.prime_chain(&s, &mut search, candidate, &mut left);
            assert_eq!(
                chain.is_some(),
                naive(&e, &s, candidate),
                "n={n} e={candidate}"
            );
            let Some(steps) = chain else { continue };
            found += 1;
            let mut known = HashSet::from([steps[0].0]);
            assert!(s.member[steps[0].0].is_some());
            for (i, &(a, h, p)) in steps.iter().enumerate() {
                assert!(known.contains(&a) && (h == 2 || h == candidate) && a + h == p);
                assert!(e.p.prime[p]);
                let terminal = p > n || s.banned[p].is_some();
                assert_eq!(terminal, i + 1 == steps.len());
                known.insert(p);
            }
        }
    }
    assert!(found > 0 && tested > found, "found {found} of {tested}");
}

#[test]
fn path_only_prime_chain_proofs_pass_the_independent_checker() {
    let mut chains = 0;
    for n in 100..=400 {
        let mut e = engine(n);
        let mut s = State::new(&e.p);
        let result = match e.initialize(&mut s) {
            Err(id) => Propagation::Conflict(id),
            Ok(()) => match e.quiesce(&mut s) {
                Propagation::Quiet => e.prime_chains(&mut s),
                other => other,
            },
        };
        let used = (0..s.proof.len())
            .filter(|&id| matches!(s.proof.get(id).rule, Rule::PrimeChain { .. }))
            .count();
        chains += used;
        let cert = proof::Certificate {
            maximum: n,
            nodes: (0..s.proof.len())
                .map(|id| s.proof.get(id).clone())
                .collect(),
            scopes: s.proof.scopes.clone(),
            root: match result {
                Propagation::Conflict(id) => id,
                _ => 0,
            },
        };
        match result {
            Propagation::Conflict(_) => assert_eq!(proof::verify(&cert, false), Ok(false)),
            _ => assert_eq!(
                proof::verify(&cert, false),
                Err("not a complete root refutation".into())
            ),
        }
    }
    assert!(chains > 0);
}

// Roots used to exercise parallel machinery: real initialized roots and
// synthetic ones that assume small members without making n a member.
fn exercise_roots() -> Vec<(Engine, State)> {
    let example = [
        1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 16, 17, 19, 21, 23, 25, 27, 29, 31, 35, 37, 39, 41,
        43, 45, 47, 49, 53, 55, 59, 61, 67, 71, 73, 77, 79, 83, 89, 91, 97, 99, 101, 103, 107, 109,
        113,
    ];
    let mut roots = vec![];
    for (n, members, additive) in [
        (113, &[][..], false),
        (31, &[1, 2][..], true),
        (160, &[1, 2][..], false),
        (300, &example[..12], false),
        (500, &example[..], true),
    ] {
        let mut e = engine(n);
        let mut s = State::new(&e.p);
        let mut ok = if members.is_empty() {
            e.initialize(&mut s).is_ok()
        } else {
            members
                .iter()
                .all(|&v| assume(&mut e, &mut s, Fact::Member(v)).is_ok())
        };
        if additive {
            ok &= e.enable_maximum_deletion(&mut s, 3).is_ok();
        }
        assert!(ok && e.quiesce(&mut s) == Propagation::Quiet, "n={n}");
        roots.push((e, s));
    }
    roots
}

// A retained worker copy must equal the root after replaying the root's undo
// entries, including after it has probed and rolled back its own changes.
#[test]
fn worker_sync_replays_root_changes_exactly() {
    let mut synced = 0;
    for (mut e, mut root) in exercise_roots() {
        let n = e.p.n;
        root.freeze_changes();
        let mut copy = root.clone();
        let mut rng = 0x005e_ed0f_5eed_u64 + n as u64;
        for round in 0..40 {
            // The worker's own probes are rolled back before the next sync.
            let v = 3 + (round * 7) % (n - 3);
            if copy.member[v].is_none() && copy.banned[v].is_none() {
                e.probe(&mut copy, vec![Fact::Ban(v)], 5000);
                let cp = copy.checkpoint();
                if let Some(sum) = e.select(&copy)
                    && copy.product_count[sum] == 0
                {
                    let (cases, ctx) = e.cover_context(&mut copy, sum);
                    let _ = e.cover(&mut copy, proof::Cover::Factor(sum), cases, ctx, 5000);
                    let _ = e.quiesce(&mut copy);
                }
                copy.rollback(&e.p, cp);
                copy.proof = root.proof.clone();
            }
            let cp = root.checkpoint();
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = 3 + (rng >> 33) as usize % (n - 3);
            let w = 3 + (rng >> 13) as usize % (n - 3);
            let f = match rng % 4 {
                0 | 1 => Fact::Ban(v),
                2 => Fact::Member(v),
                _ => Fact::Pair(v, w),
            };
            if assume(&mut e, &mut root, f).is_err() || e.quiesce(&mut root) != Propagation::Quiet {
                root.rollback(&e.p, cp);
                continue;
            }
            let changes = root.freeze_changes();
            copy.sync_from(&e.p, &root, &changes);
            assert_eq!(
                fingerprint(&copy),
                fingerprint(&root),
                "n={n} round={round}"
            );
            counts(&e, &copy);
            synced += 1;
        }
    }
    assert!(synced >= 60, "only {synced} synchronized rounds");
}

// A job that reserves the whole remaining budget used to run alone. Speculative
// batches must reproduce that trajectory exactly while running its successors
// in parallel: same root facts, proof arena, budget use, and productive jobs.
// Finite budgets also exercise the transition back to ordinary batches.
#[test]
fn speculative_root_batches_reproduce_the_serial_trajectory() {
    let (mut speculative, mut discarded, mut compared) = (0, 0, 0);
    for (case_events, budget) in [(usize::MAX, usize::MAX), (3000, 9000), (40000, 100000)] {
        let run = |e: &mut Engine, s: &mut State, speculate: bool| {
            e.cfg.no_root_speculation = !speculate;
            e.cfg.probe_case_events = case_events;
            e.cfg.probe_members = 0;
            e.probe_remaining = budget;
            parallel_root::strengthen(e, s, 4).unwrap()
        };
        for ((mut serial, mut serial_state), (mut parallel, mut parallel_state)) in
            exercise_roots().into_iter().zip(exercise_roots())
        {
            let n = serial.p.n;
            let serial_result = run(&mut serial, &mut serial_state, false);
            let parallel_result = run(&mut parallel, &mut parallel_state, true);
            assert_eq!(serial.metrics.root_speculative_batches, 0);
            speculative += parallel.metrics.root_speculative_batches;
            discarded += parallel.metrics.root_discarded_jobs;
            match (serial_result, parallel_result) {
                (Propagation::Conflict(_), Propagation::Conflict(_)) => {}
                (a, b) => {
                    assert_eq!((a, b), (Propagation::Quiet, Propagation::Quiet), "n={n}");
                    compared += 1;
                    assert_eq!(
                        fingerprint(&serial_state),
                        fingerprint(&parallel_state),
                        "n={n}"
                    );
                    assert_eq!(serial_state.proof.len(), parallel_state.proof.len());
                    assert_eq!(serial.probe_remaining, parallel.probe_remaining, "n={n}");
                    assert_eq!(serial.metrics.probe_events, parallel.metrics.probe_events);
                    assert_eq!(
                        serial.metrics.root_cover_sums,
                        parallel.metrics.root_cover_sums
                    );
                    assert_eq!(
                        serial.metrics.root_forced_absences,
                        parallel.metrics.root_forced_absences
                    );
                    assert_eq!(
                        serial.metrics.root_strengthen_rounds,
                        parallel.metrics.root_strengthen_rounds
                    );
                    assert!(
                        parallel.metrics.root_parallel_batches
                            <= serial.metrics.root_parallel_batches
                    );
                }
            }
        }
    }
    assert!(compared >= 9 && speculative > 6 && discarded > 0);
}

// Every quiescent state must be closed under each propagation rule. This is
// checked directly against the arithmetic tables, independently of the event
// order and of the bitsets and caches used to reach the fixpoint.
fn assert_closed(e: &Engine, s: &State) {
    let (n, limit) = (e.p.n, e.p.limit);
    let members = (1..=n)
        .filter(|&v| s.member[v].is_some())
        .collect::<Vec<_>>();
    for v in 1..=n {
        assert!(
            s.member[v].is_none() || s.banned[v].is_none(),
            "member and ban {v}"
        );
        if s.banned[v].is_some() {
            for &id in e.p.endpoint.get(v) {
                assert!(s.dead[id].is_some(), "ban {v} kept a witness");
            }
        }
    }
    for (i, &a) in members.iter().enumerate() {
        for &b in &members[i..] {
            assert!(s.active[a + b].is_some(), "sum {a}+{b} inactive");
        }
    }
    for (id, w) in e.p.witness.iter().enumerate() {
        let both = s.member[w.d].is_some() && s.member[w.e].is_some();
        assert!(
            !(both && s.dead[id].is_some()),
            "dead product {}*{}",
            w.d,
            w.e
        );
    }
    for sum in 2..=limit {
        let domain = e.p.domain(sum);
        assert!(
            s.active[sum].is_none() || s.bad[sum].is_none(),
            "active bad {sum}"
        );
        if s.bad[sum].is_some() {
            assert_eq!(s.product_count[sum], 0);
            assert!(
                domain.clone().all(|id| s.dead[id].is_some()),
                "bad {sum} product"
            );
            for &id in e.p.endpoint_sum.get(sum) {
                assert!(s.dead[id].is_some(), "bad {sum} sum witness");
            }
            if sum % 2 == 0 && sum / 2 <= n {
                assert!(s.banned[sum / 2].is_some(), "bad {sum} half");
            }
            for &a in &members {
                if a < sum && sum - a <= n {
                    assert!(s.banned[sum - a].is_some(), "bad {sum} minus member {a}");
                }
            }
        }
        if s.live_count[sum] == 0 && !domain.is_empty() {
            assert!(s.bad[sum].is_some(), "empty {sum} not bad");
        }
        let unresolved = s.unresolved.contains(&sum);
        if s.active[sum].is_some() {
            assert!(
                s.product_count[sum] > 0 || s.live_count[sum] >= 2,
                "unique {sum}"
            );
            assert_eq!(unresolved, s.product_count[sum] == 0, "unresolved {sum}");
        } else {
            assert!(!unresolved, "inactive unresolved {sum}");
        }
    }
    for &(d, f) in s.pairs.keys() {
        assert!(d < f);
        if let Some(id) = e.p.pair_id(d, f) {
            assert!(s.dead[id].is_some(), "pair {d},{f} witness");
        }
        assert!(
            s.member[d].is_none() || s.banned[f].is_some(),
            "pair {d},{f}"
        );
        assert!(
            s.member[f].is_none() || s.banned[d].is_some(),
            "pair {f},{d}"
        );
    }
}

#[test]
fn quiescent_states_are_closed_and_every_proof_node_checks() {
    let mut rng = 0x00c1_05ed_u64;
    let mut rand = move |m: usize| {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 33) as usize % m
    };
    let (mut closed, mut refuted) = (0, 0);
    for (n, initialized) in [
        (113, true),
        (60, false),
        (200, false),
        (700, false),
        (3000, false),
    ] {
        let mut e = engine(n);
        e.cfg.probe_members = 0;
        e.cfg.prime_chain_steps = 100000;
        let mut s = State::new(&e.p);
        if initialized {
            e.initialize(&mut s).unwrap();
        } else {
            // Checkable synthetic premises: a child scope assuming 1 and 2.
            let scope = s.proof.enter(0, vec![Fact::Member(1), Fact::Member(2)]);
            s.scope = scope;
            for v in [1, 2] {
                assume(&mut e, &mut s, Fact::Member(v)).unwrap();
            }
        }
        assert_eq!(e.quiesce(&mut s), Propagation::Quiet);
        assert_closed(&e, &s);
        assert_eq!(e.prime_chains(&mut s), Propagation::Quiet);
        assert_closed(&e, &s);
        let base = fingerprint(&s);
        for _ in 0..60 {
            let cp = s.checkpoint();
            let facts = (0..1 + rand(3))
                .map(|_| {
                    let v = 3 + rand(n - 2);
                    if rand(3) == 0 {
                        Fact::Member(v)
                    } else {
                        Fact::Ban(v)
                    }
                })
                .filter(|f| matches!(f, Fact::Member(v) | Fact::Ban(v) if s.member[*v].is_none() && s.banned[*v].is_none()))
                .collect::<Vec<_>>();
            if facts.is_empty() {
                continue;
            }
            let child = s.proof.enter(s.scope, facts.clone());
            s.scope = child;
            let mut result = Propagation::Quiet;
            for f in facts {
                if let Err(id) = assume(&mut e, &mut s, f) {
                    result = Propagation::Conflict(id);
                    break;
                }
            }
            if result == Propagation::Quiet {
                result = e.quiesce(&mut s);
            }
            if result == Propagation::Quiet {
                assert_closed(&e, &s);
                closed += 1;
                // Nested cover and absence probes create and discard cached
                // deletion proofs in grandchild scopes.
                if let Some(sum) = e.select(&s) {
                    let (cases, ctx) = e.cover_context(&mut s, sum);
                    let r = e.cover(&mut s, proof::Cover::Factor(sum), cases, ctx, 200000);
                    result = match r {
                        Err(id) => Propagation::Conflict(id),
                        Ok(()) => e.quiesce(&mut s),
                    };
                }
                if result == Propagation::Quiet {
                    assert_closed(&e, &s);
                    result = e.prime_chains(&mut s);
                }
                if result == Propagation::Quiet {
                    assert_closed(&e, &s);
                }
            }
            if let Propagation::Conflict(id) = result {
                refuted += 1;
                assert_eq!(s.proof.get(id).fact, Fact::False);
            }
            s.rollback(&e.p, cp);
            assert_eq!(fingerprint(&s), base);
        }
        // All nodes, including those of rolled-back and refuted scopes, must
        // satisfy the independent checker's rules.
        let cert = proof::Certificate {
            maximum: n,
            nodes: (0..s.proof.len())
                .map(|id| s.proof.get(id).clone())
                .collect(),
            scopes: s.proof.scopes.clone(),
            root: 0,
        };
        assert_eq!(
            proof::verify(&cert, false),
            Err("not a complete root refutation".into()),
            "n={n}"
        );
    }
    assert!(
        closed >= 100 && refuted >= 20,
        "closed {closed}, refuted {refuted}"
    );
}

// Roots refuted during parallel strengthening must yield checkable complete
// refutations and exact budget accounting, including speculative batches in
// which several jobs each reserved the whole remaining budget.
#[test]
fn parallel_root_refutations_check_and_budgets_balance() {
    let mut refuted = 0;
    for n in 115..=420 {
        for (case_events, budget) in [
            (usize::MAX, usize::MAX),
            (4000, 12000),
            (usize::MAX, 50000),
            (usize::MAX, 2400),
            (usize::MAX, 1600),
        ] {
            let mut e = engine(n);
            e.cfg.probe_case_events = case_events;
            e.cfg.probe_members = 0;
            e.probe_remaining = budget;
            let mut s = State::new(&e.p);
            if e.initialize(&mut s).is_err()
                || e.quiesce(&mut s) != Propagation::Quiet
                || e.prime_chains(&mut s) != Propagation::Quiet
            {
                break;
            }
            let result = parallel_root::strengthen(&mut e, &mut s, 4).unwrap();
            assert_eq!(e.probe_remaining + e.metrics.probe_events as usize, budget);
            if let Propagation::Conflict(id) = result {
                refuted += 1;
                let cert = s.proof.certificate(n, id);
                assert_eq!(proof::verify(&cert, false), Ok(false), "n={n}");
            }
        }
    }
    assert!(refuted >= 3, "only {refuted} refutations");
}

#[test]
fn bit_windows_match_naive_extraction_at_every_offset() {
    let mut rng = 0x0b17_5eed_u64;
    let bits = (0..5)
        .map(|_| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            rng
        })
        .collect::<Vec<_>>();
    let bit = |i: isize| {
        usize::try_from(i)
            .ok()
            .filter(|&i| i < 64 * bits.len())
            .is_some_and(|i| bits[i / 64] >> (i % 64) & 1 == 1)
    };
    for start in -200..(64 * bits.len() as isize + 70) {
        let expected = (0..64).fold(0u64, |w, b| w | (u64::from(bit(start + b)) << b));
        assert_eq!(engine::window(&bits, start), expected, "start={start}");
    }
}
