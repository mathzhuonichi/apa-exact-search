use super::problem::Problem;
use super::proof::{Arena, Cover, Fact, Id, Rule};
use super::runner::Config;
use super::{GROUPS, U, seed_cases, validate};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, clap::ValueEnum)]
pub enum Policy {
    Mrv,
    StrictMaximumRowFirst,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Metrics {
    pub nodes: u64,
    pub conflicts: u64,
    pub yes_checks: u64,
    pub witness_kills: u64,
    pub repeated_kill_attempts: u64,
    pub endpoint_occ_visits: u64,
    pub endpoint_sum_occ_visits: u64,
    pub full_domain_enumerations: u64,
    pub additive_branches: u64,
    pub dirty_domain_events: u64,
    pub low_pair_activations: u64,
    pub low_pair_word_scans: u64,
    pub new_members: u64,
    pub bans: u64,
    pub bad_products: u64,
    pub product_counter_updates: u64,
    pub prime_clause_fires: u64,
    pub even_bound_fires: u64,
    pub probe_calls: u64,
    pub probe_refutations: u64,
    pub probe_pauses: u64,
    pub probe_unstarted_cases: u64,
    pub probe_common_members: u64,
    pub probe_common_bans: u64,
    pub probe_common_e: u64,
    pub probe_common_g: u64,
    pub probe_events: u64,
    pub probe_wall_time: f64,
    pub prime_chain_steps: u64,
    pub prime_chain_wall_time: f64,
    pub root_batch_worker_wall_time: f64,
    pub root_batch_merge_wall_time: f64,
    pub root_parent_propagation_wall_time: f64,
    pub root_worker_clone_cpu_time: f64,
    pub root_worker_fragment_cpu_time: f64,
    pub proof_events: usize,
    pub root_strengthen_rounds: usize,
    pub root_cover_sums: Vec<usize>,
    pub root_forced_absences: Vec<usize>,
    pub root_parallel_batches: usize,
    pub root_parallel_workers: usize,
}
impl Metrics {
    pub(super) fn add_probe_work(&mut self, other: &Self) {
        macro_rules! add {
            ($($field:ident),* $(,)?) => { $(self.$field += other.$field;)* };
        }
        add!(
            conflicts,
            witness_kills,
            repeated_kill_attempts,
            endpoint_occ_visits,
            endpoint_sum_occ_visits,
            full_domain_enumerations,
            additive_branches,
            dirty_domain_events,
            low_pair_activations,
            low_pair_word_scans,
            new_members,
            bans,
            bad_products,
            product_counter_updates,
            prime_clause_fires,
            even_bound_fires,
            probe_calls,
            probe_refutations,
            probe_pauses,
            probe_unstarted_cases,
            probe_common_members,
            probe_common_bans,
            probe_common_e,
            probe_common_g,
            probe_events
        );
        self.root_worker_clone_cpu_time += other.root_worker_clone_cpu_time;
        self.root_worker_fragment_cpu_time += other.root_worker_fragment_cpu_time;
    }
}
#[derive(Clone, Debug)]
enum Event {
    Member(usize),
    Ban(usize),
    Bad(usize),
    Domain(usize),
    Additive,
}
#[derive(Clone, Debug)]
struct SumJob {
    a: usize,
    next: usize,
    end: usize,
}
#[derive(Clone, Debug)]
enum Undo {
    Member(usize),
    Ban(usize),
    Bad(usize),
    Active(usize),
    Kill(usize),
    Product(usize),
    Pair(usize, usize),
    Insert(usize),
    Remove { s: usize, pos: usize },
    AdditiveDead(usize),
    AdditiveSatisfied(Option<usize>),
}
#[derive(Clone, Debug)]
struct AdditiveMaximum {
    required: Id,
    dead: Vec<Option<Id>>,
    live: usize,
    satisfied: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct Checkpoint {
    trail: usize,
    revision: u64,
    scope: usize,
}
#[derive(Clone)]
pub struct State {
    pub member: Vec<Option<Id>>,
    pub banned: Vec<Option<Id>>,
    pub bad: Vec<Option<Id>>,
    pub active: Vec<Option<Id>>,
    pub dead: Vec<Option<Id>>,
    pub members: Vec<usize>,
    member_bits: Vec<u64>,
    member_words: Vec<usize>,
    active_bits: Vec<u64>,
    pub bad_values: Vec<usize>,
    pub live_count: Vec<usize>,
    pub xor_live_id: Vec<usize>,
    pub product_count: Vec<usize>,
    pub unresolved: Vec<usize>,
    position: Vec<Option<usize>>,
    pub pairs: HashMap<(usize, usize), Id>,
    additive: Option<AdditiveMaximum>,
    adjacency: Vec<Vec<usize>>,
    high: VecDeque<Event>,
    low: VecDeque<SumJob>,
    pending: Vec<u64>,
    epoch: u64,
    trail: Vec<Undo>,
    pub revision: u64,
    pub scope: usize,
    pub proof: Arena,
}
impl State {
    pub fn new(p: &Problem) -> Self {
        Self {
            member: vec![None; p.n + 1],
            banned: vec![None; p.n + 1],
            bad: vec![None; p.limit + 1],
            active: vec![None; p.limit + 1],
            dead: vec![None; p.witness.len()],
            members: vec![],
            member_bits: vec![0; (p.n + 64) / 64],
            member_words: vec![],
            active_bits: vec![0; (p.limit + 64) / 64 + 1],
            bad_values: vec![],
            live_count: (0..=p.limit).map(|s| p.domain(s).len()).collect(),
            xor_live_id: (0..=p.limit)
                .map(|s| p.domain(s).fold(0, |a, b| a ^ b))
                .collect(),
            product_count: vec![0; p.limit + 1],
            unresolved: vec![],
            position: vec![None; p.limit + 1],
            pairs: HashMap::new(),
            additive: None,
            adjacency: vec![vec![]; p.n + 1],
            high: VecDeque::new(),
            low: VecDeque::new(),
            pending: vec![0; p.limit + 1],
            epoch: 1,
            trail: vec![],
            revision: 0,
            scope: 0,
            proof: Arena::new(),
        }
    }
    pub fn quiet(&self) -> bool {
        self.high.is_empty() && self.low.is_empty()
    }
    #[cfg(test)]
    pub(super) fn fingerprint_extensions(&self) -> String {
        format!(
            "{:?}{:?}{:?}{:?}",
            self.member_bits, self.member_words, self.active_bits, self.additive
        )
    }
    pub fn checkpoint(&self) -> Checkpoint {
        assert!(self.quiet(), "checkpoint requires empty propagation queues");
        Checkpoint {
            trail: self.trail.len(),
            revision: self.revision,
            scope: self.scope,
        }
    }
    pub fn rollback(&mut self, p: &Problem, cp: Checkpoint) {
        self.high.clear();
        self.low.clear();
        self.epoch = self.epoch.checked_add(1).expect("queue epoch overflow");
        while self.trail.len() > cp.trail {
            match self.trail.pop().unwrap() {
                Undo::Member(v) => {
                    self.member[v] = None;
                    assert_eq!(self.members.pop(), Some(v));
                    let word = v / 64;
                    self.member_bits[word] &= !(1u64 << (v % 64));
                    if self.member_bits[word] == 0 {
                        assert_eq!(self.member_words.pop(), Some(word));
                    }
                }
                Undo::Ban(v) => self.banned[v] = None,
                Undo::Bad(s) => {
                    self.bad[s] = None;
                    assert_eq!(self.bad_values.pop(), Some(s));
                }
                Undo::Active(s) => {
                    self.active[s] = None;
                    self.active_bits[s / 64] &= !(1u64 << (s % 64));
                }
                Undo::Kill(id) => {
                    let s = p.witness[id].s;
                    self.dead[id] = None;
                    self.live_count[s] += 1;
                    self.xor_live_id[s] ^= id;
                }
                Undo::Product(s) => self.product_count[s] -= 1,
                Undo::Pair(d, e) => {
                    self.pairs.remove(&(d, e));
                    assert_eq!(self.adjacency[d].pop(), Some(e));
                    assert_eq!(self.adjacency[e].pop(), Some(d));
                }
                Undo::Insert(s) => {
                    assert_eq!(self.unresolved.pop(), Some(s));
                    self.position[s] = None;
                }
                Undo::Remove { s, pos } => {
                    if pos < self.unresolved.len() {
                        let moved = self.unresolved[pos];
                        self.unresolved.push(moved);
                        self.position[moved] = Some(self.unresolved.len() - 1);
                        self.unresolved[pos] = s;
                    } else {
                        self.unresolved.push(s);
                    }
                    self.position[s] = Some(pos);
                }
                Undo::AdditiveDead(a) => {
                    let additive = self.additive.as_mut().unwrap();
                    additive.dead[a] = None;
                    additive.live += 1;
                }
                Undo::AdditiveSatisfied(previous) => {
                    self.additive.as_mut().unwrap().satisfied = previous;
                }
            }
        }
        self.revision = cp.revision;
        self.scope = cp.scope;
    }
    fn dirty(&mut self, s: usize) {
        if self.pending[s] != self.epoch {
            self.pending[s] = self.epoch;
            self.high.push_back(Event::Domain(s));
        }
    }
    fn unresolved(&mut self, s: usize, include: bool) {
        match (self.position[s], include) {
            (None, true) => {
                self.position[s] = Some(self.unresolved.len());
                self.unresolved.push(s);
                self.trail.push(Undo::Insert(s));
            }
            (Some(pos), false) => {
                self.unresolved.swap_remove(pos);
                self.position[s] = None;
                if pos < self.unresolved.len() {
                    self.position[self.unresolved[pos]] = Some(pos);
                }
                self.trail.push(Undo::Remove { s, pos });
            }
            _ => {}
        }
    }
    pub fn node(&mut self, fact: Fact, rule: Rule, premises: Vec<Id>) -> Id {
        self.proof.add(self.scope, fact, rule, premises)
    }
    fn conflict(&mut self, premises: Vec<Id>) -> Id {
        self.node(Fact::False, Rule::Conflict, premises)
    }
    pub fn freeze(&mut self) {
        assert!(self.quiet());
        self.trail.clear();
        self.proof.freeze();
    }
    pub(super) fn delta(&self, cp: &Checkpoint) -> BTreeMap<Fact, Id> {
        self.trail[cp.trail..]
            .iter()
            .filter_map(|undo| match *undo {
                Undo::Member(v) => Some((Fact::Member(v), self.member[v].unwrap())),
                Undo::Ban(v) => Some((Fact::Ban(v), self.banned[v].unwrap())),
                Undo::Bad(v) => Some((Fact::Bad(v), self.bad[v].unwrap())),
                Undo::Pair(d, e) => Some((Fact::Pair(d, e), self.pairs[&(d, e)])),
                _ => None,
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct Limits {
    pub deadline: Option<Instant>,
    pub cancel: Arc<AtomicBool>,
    pub interrupted: Arc<AtomicBool>,
}
impl Limits {
    pub fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
            || self.interrupted.load(Ordering::Acquire)
            || self.deadline.is_some_and(|d| Instant::now() >= d)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Propagation {
    Quiet,
    Paused,
    Conflict(Id),
}
#[derive(Debug)]
pub enum Outcome {
    Yes(Vec<usize>),
    No(Id),
    Unknown,
    Error(String),
}
pub(super) struct Probe {
    pub scope: usize,
    pub conflict: Option<Id>,
    pub delta: BTreeMap<Fact, Id>,
}
pub struct Engine {
    pub p: Arc<Problem>,
    pub cfg: Config,
    pub policy: Policy,
    pub lane: usize,
    pub limits: Limits,
    pub metrics: Metrics,
    pub probe_remaining: usize,
}
impl Engine {
    pub fn enable_maximum_deletion(&mut self, s: &mut State, base: usize) -> Result<(), Id> {
        assert_eq!(s.scope, 0);
        let n = self.p.n;
        let required = s.node(Fact::AdditiveMaximum, Rule::PrefixAdditive { base }, vec![]);
        let satisfied = (1..=n / 2).find(|&a| {
            s.member[a].is_some() && s.member[n - a].is_some() && !s.pairs.contains_key(&(a, n - a))
        });
        s.additive = Some(AdditiveMaximum {
            required,
            dead: vec![None; n / 2 + 1],
            live: n / 2,
            satisfied,
        });
        s.high.push_back(Event::Additive);
        for id in self.p.domain(n) {
            let w = self.p.witness[id];
            if w.e == n {
                continue;
            }
            let proof = s.node(Fact::Pair(w.d, w.e), Rule::PrefixPair { base }, vec![]);
            self.pair(s, w.d, w.e, proof)?;
        }
        Ok(())
    }
    fn additive_kill(&mut self, s: &mut State, v: usize, reason: Id) {
        let n = self.p.n;
        if v == 0 || v >= n {
            return;
        }
        let a = v.min(n - v);
        let Some(additive) = s.additive.as_mut() else {
            return;
        };
        if additive.dead[a].is_none() {
            additive.dead[a] = Some(reason);
            additive.live -= 1;
            s.trail.push(Undo::AdditiveDead(a));
            s.high.push_back(Event::Additive);
            s.revision += 1;
        }
    }
    fn additive_propagate(&mut self, s: &mut State) -> Result<(), Id> {
        let Some(additive) = s.additive.as_ref() else {
            return Ok(());
        };
        if additive.satisfied.is_some() || additive.live > 1 {
            return Ok(());
        }
        let required = additive.required;
        let survivor = (1..=self.p.n / 2).find(|&a| additive.dead[a].is_none());
        let reasons = std::iter::once(required)
            .chain(additive.dead.iter().skip(1).filter_map(|&id| id))
            .collect::<Vec<_>>();
        let Some(a) = survivor else {
            return Err(s.node(Fact::False, Rule::AdditiveEmpty, reasons));
        };
        for v in [a, self.p.n - a] {
            if s.member[v].is_none() {
                let proof = s.node(Fact::Member(v), Rule::AdditiveUnique { a }, reasons.clone());
                self.force(s, v, proof)?;
            }
        }
        Ok(())
    }
    pub fn force(&mut self, s: &mut State, v: usize, r: Id) -> Result<(), Id> {
        assert!((1..=self.p.n).contains(&v));
        if let Some(b) = s.banned[v] {
            return Err(s.conflict(vec![r, b]));
        }
        if s.member[v].is_some() {
            return Ok(());
        }
        s.member[v] = Some(r);
        s.trail.push(Undo::Member(v));
        s.members.push(v);
        let word = v / 64;
        if s.member_bits[word] == 0 {
            s.member_words.push(word);
        }
        s.member_bits[word] |= 1u64 << (v % 64);
        if v < self.p.n {
            let complement = self.p.n - v;
            if s.member[complement].is_some() {
                let a = v.min(complement);
                if let Some(additive) = s.additive.as_mut()
                    && additive.dead[a].is_none()
                    && additive.satisfied.is_none()
                {
                    s.trail.push(Undo::AdditiveSatisfied(None));
                    additive.satisfied = Some(a);
                }
            }
        }
        s.revision += 1;
        self.metrics.new_members += 1;
        for i in 0..self.p.endpoint.get(v).len() {
            let id = self.p.endpoint.get(v)[i];
            let w = self.p.witness[id];
            self.metrics.endpoint_occ_visits += 1;
            if let (Some(a), Some(b)) = (s.member[w.d], s.member[w.e]) {
                if let Some(dead) = s.dead[id] {
                    return Err(s.conflict(vec![a, b, dead]));
                }
                if let Some(bad) = s.bad[w.s] {
                    let dead = s.node(Fact::Dead(w.d, w.e), Rule::Delete, vec![bad]);
                    return Err(s.conflict(vec![a, b, dead]));
                }
                s.product_count[w.s] += 1;
                s.trail.push(Undo::Product(w.s));
                self.metrics.product_counter_updates += 1;
                if s.product_count[w.s] == 1 {
                    s.dirty(w.s);
                }
            }
        }
        s.high.push_back(Event::Member(v));
        s.low.push_back(SumJob {
            a: v,
            next: 0,
            end: s.member_words.len(),
        });
        Ok(())
    }
    pub fn ban(&mut self, s: &mut State, v: usize, r: Id) -> Result<(), Id> {
        if v > self.p.n {
            return Ok(());
        }
        assert!(v > 0);
        if let Some(m) = s.member[v] {
            return Err(s.conflict(vec![r, m]));
        }
        if s.banned[v].is_none() {
            s.banned[v] = Some(r);
            s.trail.push(Undo::Ban(v));
            s.high.push_back(Event::Ban(v));
            s.revision += 1;
            self.metrics.bans += 1;
            self.additive_kill(s, v, r);
        }
        Ok(())
    }
    pub fn kill(&mut self, s: &mut State, id: usize, r: Id) -> Result<(), Id> {
        if s.dead[id].is_some() {
            self.metrics.repeated_kill_attempts += 1;
            return Ok(());
        }
        let w = self.p.witness[id];
        if let (Some(a), Some(b)) = (s.member[w.d], s.member[w.e]) {
            return Err(s.conflict(vec![a, b, r]));
        }
        s.dead[id] = Some(r);
        s.trail.push(Undo::Kill(id));
        s.live_count[w.s] -= 1;
        s.xor_live_id[w.s] ^= id;
        s.dirty(w.s);
        s.revision += 1;
        self.metrics.witness_kills += 1;
        Ok(())
    }
    fn delete(&mut self, s: &mut State, id: usize, reason: Id) -> Result<(), Id> {
        if s.dead[id].is_some() {
            self.metrics.repeated_kill_attempts += 1;
            return Ok(());
        }
        let w = self.p.witness[id];
        let r = s.node(Fact::Dead(w.d, w.e), Rule::Delete, vec![reason]);
        self.kill(s, id, r)
    }
    pub fn bad(&mut self, s: &mut State, sum: usize, r: Id) -> Result<(), Id> {
        if s.bad[sum].is_some() {
            return Ok(());
        }
        if let Some(a) = s.active[sum] {
            return Err(s.conflict(vec![r, a]));
        }
        if s.product_count[sum] > 0 {
            let id = self
                .p
                .domain(sum)
                .find(|&id| {
                    let w = self.p.witness[id];
                    s.member[w.d].is_some() && s.member[w.e].is_some()
                })
                .unwrap();
            return self.delete(s, id, r);
        }
        s.bad[sum] = Some(r);
        s.bad_values.push(sum);
        s.trail.push(Undo::Bad(sum));
        s.high.push_back(Event::Bad(sum));
        s.revision += 1;
        self.metrics.bad_products += 1;
        Ok(())
    }
    pub fn pair(&mut self, s: &mut State, d: usize, e: usize, r: Id) -> Result<(), Id> {
        let (d, e) = (d.min(e), d.max(e));
        if d == e {
            let b = s.node(Fact::Ban(d), Rule::UnitPair, vec![r]);
            return self.ban(s, d, b);
        }
        if s.pairs.contains_key(&(d, e)) {
            return Ok(());
        }
        s.pairs.insert((d, e), r);
        s.adjacency[d].push(e);
        s.adjacency[e].push(d);
        s.trail.push(Undo::Pair(d, e));
        s.revision += 1;
        if d + e == self.p.n {
            self.additive_kill(s, d, r);
        }
        if let Some(m) = s.member[d] {
            let b = s.node(Fact::Ban(e), Rule::UnitPair, vec![r, m]);
            self.ban(s, e, b)?;
        }
        if let Some(m) = s.member[e] {
            let b = s.node(Fact::Ban(d), Rule::UnitPair, vec![r, m]);
            self.ban(s, d, b)?;
        }
        if let Some(id) = self.p.pair_id(d, e) {
            self.delete(s, id, r)?;
        }
        Ok(())
    }
    fn activate(&mut self, s: &mut State, a: usize, b: usize) {
        let sum = a + b;
        if s.active[sum].is_none() {
            let r = s.node(
                Fact::Active(sum),
                Rule::Sum,
                vec![s.member[a].unwrap(), s.member[b].unwrap()],
            );
            s.active[sum] = Some(r);
            s.active_bits[sum / 64] |= 1u64 << (sum % 64);
            s.trail.push(Undo::Active(sum));
            s.dirty(sum);
            s.revision += 1;
        }
    }
    fn domain(&mut self, s: &mut State, sum: usize) -> Result<(), Id> {
        self.metrics.dirty_domain_events += 1;
        if s.live_count[sum] == 0 && s.bad[sum].is_none() {
            let reasons = self.p.domain(sum).map(|id| s.dead[id].unwrap()).collect();
            let r = s.node(Fact::Bad(sum), Rule::Empty, reasons);
            self.bad(s, sum, r)?;
        }
        if let Some(active) = s.active[sum] {
            if let Some(bad) = s.bad[sum] {
                return Err(s.conflict(vec![active, bad]));
            }
            if s.product_count[sum] > 0 {
                s.unresolved(sum, false);
                return Ok(());
            }
            if s.live_count[sum] == 1 {
                let id = s.xor_live_id[sum];
                let w = self.p.witness[id];
                assert_eq!(w.s, sum);
                assert!(s.dead[id].is_none());
                let mut reasons = vec![active];
                reasons.extend(self.p.domain(sum).filter_map(|id| s.dead[id]));
                for v in [w.d, w.e] {
                    let r = s.node(Fact::Member(v), Rule::Unique(w.d, w.e), reasons.clone());
                    self.force(s, v, r)?;
                }
                s.unresolved(sum, false);
                return Ok(());
            }
            s.unresolved(sum, true);
        } else {
            s.unresolved(sum, false);
        }
        Ok(())
    }
    fn bad_sum_ban(&mut self, s: &mut State, sum: usize, a: usize) -> Result<(), Id> {
        if sum > a && sum - a <= self.p.n {
            let v = sum - a;
            if s.banned[v].is_none() {
                let r = s.node(
                    Fact::Ban(v),
                    Rule::BadSum,
                    vec![s.bad[sum].unwrap(), s.member[a].unwrap()],
                );
                self.ban(s, v, r)?;
            }
        }
        Ok(())
    }
    fn even_bound(&mut self, s: &mut State) -> Result<(), Id> {
        if !self.cfg.even_bound {
            return Ok(());
        }
        let allowed: Vec<_> = (2..=384.min(self.p.n))
            .step_by(2)
            .filter(|&e| s.banned[e].is_none())
            .collect();
        if allowed.len() > 7 {
            return Ok(());
        }
        let premises = (2..=384.min(self.p.n))
            .step_by(2)
            .filter_map(|e| s.banned[e])
            .collect::<Vec<_>>();
        self.metrics.even_bound_fires += 1;
        if allowed.len() < 7 {
            return Err(s.node(Fact::False, Rule::EvenBound, premises));
        }
        for e in allowed {
            if s.member[e].is_none() {
                let r = s.node(Fact::Member(e), Rule::EvenBound, premises.clone());
                self.force(s, e, r)?;
            }
        }
        Ok(())
    }
    fn event(&mut self, s: &mut State, event: Event) -> Result<(), Id> {
        match event {
            Event::Additive => self.additive_propagate(s)?,
            Event::Domain(sum) => {
                s.pending[sum] = 0;
                self.domain(s, sum)?;
            }
            Event::Member(a) => {
                for i in 0..s.adjacency[a].len() {
                    let v = s.adjacency[a][i];
                    let r = s.pairs[&(a.min(v), a.max(v))];
                    let b = s.node(Fact::Ban(v), Rule::UnitPair, vec![r, s.member[a].unwrap()]);
                    self.ban(s, v, b)?;
                }
                if s.member[self.p.n].is_some() {
                    self.activate(s, self.p.n, a);
                }
                for i in 0..s.bad_values.len() {
                    let sum = s.bad_values[i];
                    self.bad_sum_ban(s, sum, a)?;
                }
            }
            Event::Ban(v) => {
                for i in 0..self.p.endpoint.get(v).len() {
                    let id = self.p.endpoint.get(v)[i];
                    self.metrics.endpoint_occ_visits += 1;
                    self.delete(s, id, s.banned[v].unwrap())?;
                }
                if self.cfg.prime_clauses
                    && let Some(group) = GROUPS.iter().position(|g| g.contains(&v))
                {
                    for (i, g) in GROUPS.iter().enumerate() {
                        if i == group {
                            continue;
                        }
                        for &q in *g {
                            self.metrics.prime_clause_fires += 1;
                            if q > self.p.n {
                                return Err(s.node(
                                    Fact::False,
                                    Rule::PrimeClause(v, q),
                                    vec![s.banned[v].unwrap()],
                                ));
                            }
                            let r = s.node(
                                Fact::Member(q),
                                Rule::PrimeClause(v, q),
                                vec![s.banned[v].unwrap()],
                            );
                            self.force(s, q, r)?;
                        }
                    }
                }
                if v <= 384 && v.is_multiple_of(2) {
                    self.even_bound(s)?;
                }
            }
            Event::Bad(sum) => {
                let r = s.bad[sum].unwrap();
                for id in self.p.domain(sum) {
                    self.delete(s, id, r)?;
                }
                for i in 0..self.p.endpoint_sum.get(sum).len() {
                    let id = self.p.endpoint_sum.get(sum)[i];
                    self.metrics.endpoint_sum_occ_visits += 1;
                    self.delete(s, id, r)?;
                }
                if sum.is_multiple_of(2) {
                    let v = sum / 2;
                    let b = s.node(Fact::Ban(v), Rule::BadSum, vec![r]);
                    self.ban(s, v, b)?;
                }
                for i in 0..s.members.len() {
                    let a = s.members[i];
                    self.bad_sum_ban(s, sum, a)?;
                }
            }
        }
        Ok(())
    }
    pub fn quiesce(&mut self, s: &mut State) -> Propagation {
        let mut budget = usize::MAX;
        self.propagate(s, &mut budget)
    }
    pub fn propagate(&mut self, s: &mut State, budget: &mut usize) -> Propagation {
        self.propagate_to(s, budget, usize::MAX)
    }
    fn propagate_to(
        &mut self,
        s: &mut State,
        budget: &mut usize,
        member_limit: usize,
    ) -> Propagation {
        loop {
            if self.limits.stopped() {
                return Propagation::Paused;
            }
            if s.quiet() {
                return Propagation::Quiet;
            }
            if *budget == 0 || s.members.len() >= member_limit {
                return Propagation::Paused;
            }
            *budget -= 1;
            let result = if let Some(event) = s.high.pop_front() {
                self.event(s, event)
            } else {
                let mut job = s.low.pop_front().unwrap();
                let word = s.member_words[job.next];
                job.next += 1;
                self.metrics.low_pair_word_scans += 1;
                let sum_word = job.a / 64 + word;
                let shift = job.a % 64;
                let mut already = s.active_bits[sum_word] >> shift;
                if shift != 0 {
                    already |= s.active_bits[sum_word + 1] << (64 - shift);
                }
                let mut missing = s.member_bits[word] & !already;
                while missing != 0 {
                    let bit = missing.trailing_zeros() as usize;
                    missing &= missing - 1;
                    self.activate(s, job.a, word * 64 + bit);
                    self.metrics.low_pair_activations += 1;
                }
                if job.next < job.end {
                    s.low.push_front(job);
                }
                Ok(())
            };
            if let Err(id) = result {
                self.metrics.conflicts += 1;
                return Propagation::Conflict(id);
            }
        }
    }
    pub fn initialize(&mut self, s: &mut State) -> Result<(), Id> {
        for v in [1, 2, self.p.n] {
            let r = s.node(Fact::Member(v), Rule::Initial, vec![]);
            self.force(s, v, r)?;
        }
        if self.cfg.universal_members {
            for &v in U {
                if v > self.p.n {
                    return Err(s.node(Fact::False, Rule::Universal(v), vec![]));
                }
                let r = s.node(Fact::Member(v), Rule::Universal(v), vec![]);
                self.force(s, v, r)?;
            }
        }
        for sum in 2..=self.p.limit {
            if s.live_count[sum] == 0 {
                s.dirty(sum);
            }
        }
        if self.cfg.prime_clauses {
            for (i, g) in GROUPS.iter().enumerate() {
                for &p in *g {
                    if p <= self.p.n {
                        continue;
                    }
                    for (j, h) in GROUPS.iter().enumerate() {
                        if i == j {
                            continue;
                        }
                        for &q in *h {
                            if q > self.p.n {
                                return Err(s.node(Fact::False, Rule::PrimeClause(p, q), vec![]));
                            }
                            let r = s.node(Fact::Member(q), Rule::PrimeClause(p, q), vec![]);
                            self.force(s, q, r)?;
                        }
                    }
                }
            }
        }
        self.even_bound(s)
    }
    pub fn select(&self, s: &State) -> Option<usize> {
        s.unresolved.iter().copied().min_by_key(|&sum| {
            let outside = usize::from(!(sum > self.p.n && s.member[sum - self.p.n].is_some()));
            match self.policy {
                Policy::Mrv => (s.live_count[sum], outside, usize::MAX - sum),
                Policy::StrictMaximumRowFirst => (outside, s.live_count[sum], usize::MAX - sum),
            }
        })
    }
    pub(super) fn apply(&mut self, s: &mut State, fact: Fact, r: Id) -> Result<(), Id> {
        match fact {
            Fact::Member(v) => self.force(s, v, r),
            Fact::Ban(v) => self.ban(s, v, r),
            Fact::Bad(v) => self.bad(s, v, r),
            Fact::Pair(d, e) => self.pair(s, d, e, r),
            _ => panic!("not an installable fact"),
        }
    }
    pub(super) fn probe(&mut self, s: &mut State, assumptions: Vec<Fact>, budget: usize) -> Probe {
        let cp = s.checkpoint();
        let scope = s.proof.enter(s.scope, assumptions.clone());
        s.scope = scope;
        self.metrics.probe_calls += 1;
        let mut conflict = None;
        for f in assumptions {
            let r = s.node(f.clone(), Rule::Assumption, vec![]);
            if let Err(id) = self.apply(s, f, r) {
                conflict = Some(id);
                break;
            }
        }
        let mut left = budget.min(self.probe_remaining);
        let before = left;
        if conflict.is_none() {
            let limit = if self.cfg.probe_members == 0 {
                usize::MAX
            } else {
                s.members.len().saturating_add(self.cfg.probe_members)
            };
            match self.propagate_to(s, &mut left, limit) {
                Propagation::Conflict(id) => conflict = Some(id),
                Propagation::Paused => self.metrics.probe_pauses += 1,
                Propagation::Quiet => {}
            }
        }
        let used = before - left;
        self.probe_remaining -= used;
        self.metrics.probe_events += used as u64;
        let mut delta = BTreeMap::new();
        if conflict.is_none() {
            for undo in &s.trail[cp.trail..] {
                let fact = match *undo {
                    Undo::Member(v) => Some((Fact::Member(v), s.member[v].unwrap())),
                    Undo::Ban(v) => Some((Fact::Ban(v), s.banned[v].unwrap())),
                    Undo::Bad(v) => Some((Fact::Bad(v), s.bad[v].unwrap())),
                    Undo::Pair(d, e) => Some((Fact::Pair(d, e), s.pairs[&(d, e)])),
                    _ => None,
                };
                if let Some((f, r)) = fact {
                    delta.insert(f, r);
                }
            }
        } else {
            self.metrics.probe_refutations += 1;
        }
        s.rollback(&self.p, cp);
        Probe {
            scope,
            conflict,
            delta,
        }
    }
    pub(super) fn cover_context(&mut self, s: &State, sum: usize) -> (Vec<Vec<Fact>>, Vec<Id>) {
        self.metrics.full_domain_enumerations += 1;
        let mut context = vec![s.active[sum].unwrap()];
        let mut cases = vec![];
        for id in self.p.domain(sum) {
            if let Some(r) = s.dead[id] {
                context.push(r);
            } else {
                let w = self.p.witness[id];
                cases.push(vec![Fact::Member(w.d), Fact::Member(w.e)]);
            }
        }
        (cases, context)
    }
    pub(super) fn cover(
        &mut self,
        s: &mut State,
        kind: Cover,
        cases: Vec<Vec<Fact>>,
        context: Vec<Id>,
        total: usize,
    ) -> Result<(), Id> {
        let start = self.probe_remaining;
        let mut records = vec![];
        for assumptions in &cases {
            if start - self.probe_remaining >= total
                || self.probe_remaining == 0
                || self.limits.stopped()
            {
                self.metrics.probe_unstarted_cases += 1;
                let scope = s.proof.enter(s.scope, assumptions.clone());
                records.push(Probe {
                    scope,
                    conflict: None,
                    delta: BTreeMap::new(),
                });
            } else {
                let budget = self
                    .cfg
                    .probe_case_events
                    .min(total - (start - self.probe_remaining));
                records.push(self.probe(s, assumptions.clone(), budget));
            }
        }
        let surviving_indices = records
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.conflict.is_none().then_some(i))
            .collect::<Vec<_>>();
        if let [i] = surviving_indices.as_slice() {
            // Even an unstarted sole survivor entails its starting assumptions.
            // Other cases have complete refutations; this is not a timeout inference.
            for fact in &cases[*i] {
                let record = &mut records[*i];
                let id = s
                    .proof
                    .add(record.scope, fact.clone(), Rule::Assumption, vec![]);
                record.delta.entry(fact.clone()).or_insert(id);
            }
        }
        let survivors: Vec<_> = records.iter().filter(|r| r.conflict.is_none()).collect();
        let scopes = records.iter().map(|r| r.scope).collect::<Vec<_>>();
        if survivors.is_empty() {
            let mut premises = context.clone();
            premises.extend(records.iter().map(|r| r.conflict.unwrap()));
            return Err(s.node(
                Fact::False,
                Rule::Join {
                    cover: kind,
                    scopes,
                    context: context.len(),
                },
                premises,
            ));
        }
        let common = survivors[0]
            .delta
            .keys()
            .filter(|f| survivors.iter().all(|r| r.delta.contains_key(*f)))
            .cloned()
            .collect::<Vec<_>>();
        if matches!(kind, Cover::Factor(_)) {
            for (case, r) in cases.iter().zip(&records) {
                if let Some(conflict) = r.conflict {
                    let (Fact::Member(d), Fact::Member(e)) = (&case[0], &case[1]) else {
                        unreachable!()
                    };
                    let proof =
                        s.node(Fact::Pair(*d, *e), Rule::Discharge(r.scope), vec![conflict]);
                    self.pair(s, *d, *e, proof)?;
                }
            }
        }
        for fact in common {
            let mut premises = context.clone();
            premises.extend(
                records
                    .iter()
                    .map(|r| r.conflict.unwrap_or_else(|| r.delta[&fact])),
            );
            let r = s.node(
                fact.clone(),
                Rule::Join {
                    cover: kind.clone(),
                    scopes: scopes.clone(),
                    context: context.len(),
                },
                premises,
            );
            match fact {
                Fact::Member(_) => self.metrics.probe_common_members += 1,
                Fact::Ban(_) => self.metrics.probe_common_bans += 1,
                Fact::Bad(_) => self.metrics.probe_common_e += 1,
                Fact::Pair(..) => self.metrics.probe_common_g += 1,
                _ => {}
            }
            self.apply(s, fact, r)?;
        }
        Ok(())
    }
    fn strengthen(&mut self, s: &mut State) -> Propagation {
        let started = Instant::now();
        let initial = self.probe_remaining;
        let result = self.strengthen_inner(s, initial);
        self.metrics.probe_wall_time += started.elapsed().as_secs_f64();
        result
    }
    pub fn strengthen_root(&mut self, s: &mut State) -> Propagation {
        let start = Instant::now();
        let result = self.strengthen_root_inner(s);
        self.metrics.probe_wall_time += start.elapsed().as_secs_f64();
        result
    }
    fn strengthen_root_inner(&mut self, s: &mut State) -> Propagation {
        let mut last_chains_members = s.members.len();
        loop {
            if self.limits.stopped() {
                return Propagation::Paused;
            }
            if self.probe_remaining == 0 {
                return self.quiesce(s);
            }
            self.metrics.root_strengthen_rounds += 1;
            let revision = s.revision;
            let mut sums = s
                .unresolved
                .iter()
                .copied()
                .filter(|&sum| (2..=self.cfg.root_probe_width).contains(&s.live_count[sum]))
                .collect::<Vec<_>>();
            sums.sort_unstable();
            for sum in sums {
                if self.probe_remaining == 0 {
                    break;
                }
                if s.product_count[sum] > 0
                    || !(2..=self.cfg.root_probe_width).contains(&s.live_count[sum])
                {
                    continue;
                }
                let before = s.revision;
                let (cases, context) = self.cover_context(s, sum);
                let total = self
                    .cfg
                    .probe_case_events
                    .saturating_mul(cases.len())
                    .min(self.probe_remaining);
                if let Err(id) = self.cover(s, Cover::Factor(sum), cases, context, total) {
                    return Propagation::Conflict(id);
                }
                let r = self.quiesce(s);
                if r != Propagation::Quiet {
                    return r;
                }
                if s.revision != before {
                    self.metrics.root_cover_sums.push(sum);
                }
                if s.members.len() > last_chains_members {
                    let r = self.prime_chains(s);
                    if r != Propagation::Quiet {
                        return r;
                    }
                    last_chains_members = s.members.len();
                    break;
                }
            }
            // Revisit absence only after the root has acquired the conclusions
            // and prime-chain exclusions needed to make it informative.
            for v in std::iter::once(4).chain(GROUPS.iter().flat_map(|g| g.iter().copied())) {
                if v > self.p.n
                    || s.member[v].is_some()
                    || s.banned[v].is_some()
                    || self.probe_remaining == 0
                {
                    continue;
                }
                let r = self.probe(s, vec![Fact::Ban(v)], self.cfg.probe_case_events);
                if let Some(id) = r.conflict {
                    let proof = s.node(Fact::Member(v), Rule::Discharge(r.scope), vec![id]);
                    if let Err(id) = self.force(s, v, proof) {
                        return Propagation::Conflict(id);
                    }
                    self.metrics.root_forced_absences.push(v);
                }
                let r = self.quiesce(s);
                if r != Propagation::Quiet {
                    return r;
                }
            }
            if s.members.len() > last_chains_members {
                let r = self.prime_chains(s);
                if r != Propagation::Quiet {
                    return r;
                }
                last_chains_members = s.members.len();
            }
            if s.revision == revision {
                return Propagation::Quiet;
            }
        }
    }
    fn strengthen_inner(&mut self, s: &mut State, initial: usize) -> Propagation {
        let budget = self.cfg.probe_node_events.min(initial);
        if self.cfg.seed_probe && s.scope == 0 {
            if let Err(id) = self.cover(s, Cover::Seeds, seed_cases(), vec![], budget) {
                return Propagation::Conflict(id);
            }
            let r = self.quiesce(s);
            if r != Propagation::Quiet {
                return r;
            }
        }
        let mut sums = s
            .unresolved
            .iter()
            .copied()
            .filter(|&sum| {
                sum > self.p.n
                    && s.member[sum - self.p.n].is_some()
                    && (2..=4).contains(&s.live_count[sum])
            })
            .collect::<Vec<_>>();
        sums.sort_unstable_by(|a, b| b.cmp(a));
        sums.truncate(self.cfg.probe_domains);
        for sum in sums {
            if initial - self.probe_remaining >= budget {
                break;
            }
            if s.product_count[sum] > 0 || !(2..=4).contains(&s.live_count[sum]) {
                continue;
            }
            let (cases, context) = self.cover_context(s, sum);
            if let Err(id) = self.cover(
                s,
                Cover::Factor(sum),
                cases,
                context,
                budget - (initial - self.probe_remaining),
            ) {
                return Propagation::Conflict(id);
            }
            let r = self.quiesce(s);
            if r != Propagation::Quiet {
                return r;
            }
        }
        let targets = std::iter::once(4)
            .chain(GROUPS.iter().flat_map(|g| g.iter().copied()))
            .filter(|&v| v <= self.p.n)
            .collect::<Vec<_>>();
        for v in targets
            .into_iter()
            .filter(|&v| s.member[v].is_none() && s.banned[v].is_none())
            .take(self.cfg.absence_targets)
            .collect::<Vec<_>>()
        {
            if initial - self.probe_remaining >= budget {
                break;
            }
            let r = self.probe(
                s,
                vec![Fact::Ban(v)],
                self.cfg
                    .probe_case_events
                    .min(budget - (initial - self.probe_remaining)),
            );
            if let Some(id) = r.conflict {
                let proof = s.node(Fact::Member(v), Rule::Discharge(r.scope), vec![id]);
                if let Err(id) = self.force(s, v, proof) {
                    return Propagation::Conflict(id);
                }
            }
            let r = self.quiesce(s);
            if r != Propagation::Quiet {
                return r;
            }
        }
        Propagation::Quiet
    }
    pub fn prime_chains(&mut self, s: &mut State) -> Propagation {
        let started = Instant::now();
        let mut left = self.cfg.prime_chain_steps;
        // Each candidate starts a fresh reachability search. Reuse the large
        // visitation array with generation stamps instead of clearing it for
        // every candidate even value.
        let mut known = vec![0u32; self.p.limit + 1];
        let mut generation = 0u32;
        let mut queue = VecDeque::new();
        for e in (2..=self.p.n).step_by(2) {
            if left == 0 || self.limits.stopped() {
                break;
            }
            if s.banned[e].is_some() || s.member[e].is_some() {
                continue;
            }
            if generation == u32::MAX {
                known.fill(0);
                generation = 0;
            }
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
            reasons.push(s.member[2].unwrap());
            let mut steps = vec![];
            let mut terminal = false;
            'chain: while let Some(a) = queue.pop_front() {
                for h in [2, e] {
                    if left == 0 || self.limits.stopped() {
                        break 'chain;
                    }
                    left -= 1;
                    self.metrics.prime_chain_steps += 1;
                    let p = a + h;
                    if !self.p.prime[p] {
                        continue;
                    }
                    if known[p] == generation {
                        continue;
                    }
                    steps.push((a, h, p));
                    known[p] = generation;
                    if p > self.p.n {
                        terminal = true;
                        break 'chain;
                    }
                    if let Some(b) = s.banned[p] {
                        reasons.push(b);
                        terminal = true;
                        break 'chain;
                    }
                    queue.push_back(p);
                }
            }
            if terminal {
                let r = s.node(
                    Fact::Ban(e),
                    Rule::PrimeChain {
                        candidate: e,
                        steps,
                    },
                    reasons,
                );
                if let Err(id) = self.ban(s, e, r) {
                    self.metrics.prime_chain_wall_time += started.elapsed().as_secs_f64();
                    return Propagation::Conflict(id);
                }
                let r = self.quiesce(s);
                if r != Propagation::Quiet {
                    self.metrics.prime_chain_wall_time += started.elapsed().as_secs_f64();
                    return r;
                }
            }
        }
        let result = self.quiesce(s);
        self.metrics.prime_chain_wall_time += started.elapsed().as_secs_f64();
        result
    }
    pub fn dfs(&mut self, s: &mut State) -> Outcome {
        self.metrics.nodes += 1;
        match self.quiesce(s) {
            Propagation::Conflict(id) => return Outcome::No(id),
            Propagation::Paused => return Outcome::Unknown,
            _ => {}
        }
        if self.cfg.node_limit > 0 && self.metrics.nodes > self.cfg.node_limit {
            return Outcome::Unknown;
        }
        if self.cfg.probes && self.probe_remaining > 0 {
            match self.strengthen(s) {
                Propagation::Conflict(id) => return Outcome::No(id),
                Propagation::Paused => return Outcome::Unknown,
                _ => {}
            }
        }
        if self.limits.stopped() {
            return Outcome::Unknown;
        }
        let product_sum = self.select(s);
        let additive = s.additive.as_ref().is_some_and(|a| {
            a.satisfied.is_none()
                && a.live > 1
                && product_sum.is_none_or(|sum| a.live < s.live_count[sum])
        });
        if product_sum.is_none() && !additive {
            self.metrics.yes_checks += 1;
            let mut values = s.members.clone();
            values.sort_unstable();
            return if validate(self.p.n, &values) {
                Outcome::Yes(values)
            } else {
                Outcome::Error("raw YES validation failed".into())
            };
        }
        self.metrics.full_domain_enumerations += 1;
        let mut options = if additive {
            self.metrics.additive_branches += 1;
            let a = s.additive.as_ref().unwrap();
            (1..=self.p.n / 2)
                .filter(|&v| a.dead[v].is_none())
                .map(|v| (v, self.p.n - v))
                .collect::<Vec<_>>()
        } else {
            self.p
                .domain(product_sum.unwrap())
                .filter(|&id| s.dead[id].is_none())
                .map(|id| {
                    let w = self.p.witness[id];
                    (w.d, w.e)
                })
                .collect::<Vec<_>>()
        };
        if self.lane % 4 >= 2 {
            options.reverse();
        }
        let len = options.len();
        if len > 0 {
            options.rotate_left(self.lane % len);
        }
        for (d, e) in options {
            match self.quiesce(s) {
                Propagation::Conflict(id) => return Outcome::No(id),
                Propagation::Paused => return Outcome::Unknown,
                _ => {}
            }
            if additive && s.additive.as_ref().unwrap().dead[d].is_some() {
                continue;
            }
            if !additive && self.p.pair_id(d, e).is_some_and(|id| s.dead[id].is_some()) {
                continue;
            }
            let cp = s.checkpoint();
            let child = s
                .proof
                .enter(s.scope, vec![Fact::Member(d), Fact::Member(e)]);
            s.scope = child;
            let mut initial = Ok(());
            for v in [d, e] {
                let r = s.node(Fact::Member(v), Rule::Assumption, vec![]);
                initial = self.force(s, v, r);
                if initial.is_err() {
                    break;
                }
            }
            let result = match initial {
                Err(id) => Outcome::No(id),
                Ok(()) => self.dfs(s),
            };
            if let Outcome::No(id) = &result
                && s.proof.get(*id).scope != child
            {
                s.rollback(&self.p, cp);
                return Outcome::Error("child refutation has wrong scope".into());
            }
            s.rollback(&self.p, cp);
            match result {
                Outcome::No(id) => {
                    let r = s.node(Fact::Pair(d, e), Rule::Discharge(child), vec![id]);
                    if let Err(id) = self.pair(s, d, e, r) {
                        return Outcome::No(id);
                    }
                }
                other => return other,
            }
        }
        // Every original alternative now has a recorded exclusion, including
        // siblings removed by already discharged refutations. The empty-domain
        // proof includes every pair from the immutable complete factor table.
        match self.quiesce(s) {
            Propagation::Conflict(id) => Outcome::No(id),
            Propagation::Paused => Outcome::Unknown,
            Propagation::Quiet => {
                Outcome::Error("exhausted factor cover without contradiction".into())
            }
        }
    }
}
