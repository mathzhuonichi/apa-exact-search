use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

type Pair = (usize, usize);

#[derive(Debug)]
pub struct Problem {
    pub maximum: usize,
    prime: Vec<bool>,
    spf: Vec<usize>,
    prime_bits: Vec<u64>,
    factor_cache: Vec<OnceLock<Arc<[Pair]>>>,
}

impl Problem {
    pub fn new(maximum: usize) -> Self {
        let limit = 2 * maximum;
        let mut spf = vec![0; limit + 1];
        for p in 2..=limit {
            if spf[p] != 0 {
                continue;
            }
            spf[p] = p;
            if p <= limit / p {
                let mut q = p * p;
                while q <= limit {
                    if spf[q] == 0 {
                        spf[q] = p;
                    }
                    q += p;
                }
            }
        }
        let mut prime = vec![false; limit + 1];
        let mut prime_bits = vec![0; (limit + 64) / 64];
        for v in 2..=limit {
            prime[v] = spf[v] == v;
            if prime[v] {
                prime_bits[v >> 6] |= 1_u64 << (v & 63);
            }
        }
        Self {
            maximum,
            prime,
            spf,
            prime_bits,
            factor_cache: std::iter::repeat_with(OnceLock::new)
                .take(limit + 1)
                .collect(),
        }
    }

    fn factor_pairs(&self, sum: usize) -> Arc<[Pair]> {
        Arc::clone(self.factor_cache[sum].get_or_init(|| self.compute_factor_pairs(sum).into()))
    }

    fn compute_factor_pairs(&self, sum: usize) -> Vec<Pair> {
        let mut rest = sum;
        let mut divisors = vec![1];
        while rest > 1 {
            let p = self.spf[rest];
            let mut power = 1;
            let old = divisors.len();
            loop {
                rest /= p;
                power *= p;
                for i in 0..old {
                    divisors.push(divisors[i] * power);
                }
                if !rest.is_multiple_of(p) {
                    break;
                }
            }
        }
        let mut result = Vec::new();
        for d in divisors {
            if d <= sum / d && sum / d <= self.maximum {
                result.push((d, sum / d));
            }
        }
        result.sort_unstable();
        result
    }
}

#[derive(Clone)]
pub struct State {
    member: Vec<u8>,
    member_bits: Vec<u64>,
    banned_bits: Vec<u64>,
    active_bits: Vec<u64>,
    done_bits: Vec<u64>,
    watch1: Vec<usize>,
    watch2: Vec<usize>,
    members: Vec<usize>,
    demands: Vec<usize>,
    reason_a: Vec<usize>,
    reason_b: Vec<usize>,
    product_count: Vec<u32>,
    maximum_deletion: bool,
    additive_live: Vec<u64>,
    additive_count: usize,
    additive_satisfied: usize,
    banned_undo: Vec<(usize, u64)>,
    additive_undo: Vec<(usize, u64, usize)>,
    product_undo: Vec<usize>,
    active_undo: Vec<usize>,
    done_undo: Vec<usize>,
    watch_undo: Vec<(usize, usize, usize)>,
    nogoods: Vec<Pair>,
    processed: usize,
    root_conflict: bool,
}

impl State {
    fn empty(maximum: usize) -> Self {
        Self {
            member: vec![0; maximum + 1],
            member_bits: vec![0; (maximum + 64) / 64],
            banned_bits: vec![0; (maximum + 64) / 64],
            active_bits: vec![0; (2 * maximum + 64) / 64],
            done_bits: vec![0; (2 * maximum + 64) / 64],
            watch1: vec![0; 2 * maximum + 1],
            watch2: vec![0; 2 * maximum + 1],
            members: Vec::new(),
            demands: Vec::new(),
            reason_a: vec![0; 2 * maximum + 1],
            reason_b: vec![0; 2 * maximum + 1],
            product_count: vec![0; 2 * maximum + 1],
            maximum_deletion: false,
            additive_live: vec![0; (maximum / 2).div_ceil(64)],
            additive_count: 0,
            additive_satisfied: 0,
            banned_undo: Vec::new(),
            additive_undo: Vec::new(),
            product_undo: Vec::new(),
            active_undo: Vec::new(),
            done_undo: Vec::new(),
            watch_undo: Vec::new(),
            nogoods: Vec::new(),
            processed: 0,
            root_conflict: false,
        }
    }

    pub fn seeded(maximum: usize) -> Result<Self, String> {
        const SEEDS: [usize; 21] = [
            1, 2, 3, 5, 7, 11, 13, 17, 19, 23, 31, 43, 47, 53, 61, 71, 73, 83, 103, 109, 113,
        ];
        // The membership lemma for 1 requires a member greater than 2.
        // At maximum 2, the valid singleton {2} must remain available.
        let mut state = Self::empty(maximum);
        for value in SEEDS
            .into_iter()
            .filter(|v| *v <= maximum && (maximum > 2 || *v == 2))
            .chain(std::iter::once(maximum))
        {
            state.insert_root_member(value)?;
        }
        state.clear_undo();
        state.rebuild_products(maximum);
        state.validate(maximum)?;
        Ok(state)
    }

    pub fn prepared(problem: &Problem) -> Result<Self, String> {
        let maximum = problem.maximum;
        let seed = Self::seeded(maximum)?;
        let mut allowed = vec![true; maximum + 1];
        let mut mandatory = vec![false; maximum + 1];
        let mut forced = seed.members;
        for &value in &forced {
            mandatory[value] = true;
        }
        let mut new_roots: Vec<_> = forced.iter().copied().filter(|v| v % 2 == 1).collect();
        let mut seen = vec![0_u32; maximum + 1];
        let mut stamp = 0_u32;
        let mut conflict = false;
        let mut factors: HashMap<usize, Arc<[Pair]>> = HashMap::new();

        while !conflict {
            new_roots.sort_unstable_by(|a, b| b.cmp(a));
            if !new_roots.is_empty() {
                for e in (2..=maximum).step_by(2) {
                    if !allowed[e] {
                        continue;
                    }
                    stamp = stamp
                        .checked_add(1)
                        .ok_or("root propagation stamp overflow")?;
                    let mut queue = new_roots.clone();
                    for &root in &new_roots {
                        seen[root] = stamp;
                    }
                    let mut index = 0;
                    let mut bad = false;
                    while index < queue.len() && !bad {
                        let a = queue[index];
                        index += 1;
                        for shift in [2, e] {
                            let value = a + shift;
                            if !problem.prime[value] {
                                continue;
                            }
                            if value > maximum {
                                allowed[e] = false;
                                bad = true;
                                if mandatory[e] {
                                    conflict = true;
                                }
                                break;
                            }
                            if seen[value] != stamp {
                                seen[value] = stamp;
                                queue.push(value);
                            }
                        }
                    }
                    if conflict {
                        break;
                    }
                }
            }
            new_roots.clear();
            if conflict {
                break;
            }

            let mut changed = true;
            let mut yield_to_unary = false;
            while changed && !conflict && !yield_to_unary {
                changed = false;
                forced.sort_unstable_by(|a, b| b.cmp(a));
                let old_size = forced.len();
                for i in 0..old_size {
                    for j in i..old_size {
                        let sum = forced[i] + forced[j];
                        let domain = factors
                            .entry(sum)
                            .or_insert_with(|| problem.factor_pairs(sum))
                            .clone();
                        let live: Vec<_> = domain
                            .iter()
                            .copied()
                            .filter(|&(d, e)| allowed[d] && allowed[e])
                            .collect();
                        if live.is_empty() {
                            conflict = true;
                            break;
                        }
                        for value in [live[0].0, live[0].1] {
                            if mandatory[value] {
                                continue;
                            }
                            let common = live.iter().all(|&(d, e)| d == value || e == value);
                            if common {
                                mandatory[value] = true;
                                forced.push(value);
                                changed = true;
                                if value % 2 == 1 {
                                    new_roots.push(value);
                                }
                                if new_roots.len() >= 64 {
                                    yield_to_unary = true;
                                    break;
                                }
                            }
                        }
                        if yield_to_unary {
                            break;
                        }
                    }
                    if conflict || yield_to_unary {
                        break;
                    }
                }
            }
            if new_roots.is_empty() {
                break;
            }
        }

        let mut state = Self::empty(maximum);
        forced.sort_unstable();
        forced.dedup();
        for value in forced {
            state.insert_root_member(value)?;
        }
        for (value, is_allowed) in allowed.into_iter().enumerate().skip(1) {
            if !is_allowed && state.member[value] == 0 {
                state.insert_root_ban(value)?;
            }
        }
        state.root_conflict = conflict;
        state.clear_undo();
        state.rebuild_products(maximum);
        state.validate(maximum)?;
        Ok(state)
    }

    pub fn from_file(maximum: usize, path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut state = Self::empty(maximum);
        for (index, line) in text.lines().enumerate() {
            let mut fields = line.split_whitespace();
            let tag = fields
                .next()
                .ok_or_else(|| format!("empty state line {}", index + 1))?;
            let value = fields
                .next()
                .ok_or_else(|| format!("missing value on state line {}", index + 1))?
                .parse::<usize>()
                .map_err(|_| format!("bad value on state line {}", index + 1))?;
            if fields.next().is_some() || value == 0 || value > maximum {
                return Err(format!("bad state line {}", index + 1));
            }
            match tag {
                "F" => state.insert_root_member(value)?,
                "B" => state.insert_root_ban(value)?,
                _ => return Err(format!("bad state tag on line {}", index + 1)),
            }
        }
        state.clear_undo();
        state.rebuild_products(maximum);
        state.validate(maximum)?;
        Ok(state)
    }

    fn insert_root_member(&mut self, value: usize) -> Result<(), String> {
        if bit(&self.banned_bits, value) {
            return Err(format!("root both forces and bans {value}"));
        }
        if self.member[value] == 0 {
            self.member[value] = 1;
            set_bit(&mut self.member_bits, value);
            self.members.push(value);
        }
        Ok(())
    }

    fn insert_root_ban(&mut self, value: usize) -> Result<(), String> {
        if self.member[value] != 0 {
            return Err(format!("root both forces and bans {value}"));
        }
        set_bit(&mut self.banned_bits, value);
        Ok(())
    }

    fn clear_undo(&mut self) {
        self.banned_undo.clear();
        self.additive_undo.clear();
        self.product_undo.clear();
        self.active_undo.clear();
        self.done_undo.clear();
        self.watch_undo.clear();
    }

    /// Enables the consequence of deleting the maximum from a hypothetical
    /// counterexample. Soundness requires that every maximum in
    /// (complete_prefix_base, maximum) has already been excluded.
    pub fn enable_maximum_deletion(
        &mut self,
        problem: &Problem,
        complete_prefix_base: usize,
    ) -> Result<(), String> {
        if problem.maximum <= 2 {
            return Err("maximum-deletion requires maximum > 2".into());
        }
        let threshold = complete_prefix_base
            .checked_mul(complete_prefix_base.saturating_sub(1))
            .ok_or("complete-prefix threshold overflow")?;
        if problem.maximum <= threshold {
            return Err(format!(
                "maximum-deletion requires maximum > {complete_prefix_base}*{} = {threshold}",
                complete_prefix_base.saturating_sub(1)
            ));
        }

        self.maximum_deletion = true;
        let pair_count = problem.maximum / 2;
        self.additive_live.fill(u64::MAX);
        if pair_count & 63 != 0 {
            let last = self.additive_live.len() - 1;
            self.additive_live[last] = (1_u64 << (pair_count & 63)) - 1;
        }
        self.additive_count = pair_count;

        // n must not be a product of two members of B=A\{n}.
        for &(d, e) in problem.factor_pairs(problem.maximum).iter() {
            if e == problem.maximum {
                continue;
            }
            if d == e {
                if self.member[d] != 0 {
                    self.root_conflict = true;
                } else {
                    self.insert_root_ban(d)?;
                }
            } else {
                self.nogoods.push((d, e));
            }
        }

        for value in 1..problem.maximum {
            if bit(&self.banned_bits, value) {
                self.clear_additive_for_value(problem.maximum, value, false);
            }
        }
        self.additive_satisfied = (1..=pair_count)
            .filter(|&a| self.member[a] != 0 && self.member[problem.maximum - a] != 0)
            .count();
        if self.additive_count == 0 && self.additive_satisfied == 0 {
            self.root_conflict = true;
        }
        self.clear_undo();
        Ok(())
    }

    fn clear_additive_for_value(&mut self, maximum: usize, value: usize, trail: bool) {
        if !self.maximum_deletion || value == 0 || value >= maximum {
            return;
        }
        let index = value.min(maximum - value) - 1;
        let word = index >> 6;
        let mask = 1_u64 << (index & 63);
        if self.additive_live[word] & mask == 0 {
            return;
        }
        if trail {
            self.additive_undo
                .push((word, self.additive_live[word], self.additive_count));
        }
        self.additive_live[word] &= !mask;
        self.additive_count -= 1;
    }

    fn rebuild_products(&mut self, maximum: usize) {
        self.product_count.fill(0);
        let limit = 2 * maximum;
        for i in 0..self.members.len() {
            for j in i..self.members.len() {
                let left = self.members[i];
                let right = self.members[j];
                if left <= limit / right {
                    self.product_count[left * right] += 1;
                }
            }
        }
    }

    pub fn validate(&self, maximum: usize) -> Result<(), String> {
        if self.member.len() != maximum + 1
            || maximum < 2
            || (maximum > 2 && self.member[1] == 0)
            || self.member[2] == 0
            || self.member[maximum] == 0
        {
            return Err("state must force 2 and the maximum, and also 1 when maximum > 2".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DemandOrder {
    Insertion,
    MaximumFirst,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    None,
    Adaptive,
    Full,
}

#[derive(Clone, Debug, Serialize)]
pub struct LaneConfig {
    pub lane: usize,
    pub lookahead: usize,
    pub order: DemandOrder,
    pub compatibility: Compatibility,
    pub reverse_witnesses: bool,
    pub learn_nogoods: bool,
    pub early_prime_filter: bool,
    pub rotation: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LaneOutcome {
    No,
    Yes,
    Unknown,
    Cancelled,
}

#[derive(Default, Debug, Serialize)]
pub struct Metrics {
    pub nodes: u64,
    pub branches: u64,
    pub conflicts: u64,
    pub factor_visits: u64,
    pub compatibility_checks: u64,
    pub support_rejections: u64,
    pub learned_pairs: u64,
    pub demand_scans: u64,
    pub watch_rescans: u64,
}

#[derive(Debug, Serialize)]
pub struct LaneReport {
    pub config: LaneConfig,
    pub outcome: LaneOutcome,
    pub seconds: f64,
    pub metrics: Metrics,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    members: usize,
    demands: usize,
    processed: usize,
    banned: usize,
    products: usize,
    active: usize,
    done: usize,
    watches: usize,
    nogoods: usize,
    additive: usize,
    additive_satisfied: usize,
}

struct Choice {
    sum: usize,
    endpoints: usize,
    degree: usize,
    options: Vec<Pair>,
    valid: Vec<bool>,
}

struct Selected {
    options: Vec<Pair>,
    valid: Vec<bool>,
}

pub struct Solver {
    problem: Arc<Problem>,
    config: LaneConfig,
    deadline: Option<Instant>,
    cancel: Arc<AtomicBool>,
    interrupted: Arc<AtomicBool>,
    cache: HashMap<usize, Arc<[Pair]>>,
    metrics: Metrics,
    example: Vec<usize>,
    started: Instant,
}

impl Solver {
    pub fn new(
        problem: Arc<Problem>,
        config: LaneConfig,
        seconds: f64,
        cancel: Arc<AtomicBool>,
        interrupted: Arc<AtomicBool>,
    ) -> Self {
        let started = Instant::now();
        let deadline = if seconds == 0.0 {
            None
        } else {
            Some(started + Duration::from_secs_f64(seconds))
        };
        Self {
            problem,
            config,
            deadline,
            cancel,
            interrupted,
            cache: HashMap::new(),
            metrics: Metrics::default(),
            example: Vec::new(),
            started,
        }
    }

    pub fn run(mut self, mut state: State) -> LaneReport {
        let outcome = if state.root_conflict {
            LaneOutcome::No
        } else {
            self.dfs(&mut state)
        };
        LaneReport {
            config: self.config,
            outcome,
            seconds: self.started.elapsed().as_secs_f64(),
            metrics: self.metrics,
            example: self.example,
        }
    }

    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
            || self.interrupted.load(Ordering::Acquire)
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn stop_outcome(&self) -> LaneOutcome {
        if self.cancel.load(Ordering::Acquire) || self.interrupted.load(Ordering::Acquire) {
            LaneOutcome::Cancelled
        } else {
            LaneOutcome::Unknown
        }
    }

    fn factors(&mut self, sum: usize) -> Arc<[Pair]> {
        if let Some(found) = self.cache.get(&sum) {
            return Arc::clone(found);
        }
        let result = self.problem.factor_pairs(sum);
        self.cache.insert(sum, Arc::clone(&result));
        result
    }

    fn banned(&self, state: &State, value: usize) -> bool {
        bit(&state.banned_bits, value)
    }

    fn live(&self, state: &State, pair: Pair) -> bool {
        !self.banned(state, pair.0) && !self.banned(state, pair.1)
    }

    fn satisfied(&self, state: &State, sum: usize) -> bool {
        state.product_count[sum] != 0
    }

    fn add(&mut self, state: &mut State, value: usize) -> bool {
        if value == 0 || value > self.problem.maximum || self.banned(state, value) {
            return false;
        }
        if state.member[value] != 0 {
            return true;
        }
        if self.config.early_prime_filter {
            let begin = state.members.len().saturating_sub(8);
            for &other in &state.members[begin..] {
                let sum = value + other;
                if self.problem.prime[sum]
                    && (sum > self.problem.maximum || self.banned(state, sum))
                {
                    return false;
                }
            }
        }
        state.member[value] = 1;
        set_bit(&mut state.member_bits, value);
        if state.maximum_deletion && value < self.problem.maximum {
            let complement = self.problem.maximum - value;
            if complement == value || state.member[complement] != 0 {
                state.additive_satisfied += 1;
            }
        }
        let product_limit = 2 * self.problem.maximum;
        for &other in &state.members {
            if other <= product_limit / value {
                let product = value * other;
                state.product_count[product] += 1;
                state.product_undo.push(product);
            }
        }
        if value <= product_limit / value {
            let product = value * value;
            state.product_count[product] += 1;
            state.product_undo.push(product);
        }
        state.members.push(value);
        true
    }

    fn ban(&self, state: &mut State, value: usize) -> bool {
        if value == 0 || value > self.problem.maximum {
            return true;
        }
        if state.member[value] != 0 {
            return false;
        }
        let word = value >> 6;
        let mask = 1_u64 << (value & 63);
        if state.banned_bits[word] & mask == 0 {
            state.banned_undo.push((word, state.banned_bits[word]));
            state.banned_bits[word] |= mask;
            state.clear_additive_for_value(self.problem.maximum, value, true);
            if self.problem.prime[value] {
                let member_count = state.members.len();
                for index in 0..member_count {
                    let member = state.members[index];
                    if member < value && !self.ban(state, value - member) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn apply_banned_prime_bans(&self, state: &mut State, member: usize) -> bool {
        let first = member + 1;
        let last = self.problem.maximum;
        for word in (first >> 6)..=(last >> 6) {
            let base = word << 6;
            let mut mask = self.problem.prime_bits[word] & state.banned_bits[word];
            if base < first {
                mask &= u64::MAX << (first - base);
            }
            if base + 63 > last {
                mask &= u64::MAX >> (base + 63 - last);
            }
            while mask != 0 {
                let offset = mask.trailing_zeros() as usize;
                if !self.ban(state, base + offset - member) {
                    return false;
                }
                mask &= mask - 1;
            }
        }
        true
    }

    fn propagate_nogoods(&mut self, state: &mut State) -> bool {
        for index in 0..state.nogoods.len() {
            let (d, e) = state.nogoods[index];
            if state.member[d] != 0 && state.member[e] != 0 {
                self.metrics.conflicts += 1;
                return false;
            }
            if state.member[d] != 0 && !self.banned(state, e) && !self.ban(state, e) {
                return false;
            }
            if state.member[e] != 0 && !self.banned(state, d) && !self.ban(state, d) {
                return false;
            }
        }
        true
    }

    fn mark_active(state: &mut State, sum: usize) {
        if !bit(&state.active_bits, sum) {
            set_bit(&mut state.active_bits, sum);
            state.active_undo.push(sum);
        }
    }

    fn mark_done(state: &mut State, sum: usize) {
        if !bit(&state.done_bits, sum) {
            set_bit(&mut state.done_bits, sum);
            state.done_undo.push(sum);
        }
    }

    fn set_watch(state: &mut State, sum: usize, first: usize, second: usize) {
        state
            .watch_undo
            .push((sum, state.watch1[sum], state.watch2[sum]));
        state.watch1[sum] = first;
        state.watch2[sum] = second;
    }

    fn checkpoint(state: &State) -> Checkpoint {
        Checkpoint {
            members: state.members.len(),
            demands: state.demands.len(),
            processed: state.processed,
            banned: state.banned_undo.len(),
            products: state.product_undo.len(),
            active: state.active_undo.len(),
            done: state.done_undo.len(),
            watches: state.watch_undo.len(),
            nogoods: state.nogoods.len(),
            additive: state.additive_undo.len(),
            additive_satisfied: state.additive_satisfied,
        }
    }

    fn rollback(state: &mut State, point: Checkpoint) {
        while state.watch_undo.len() > point.watches {
            let (sum, first, second) = state.watch_undo.pop().unwrap();
            state.watch1[sum] = first;
            state.watch2[sum] = second;
        }
        while state.done_undo.len() > point.done {
            clear_bit(&mut state.done_bits, state.done_undo.pop().unwrap());
        }
        while state.active_undo.len() > point.active {
            clear_bit(&mut state.active_bits, state.active_undo.pop().unwrap());
        }
        while state.banned_undo.len() > point.banned {
            let (word, old) = state.banned_undo.pop().unwrap();
            state.banned_bits[word] = old;
        }
        while state.additive_undo.len() > point.additive {
            let (word, old, count) = state.additive_undo.pop().unwrap();
            state.additive_live[word] = old;
            state.additive_count = count;
        }
        while state.product_undo.len() > point.products {
            let product = state.product_undo.pop().unwrap();
            state.product_count[product] -= 1;
        }
        while state.members.len() > point.members {
            let value = state.members.pop().unwrap();
            state.member[value] = 0;
            clear_bit(&mut state.member_bits, value);
        }
        state.demands.truncate(point.demands);
        state.processed = point.processed;
        state.nogoods.truncate(point.nogoods);
        state.additive_satisfied = point.additive_satisfied;
    }

    fn apply_prime_bans(&mut self, state: &mut State, a: usize) -> bool {
        let low = self.problem.maximum + 1 - a;
        let first_word = low.max(1) >> 6;
        let last_word = self.problem.maximum >> 6;
        for word in first_word..=last_word {
            let base = word << 6;
            let mut mask = bit_window(&self.problem.prime_bits, base as isize + a as isize);
            if base < low {
                mask &= u64::MAX << (low - base);
            }
            if base + 63 > self.problem.maximum {
                mask &= u64::MAX >> (base + 63 - self.problem.maximum);
            }
            mask &= !state.banned_bits[word];
            if mask == 0 {
                continue;
            }
            if state.member_bits[word] & mask != 0 {
                self.metrics.conflicts += 1;
                return false;
            }
            let mut newly_banned = mask;
            while newly_banned != 0 {
                let offset = newly_banned.trailing_zeros() as usize;
                state.clear_additive_for_value(self.problem.maximum, base + offset, true);
                newly_banned &= newly_banned - 1;
            }
            let updated = state.banned_bits[word] | mask;
            if updated != state.banned_bits[word] {
                state.banned_undo.push((word, state.banned_bits[word]));
                state.banned_bits[word] = updated;
            }
        }
        true
    }

    fn activate_sum(&mut self, state: &mut State, a: usize, sum: usize) -> bool {
        if self.problem.prime[sum] {
            if sum > self.problem.maximum || !self.add(state, sum) {
                self.metrics.conflicts += 1;
                return false;
            }
            return true;
        }
        if bit(&state.active_bits, sum) {
            return true;
        }
        if self.satisfied(state, sum) {
            Self::mark_active(state, sum);
            Self::mark_done(state, sum);
            return true;
        }
        let options = self.factors(sum);
        let mut first = None;
        let mut second = None;
        for (index, &option) in options.iter().enumerate() {
            self.metrics.factor_visits += 1;
            if self.live(state, option) {
                if first.is_none() {
                    first = Some(index);
                } else if second.is_none() {
                    second = Some(index);
                    break;
                }
            }
        }
        Self::mark_active(state, sum);
        let Some(first) = first else {
            self.metrics.conflicts += 1;
            return false;
        };
        if second.is_none() {
            let (d, e) = options[first];
            if !self.add(state, d) || !self.add(state, e) {
                self.metrics.conflicts += 1;
                return false;
            }
            Self::mark_done(state, sum);
            return true;
        }
        state.demands.push(sum);
        state.reason_a[sum] = a;
        state.reason_b[sum] = sum - a;
        Self::set_watch(state, sum, first, second.unwrap());
        true
    }

    fn activate_translates(&mut self, state: &mut State, a: usize) -> Result<(), LaneOutcome> {
        let snapshot = state.members.len();
        for index in 0..snapshot {
            if index & 8191 == 0 && self.stopped() {
                return Err(self.stop_outcome());
            }
            let sum = a + state.members[index];
            if !self.activate_sum(state, a, sum) {
                return Err(LaneOutcome::No);
            }
        }
        Ok(())
    }

    fn additive_options(&self, state: &State) -> Vec<Pair> {
        if !state.maximum_deletion || state.additive_satisfied != 0 {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(state.additive_count);
        for (word_index, &word) in state.additive_live.iter().enumerate() {
            let mut live = word;
            while live != 0 {
                let offset = live.trailing_zeros() as usize;
                let a = (word_index << 6) + offset + 1;
                result.push((a, self.problem.maximum - a));
                live &= live - 1;
            }
        }
        result
    }

    fn propagate_additive(&mut self, state: &mut State) -> Result<bool, LaneOutcome> {
        if !state.maximum_deletion || state.additive_satisfied != 0 {
            return Ok(false);
        }
        if state.additive_count == 0 {
            self.metrics.conflicts += 1;
            return Err(LaneOutcome::No);
        }
        if state.additive_count != 1 {
            return Ok(false);
        }
        let old = state.members.len();
        let pair = self.additive_options(state)[0];
        if !self.add(state, pair.0) || !self.add(state, pair.1) {
            self.metrics.conflicts += 1;
            return Err(LaneOutcome::No);
        }
        Ok(state.members.len() != old)
    }

    fn propagate(&mut self, state: &mut State) -> Result<(), LaneOutcome> {
        loop {
            while state.processed < state.members.len() {
                if self.stopped() {
                    return Err(self.stop_outcome());
                }
                if !self.propagate_nogoods(state) {
                    return Err(LaneOutcome::No);
                }
                let a = state.members[state.processed];
                state.processed += 1;
                if !self.apply_banned_prime_bans(state, a) {
                    return Err(LaneOutcome::No);
                }
                if !self.apply_prime_bans(state, a) {
                    return Err(LaneOutcome::No);
                }
                self.activate_translates(state, a)?;
            }
            if !self.propagate_nogoods(state) {
                return Err(LaneOutcome::No);
            }
            let mut changed = self.propagate_additive(state)?;
            for index in 0..state.demands.len() {
                self.metrics.demand_scans += 1;
                if self.metrics.demand_scans & 8191 == 0 && self.stopped() {
                    return Err(self.stop_outcome());
                }
                let sum = state.demands[index];
                if bit(&state.done_bits, sum) {
                    continue;
                }
                let options = self.factors(sum);
                if self.satisfied(state, sum) {
                    Self::mark_done(state, sum);
                    continue;
                }
                let one = options[state.watch1[sum]];
                let two = options[state.watch2[sum]];
                if self.live(state, one) && self.live(state, two) {
                    continue;
                }
                self.metrics.watch_rescans += 1;
                let mut live = Vec::with_capacity(2);
                for (option_index, &option) in options.iter().enumerate() {
                    self.metrics.factor_visits += 1;
                    if self.live(state, option) && live.len() < 2 {
                        live.push(option_index);
                        if live.len() == 2 {
                            break;
                        }
                    }
                }
                if live.is_empty() {
                    self.metrics.conflicts += 1;
                    return Err(LaneOutcome::No);
                } else if live.len() == 1 {
                    let old = state.members.len();
                    let (d, e) = options[live[0]];
                    if !self.add(state, d) || !self.add(state, e) {
                        self.metrics.conflicts += 1;
                        return Err(LaneOutcome::No);
                    }
                    Self::mark_done(state, sum);
                    changed |= state.members.len() > old;
                } else {
                    Self::set_watch(state, sum, live[0], live[1]);
                }
            }
            if !changed && state.processed == state.members.len() {
                return Ok(());
            }
        }
    }

    fn incompatible(&mut self, state: &State, left: Pair, right: Pair) -> bool {
        for x in [left.0, left.1] {
            for y in [right.0, right.1] {
                self.metrics.compatibility_checks += 1;
                let sum = x + y;
                if self.problem.prime[sum]
                    && (sum > self.problem.maximum || self.banned(state, sum))
                {
                    return true;
                }
            }
        }
        false
    }

    fn choose(&mut self, state: &mut State) -> Result<Option<Selected>, LaneOutcome> {
        let mut order = Vec::with_capacity(state.demands.len());
        if matches!(self.config.order, DemandOrder::MaximumFirst) {
            for &member in &state.members {
                let sum = self.problem.maximum + member;
                if bit(&state.active_bits, sum) && !bit(&state.done_bits, sum) {
                    order.push(sum);
                }
            }
        }
        for &sum in &state.demands {
            if !matches!(self.config.order, DemandOrder::MaximumFirst)
                || !(sum > self.problem.maximum && state.member[sum - self.problem.maximum] != 0)
            {
                order.push(sum);
            }
        }
        let mut choices = Vec::new();
        let mut fallback = None;
        for sum in order {
            if self.stopped() {
                return Err(self.stop_outcome());
            }
            if bit(&state.done_bits, sum) {
                continue;
            }
            let options = self.factors(sum);
            if self.satisfied(state, sum) {
                Self::mark_done(state, sum);
                continue;
            }
            let mut live = Vec::with_capacity(3);
            for &option in options.iter() {
                self.metrics.factor_visits += 1;
                if self.live(state, option) && live.len() < 3 {
                    live.push(option);
                    if live.len() == 3 {
                        break;
                    }
                }
            }
            if live.len() < 2 {
                return Err(LaneOutcome::No);
            }
            if live.len() == 2 {
                let endpoints = live
                    .iter()
                    .map(|&(d, e)| {
                        usize::from(state.member[d] == 0)
                            + usize::from(d != e && state.member[e] == 0)
                    })
                    .sum();
                choices.push(Choice {
                    sum,
                    endpoints,
                    degree: 0,
                    options: live,
                    valid: vec![true, true],
                });
                if choices.len() >= self.config.lookahead {
                    break;
                }
            } else if fallback.is_none() {
                fallback = Some(sum);
            }
        }
        if choices.is_empty() {
            let additive = self.additive_options(state);
            let product = fallback.map(|sum| {
                let factors = self.factors(sum);
                let options: Vec<_> = factors
                    .iter()
                    .copied()
                    .filter(|&pair| self.live(state, pair))
                    .collect();
                (sum, options)
            });
            let (sum, mut options) = match product {
                Some((sum, options)) if additive.is_empty() || options.len() <= additive.len() => {
                    (sum, options)
                }
                Some(_) | None if !additive.is_empty() => (self.problem.maximum, additive),
                None => return Ok(None),
                Some(_) => unreachable!(),
            };
            self.order_witnesses(sum, &mut options);
            return Ok(Some(Selected {
                valid: vec![true; options.len()],
                options,
            }));
        }

        let should_filter = match self.config.compatibility {
            Compatibility::None => false,
            Compatibility::Full => true,
            Compatibility::Adaptive => {
                let sample = choices.len().min(8);
                let mut conflict = false;
                'outer: for i in 0..sample {
                    for j in i + 1..sample {
                        if self.incompatible(state, choices[i].options[0], choices[j].options[0]) {
                            conflict = true;
                            break 'outer;
                        }
                    }
                }
                conflict
            }
        };
        if should_filter {
            let count = choices.len();
            let mut compatible = vec![true; count * count * 4];
            for i in 0..count {
                for j in i + 1..count {
                    for q in 0..2 {
                        for r in 0..2 {
                            let ok = !self.incompatible(
                                state,
                                choices[i].options[q],
                                choices[j].options[r],
                            );
                            compatible[(i * count + j) * 4 + q * 2 + r] = ok;
                            compatible[(j * count + i) * 4 + r * 2 + q] = ok;
                            if !ok {
                                choices[i].degree += 1;
                                choices[j].degree += 1;
                            }
                        }
                    }
                }
            }
            loop {
                let mut changed = false;
                for i in 0..count {
                    for q in 0..2 {
                        if !choices[i].valid[q] {
                            continue;
                        }
                        for j in 0..count {
                            if i == j {
                                continue;
                            }
                            let supported = (0..2).any(|r| {
                                choices[j].valid[r] && compatible[(i * count + j) * 4 + q * 2 + r]
                            });
                            if !supported {
                                choices[i].valid[q] = false;
                                self.metrics.support_rejections += 1;
                                changed = true;
                                break;
                            }
                        }
                    }
                }
                if !changed {
                    break;
                }
            }

            // Each binary demand can designate either witness. Every forbidden
            // pair of designations is a 2-SAT clause. Transitive implication
            // closure detects contradictions and assignments that force their
            // own opposite; this only removes witness choices that cannot occur
            // in any completion of the current state.
            let literal_count = 2 * count;
            let word_count = literal_count.div_ceil(64);
            let mut reach = vec![vec![0_u64; word_count]; literal_count];
            for (literal, row) in reach.iter_mut().enumerate() {
                set_bit(row, literal);
            }
            for i in 0..count {
                for q in 0..2 {
                    if !choices[i].valid[q] {
                        set_bit(&mut reach[2 * i + q], 2 * i + 1 - q);
                    }
                }
                for j in i + 1..count {
                    for q in 0..2 {
                        for r in 0..2 {
                            if !compatible[(i * count + j) * 4 + q * 2 + r] {
                                set_bit(&mut reach[2 * i + q], 2 * j + 1 - r);
                                set_bit(&mut reach[2 * j + r], 2 * i + 1 - q);
                            }
                        }
                    }
                }
            }
            for pivot in 0..literal_count {
                let pivot_row = reach[pivot].clone();
                for row in &mut reach {
                    if bit(row, pivot) {
                        for word in 0..word_count {
                            row[word] |= pivot_row[word];
                        }
                    }
                }
            }
            for i in 0..count {
                for q in 0..2 {
                    if choices[i].valid[q] && bit(&reach[2 * i + q], 2 * i + 1 - q) {
                        choices[i].valid[q] = false;
                        self.metrics.support_rejections += 1;
                    }
                }
                if !choices[i].valid[0] && !choices[i].valid[1] {
                    self.metrics.conflicts += 1;
                    return Err(LaneOutcome::No);
                }
            }
        }
        let pick = (0..choices.len())
            .min_by_key(|&i| {
                let choice = &choices[i];
                (
                    choice.valid.iter().filter(|v| **v).count(),
                    usize::MAX - choice.degree,
                    choice.endpoints,
                    usize::from(choice.sum <= self.problem.maximum),
                    usize::MAX - choice.sum,
                )
            })
            .unwrap();
        let mut choice = choices.swap_remove(pick);
        self.order_witnesses(choice.sum, &mut choice.options);
        if choice.valid.len() > 1 {
            let shift = self.witness_shift(choice.sum, choice.valid.len());
            choice.valid.rotate_left(shift);
        }
        if self.config.reverse_witnesses {
            choice.valid.reverse();
        }
        Ok(Some(Selected {
            options: choice.options,
            valid: choice.valid,
        }))
    }

    fn order_witnesses(&self, sum: usize, options: &mut [Pair]) {
        if options.len() > 1 {
            let shift = self.witness_shift(sum, options.len());
            options.rotate_left(shift);
        }
        if self.config.reverse_witnesses {
            options.reverse();
        }
    }

    fn witness_shift(&self, sum: usize, count: usize) -> usize {
        mix64(sum as u64 ^ mix64(self.config.rotation as u64)) as usize % count
    }

    fn dfs(&mut self, state: &mut State) -> LaneOutcome {
        self.metrics.nodes += 1;
        if self.stopped() {
            return self.stop_outcome();
        }
        if let Err(outcome) = self.propagate(state) {
            return outcome;
        }
        let selected = match self.choose(state) {
            Ok(Some(selected)) => selected,
            Ok(None) => {
                self.example = state.members.clone();
                self.example.sort_unstable();
                return LaneOutcome::Yes;
            }
            Err(outcome) => return outcome,
        };
        for (index, (d, e)) in selected.options.into_iter().enumerate() {
            if self.stopped() {
                return self.stop_outcome();
            }
            self.metrics.branches += 1;
            let point = Self::checkpoint(state);
            let outcome = if !selected.valid[index] || !self.add(state, d) || !self.add(state, e) {
                self.metrics.conflicts += 1;
                LaneOutcome::No
            } else {
                self.dfs(state)
            };
            Self::rollback(state, point);
            match outcome {
                LaneOutcome::No => {
                    if self.config.learn_nogoods {
                        state.nogoods.push((d, e));
                        if d + e == self.problem.maximum {
                            state.clear_additive_for_value(self.problem.maximum, d, true);
                        }
                        self.metrics.learned_pairs += 1;
                        if !self.propagate_nogoods(state) {
                            return LaneOutcome::No;
                        }
                    }
                }
                other => return other,
            }
        }
        LaneOutcome::No
    }
}

fn bit(bits: &[u64], value: usize) -> bool {
    bits[value >> 6] >> (value & 63) & 1 != 0
}

fn set_bit(bits: &mut [u64], value: usize) {
    bits[value >> 6] |= 1_u64 << (value & 63);
}

fn clear_bit(bits: &mut [u64], value: usize) {
    bits[value >> 6] &= !(1_u64 << (value & 63));
}

fn bit_window(source: &[u64], start: isize) -> u64 {
    if start <= -64 {
        return 0;
    }
    if start < 0 {
        return source.first().copied().unwrap_or(0) << (-start as usize);
    }
    let index = start as usize >> 6;
    let offset = start as usize & 63;
    if index >= source.len() {
        return 0;
    }
    let mut result = source[index] >> offset;
    if offset != 0 && index + 1 < source.len() {
        result |= source[index + 1] << (64 - offset);
    }
    result
}

fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_small_control(state: State, maximum: usize) -> LaneReport {
        Solver::new(
            Arc::new(Problem::new(maximum)),
            LaneConfig {
                lane: 0,
                lookahead: 1,
                order: DemandOrder::Insertion,
                compatibility: Compatibility::None,
                reverse_witnesses: false,
                learn_nogoods: true,
                early_prime_filter: true,
                rotation: 0,
            },
            0.0,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        )
        .run(state)
    }

    #[test]
    fn maximum_two_preserves_the_valid_singleton() {
        let problem = Problem::new(2);
        for state in [
            State::seeded(2).unwrap(),
            State::prepared(&problem).unwrap(),
        ] {
            let report = run_small_control(state, 2);
            assert_eq!(report.outcome, LaneOutcome::Yes);
            assert_eq!(report.example, vec![2]);
            assert!(crate::events::validate(2, &report.example));
        }
    }

    #[test]
    fn maximum_two_loaded_root_preserves_explicit_assumptions() {
        let path = std::env::temp_dir().join(format!(
            "apa-original-singleton-control-{}.state.txt",
            std::process::id()
        ));
        fs::write(&path, "F 2\nB 1\n").unwrap();
        let singleton = State::from_file(2, &path).unwrap();
        fs::write(&path, "F 1\nF 2\n").unwrap();
        let forced_one = State::from_file(2, &path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(run_small_control(singleton, 2).outcome, LaneOutcome::Yes);
        assert_eq!(run_small_control(forced_one, 2).outcome, LaneOutcome::No);
    }

    #[test]
    fn maximum_deletion_rejects_the_singleton_case() {
        let problem = Problem::new(2);
        let mut state = State::seeded(2).unwrap();
        assert!(state.enable_maximum_deletion(&problem, 1).is_err());
        assert!(!state.maximum_deletion);
    }

    #[test]
    fn seeded_state_has_required_members() {
        let state = State::seeded(113).unwrap();
        assert_eq!(state.member[1], 1);
        assert_eq!(state.member[2], 1);
        assert_eq!(state.member[113], 1);
    }

    #[test]
    fn bit_windows_cross_words() {
        let mut bits = vec![0_u64; 3];
        set_bit(&mut bits, 63);
        set_bit(&mut bits, 64);
        assert_eq!(bit_window(&bits, 63) & 3, 3);
    }

    #[test]
    fn maximum_deletion_builds_exact_domains() {
        let problem = Problem::new(25);
        let mut state = State::empty(25);
        state.insert_root_member(1).unwrap();
        state.insert_root_member(2).unwrap();
        state.insert_root_member(25).unwrap();
        state.enable_maximum_deletion(&problem, 5).unwrap();

        assert!(state.maximum_deletion);
        assert_eq!(state.additive_count, 11); // 12 pairs, but 5+20 uses banned sqrt(25).
        assert!(bit(&state.banned_bits, 5));
        assert_eq!(state.additive_satisfied, 0);
    }

    #[test]
    fn additive_domain_and_satisfaction_roll_back() {
        let problem = Arc::new(Problem::new(17));
        let mut state = State::empty(17);
        state.insert_root_member(1).unwrap();
        state.insert_root_member(2).unwrap();
        state.insert_root_member(17).unwrap();
        state.enable_maximum_deletion(&problem, 4).unwrap();
        let config = LaneConfig {
            lane: 0,
            lookahead: 1,
            order: DemandOrder::Insertion,
            compatibility: Compatibility::None,
            reverse_witnesses: false,
            learn_nogoods: true,
            early_prime_filter: false,
            rotation: 0,
        };
        let mut solver = Solver::new(
            problem,
            config,
            0.0,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let point = Solver::checkpoint(&state);
        assert!(solver.ban(&mut state, 4));
        assert_eq!(state.additive_count, 7);
        assert!(solver.add(&mut state, 8));
        assert!(solver.add(&mut state, 9));
        assert_eq!(state.additive_satisfied, 1);
        Solver::rollback(&mut state, point);
        assert_eq!(state.additive_count, 8);
        assert_eq!(state.additive_satisfied, 0);
        assert_eq!(state.member[8], 0);
        assert_eq!(state.member[9], 0);
    }
}
