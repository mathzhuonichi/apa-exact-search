use super::engine::{Engine, Limits, Metrics, Policy, Propagation, State};
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
        "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{}{}",
        s.member,
        s.banned,
        s.bad,
        s.active,
        s.dead,
        s.members,
        s.bad_values,
        s.live_count,
        s.xor_live_id,
        s.product_count,
        s.unresolved,
        pairs,
        s.revision,
        s.scope
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
    let (cases, ctx) = e.cover_context(&s, sum);
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
