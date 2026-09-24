//! Arithmetic proof checker: no dependency on the event engine or its factor tables.
use super::{GROUPS, U, seed_cases};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

pub type Id = usize;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Fact {
    Member(usize),
    Ban(usize),
    Bad(usize),
    Active(usize),
    Dead(usize, usize),
    Pair(usize, usize),
    AdditiveMaximum,
    False,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Cover {
    Factor(usize),
    Seeds,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Rule {
    Initial,
    Boundary,
    Assumption,
    Universal(usize),
    PrimeClause(usize, usize),
    EvenBound,
    Sum,
    Delete,
    Empty,
    Unique(usize, usize),
    BadSum,
    UnitPair,
    Conflict,
    Discharge(usize),
    Join {
        cover: Cover,
        scopes: Vec<usize>,
        context: usize,
    },
    PrimeChain {
        candidate: usize,
        steps: Vec<(usize, usize, usize)>,
    },
    PrefixAdditive {
        base: usize,
    },
    PrefixPair {
        base: usize,
    },
    AdditiveUnique {
        a: usize,
    },
    AdditiveEmpty,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub scope: usize,
    pub fact: Fact,
    pub rule: Rule,
    pub premises: Vec<Id>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scope {
    pub parent: Option<usize>,
    pub assumptions: Vec<Fact>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub maximum: usize,
    pub nodes: Vec<Node>,
    pub scopes: Vec<Scope>,
    pub root: Id,
}

#[derive(Clone)]
pub struct Arena {
    pub prefix: Arc<Vec<Node>>,
    pub nodes: Vec<Node>,
    pub scopes: Vec<Scope>,
}
impl Arena {
    pub fn new() -> Self {
        Self {
            prefix: Arc::new(Vec::new()),
            nodes: Vec::new(),
            scopes: vec![Scope {
                parent: None,
                assumptions: vec![],
            }],
        }
    }
    pub fn len(&self) -> usize {
        self.prefix.len() + self.nodes.len()
    }
    pub fn get(&self, id: Id) -> &Node {
        if id < self.prefix.len() {
            &self.prefix[id]
        } else {
            &self.nodes[id - self.prefix.len()]
        }
    }
    pub fn add(&mut self, scope: usize, fact: Fact, rule: Rule, premises: Vec<Id>) -> Id {
        let id = self.len();
        debug_assert!(premises.iter().all(|p| *p < id));
        self.nodes.push(Node {
            scope,
            fact,
            rule,
            premises,
        });
        id
    }
    pub fn enter(&mut self, parent: usize, assumptions: Vec<Fact>) -> usize {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            parent: Some(parent),
            assumptions,
        });
        id
    }
    pub fn freeze(&mut self) {
        if !self.nodes.is_empty() {
            Arc::make_mut(&mut self.prefix).append(&mut self.nodes);
        }
    }
    /// Import only the dependency closure of conclusions from a shared snapshot.
    /// Local proof IDs and scope IDs belong to the worker and must both be remapped.
    pub(super) fn import(&mut self, source: &Arena, roots: &[Id], shared_scopes: usize) -> Vec<Id> {
        assert!(Arc::ptr_eq(&self.prefix, &source.prefix));
        let base = self.prefix.len();
        let mut used = BTreeSet::new();
        let mut todo = roots.to_vec();
        while let Some(id) = todo.pop() {
            if id >= base && used.insert(id) {
                todo.extend(&source.get(id).premises);
            }
        }
        let mut scopes = BTreeSet::new();
        let mut scope_todo = vec![];
        for &id in &used {
            let node = source.get(id);
            scope_todo.push(node.scope);
            match &node.rule {
                Rule::Discharge(scope) => scope_todo.push(*scope),
                Rule::Join { scopes, .. } => scope_todo.extend(scopes),
                _ => {}
            }
        }
        while let Some(scope) = scope_todo.pop() {
            if scope >= shared_scopes && scopes.insert(scope) {
                scope_todo.extend(source.scopes[scope].parent);
            }
        }
        let mut scope_map = HashMap::new();
        for scope in scopes {
            let old = &source.scopes[scope];
            let parent = old.parent.unwrap();
            let parent = scope_map.get(&parent).copied().unwrap_or(parent);
            scope_map.insert(scope, self.enter(parent, old.assumptions.clone()));
        }
        let mut ids = HashMap::new();
        for id in used {
            let mut node = source.get(id).clone();
            node.scope = scope_map.get(&node.scope).copied().unwrap_or(node.scope);
            match &mut node.rule {
                Rule::Discharge(scope) => *scope = scope_map.get(scope).copied().unwrap_or(*scope),
                Rule::Join { scopes, .. } => {
                    for scope in scopes {
                        *scope = scope_map.get(scope).copied().unwrap_or(*scope);
                    }
                }
                _ => {}
            }
            node.premises = node
                .premises
                .iter()
                .map(|p| ids.get(p).copied().unwrap_or(*p))
                .collect();
            let new = self.add(node.scope, node.fact, node.rule, node.premises);
            ids.insert(id, new);
        }
        roots
            .iter()
            .map(|id| ids.get(id).copied().unwrap_or(*id))
            .collect()
    }
    pub(super) fn fragment(&self, roots: &[Id], shared_scopes: usize) -> (Arena, Vec<Id>) {
        let mut fragment = Arena {
            prefix: Arc::clone(&self.prefix),
            nodes: vec![],
            scopes: self.scopes[..shared_scopes].to_vec(),
        };
        let roots = fragment.import(self, roots, shared_scopes);
        (fragment, roots)
    }
    /// Fragments already contain exactly their dependency closure in topological
    /// order. Move them directly instead of walking and cloning the DAG again.
    pub(super) fn append_fragment(
        &mut self,
        fragment: Arena,
        mut roots: Vec<Id>,
        shared_scopes: usize,
    ) -> Vec<Id> {
        assert!(Arc::ptr_eq(&self.prefix, &fragment.prefix));
        let base = self.prefix.len();
        let node_offset = self.nodes.len();
        let scope_offset = self.scopes.len() - shared_scopes;
        let map_id = |id: &mut usize| {
            if *id >= base {
                *id += node_offset;
            }
        };
        let map_scope = |scope: &mut usize| {
            if *scope >= shared_scopes {
                *scope += scope_offset;
            }
        };
        self.scopes.extend(
            fragment
                .scopes
                .into_iter()
                .skip(shared_scopes)
                .map(|mut scope| {
                    if let Some(parent) = &mut scope.parent {
                        map_scope(parent);
                    }
                    scope
                }),
        );
        self.nodes
            .extend(fragment.nodes.into_iter().map(|mut node| {
                map_scope(&mut node.scope);
                for id in &mut node.premises {
                    map_id(id);
                }
                match &mut node.rule {
                    Rule::Discharge(scope) => map_scope(scope),
                    Rule::Join { scopes, .. } => scopes.iter_mut().for_each(map_scope),
                    _ => {}
                }
                node
            }));
        roots.iter_mut().for_each(map_id);
        roots
    }
    pub fn certificate(&self, n: usize, root: Id) -> Certificate {
        let mut used = BTreeSet::new();
        let mut todo = vec![root];
        while let Some(id) = todo.pop() {
            if used.insert(id) {
                todo.extend(&self.get(id).premises);
            }
        }
        let remap: HashMap<_, _> = used
            .iter()
            .enumerate()
            .map(|(new, &old)| (old, new))
            .collect();
        let nodes = used
            .into_iter()
            .map(|id| {
                let mut node = self.get(id).clone();
                node.premises = node.premises.iter().map(|p| remap[p]).collect();
                node
            })
            .collect();
        Certificate {
            maximum: n,
            nodes,
            scopes: self.scopes.clone(),
            root: remap[&root],
        }
    }
}

fn domain(n: usize, s: usize) -> Vec<(usize, usize)> {
    (1..=s)
        .take_while(|d| *d <= s / *d)
        .filter(|d| s.is_multiple_of(*d) && s / *d <= n)
        .map(|d| (d, s / d))
        .collect()
}
fn ancestor(scopes: &[Scope], ancestor: usize, mut child: usize) -> bool {
    loop {
        if ancestor == child {
            return true;
        }
        match scopes[child].parent {
            Some(p) => child = p,
            None => return false,
        }
    }
}
fn clause(p: usize, q: usize) -> bool {
    let a = GROUPS.iter().position(|g| g.contains(&p));
    let b = GROUPS.iter().position(|g| g.contains(&q));
    a.is_some() && b.is_some() && a != b
}
fn prefix_valid(n: usize, base: usize) -> bool {
    base >= 2
        && base < n
        && base
            .checked_mul(base - 1)
            .is_some_and(|threshold| n > threshold)
}
fn additive_cover_excluded(facts: &[&Fact], n: usize, survivor: Option<usize>) -> bool {
    let banned: HashSet<_> = facts
        .iter()
        .filter_map(|f| match f {
            Fact::Ban(v) => Some(*v),
            _ => None,
        })
        .collect();
    let pairs: HashSet<_> = facts
        .iter()
        .filter_map(|f| match f {
            Fact::Pair(d, e) => Some((*d, *e)),
            _ => None,
        })
        .collect();
    (1..=n / 2)
        .filter(|a| Some(*a) != survivor)
        .all(|a| banned.contains(&a) || banned.contains(&(n - a)) || pairs.contains(&(a, n - a)))
}

/// External universal lemmas are explicit trust inputs, never silently certified.
/// Returns whether the dependency closure actually uses an external lemma.
pub fn verify(cert: &Certificate, allow_external_lemmas: bool) -> Result<bool, String> {
    let n = cert.maximum;
    if !(1..=1_000_000).contains(&n)
        || cert.scopes.is_empty()
        || cert.scopes[0].parent.is_some()
        || !cert.scopes[0].assumptions.is_empty()
    {
        return Err("invalid root".into());
    }
    for (i, s) in cert.scopes.iter().enumerate().skip(1) {
        if !s.parent.is_some_and(|p| p < i)
            || s.assumptions.is_empty()
            || s.assumptions
                .iter()
                .any(|f| !matches!(f, Fact::Member(v) | Fact::Ban(v) if (1..=n).contains(v)))
        {
            return Err("invalid scope".into());
        }
    }
    let mut external = false;
    for (id, node) in cert.nodes.iter().enumerate() {
        if node.scope >= cert.scopes.len() || node.premises.iter().any(|p| *p >= id) {
            return Err(format!("forward reference at {id}"));
        }
        let premises: Vec<_> = node.premises.iter().map(|&p| &cert.nodes[p]).collect();
        let facts: Vec<_> = premises.iter().map(|p| &p.fact).collect();
        let has = |f: Fact| facts.contains(&&f);
        let all_local = || {
            premises
                .iter()
                .all(|p| ancestor(&cert.scopes, p.scope, node.scope))
        };
        let valid_fact = match node.fact {
            Fact::Member(v) | Fact::Ban(v) => (1..=n).contains(&v),
            Fact::Bad(s) | Fact::Active(s) => (2..=2 * n).contains(&s),
            Fact::Pair(d, e) | Fact::Dead(d, e) => {
                1 <= d
                    && d <= e
                    && e <= n
                    && (!matches!(node.fact, Fact::Dead(..))
                        || d.checked_mul(e).is_some_and(|p| (2..=2 * n).contains(&p)))
            }
            Fact::AdditiveMaximum | Fact::False => true,
        };
        let trusted_rule = matches!(
            node.rule,
            Rule::Universal(_)
                | Rule::PrimeClause(..)
                | Rule::EvenBound
                | Rule::PrefixAdditive { .. }
                | Rule::PrefixPair { .. }
        ) || matches!(
            node.rule,
            Rule::Join {
                cover: Cover::Seeds,
                ..
            }
        );
        if trusted_rule {
            external = true;
        }
        if let Rule::Unique(d, e) = node.rule
            && (d == 0
                || d > e
                || e > n
                || !d.checked_mul(e).is_some_and(|p| (2..=2 * n).contains(&p)))
        {
            return Err(format!("invalid unique witness at {id}"));
        }
        let ok = valid_fact
            && (!trusted_rule || (allow_external_lemmas && n > 2))
            && match &node.rule {
                Rule::Initial => {
                    node.scope == 0
                        && facts.is_empty()
                        && matches!(node.fact, Fact::Member(v) if n>2 && (v==1 || v==2 || v==n))
                }
                Rule::Boundary => {
                    node.scope == 0 && facts.is_empty() && n == 1 && node.fact == Fact::False
                }
                Rule::Assumption => {
                    facts.is_empty() && cert.scopes[node.scope].assumptions.contains(&node.fact)
                }
                Rule::Universal(v) => {
                    facts.is_empty()
                        && U.contains(v)
                        && (node.fact == Fact::Member(*v) || (*v > n && node.fact == Fact::False))
                }
                Rule::PrefixAdditive { base } => {
                    node.scope == 0
                        && facts.is_empty()
                        && prefix_valid(n, *base)
                        && node.fact == Fact::AdditiveMaximum
                }
                Rule::PrefixPair { base } => {
                    node.scope == 0
                        && facts.is_empty()
                        && prefix_valid(n, *base)
                        && matches!(node.fact, Fact::Pair(d,e) if d>=1 && d<=e && e<n && d.checked_mul(e)==Some(n))
                }
                Rule::AdditiveUnique { a } => {
                    all_local()
                        && (1..=n / 2).contains(a)
                        && has(Fact::AdditiveMaximum)
                        && additive_cover_excluded(&facts, n, Some(*a))
                        && matches!(node.fact, Fact::Member(v) if v==*a || v==n-*a)
                }
                Rule::AdditiveEmpty => {
                    all_local()
                        && node.fact == Fact::False
                        && has(Fact::AdditiveMaximum)
                        && additive_cover_excluded(&facts, n, None)
                }
                Rule::PrimeClause(p, q) => {
                    all_local()
                        && clause(*p, *q)
                        && (*p > n || has(Fact::Ban(*p)))
                        && (node.fact == Fact::Member(*q) || (node.fact == Fact::False && *q > n))
                }
                Rule::EvenBound => {
                    let allowed: Vec<_> = (2..=384.min(n))
                        .step_by(2)
                        .filter(|e| !has(Fact::Ban(*e)))
                        .collect();
                    all_local()
                        && match node.fact {
                            Fact::False => allowed.len() < 7,
                            Fact::Member(v) => allowed.len() == 7 && allowed.contains(&v),
                            _ => false,
                        }
                }
                Rule::Sum => {
                    all_local()
                        && matches!(node.fact, Fact::Active(s) if facts.iter().any(|f| matches!(f, Fact::Member(a) if facts.iter().any(|g| matches!(g, Fact::Member(b) if a+b==s)))))
                }
                Rule::Delete => {
                    all_local()
                        && matches!(node.fact, Fact::Dead(d,e) if has(Fact::Ban(d)) || has(Fact::Ban(e)) || has(Fact::Pair(d,e)) || has(Fact::Bad(d*e)) || has(Fact::Bad(d+e)))
                }
                Rule::Empty => {
                    all_local()
                        && matches!(node.fact, Fact::Bad(s) if domain(n,s).iter().all(|&(d,e)| has(Fact::Dead(d,e))))
                }
                Rule::Unique(d, e) => {
                    let s = d * e;
                    all_local()
                        && s >= 2
                        && s <= 2 * n
                        && has(Fact::Active(s))
                        && domain(n, s).contains(&(*d, *e))
                        && domain(n, s)
                            .into_iter()
                            .filter(|p| *p != (*d, *e))
                            .all(|(a, b)| has(Fact::Dead(a, b)))
                        && matches!(node.fact, Fact::Member(v) if v==*d || v==*e)
                }
                Rule::BadSum => {
                    all_local()
                        && matches!(node.fact, Fact::Ban(v) if has(Fact::Bad(2*v)) || facts.iter().any(|f| matches!(f, Fact::Member(a) if has(Fact::Bad(a+v)))))
                }
                Rule::UnitPair => {
                    all_local()
                        && matches!(node.fact, Fact::Ban(v) if facts.iter().any(|f| matches!(f, Fact::Pair(d,e) if (*d==v && (*e==v || has(Fact::Member(*e)))) || (*e==v && has(Fact::Member(*d))))))
                }
                Rule::Conflict => {
                    all_local()
                        && node.fact == Fact::False
                        && facts.iter().any(|f| match f {
                            Fact::Member(v) => has(Fact::Ban(*v)),
                            Fact::Bad(s) => has(Fact::Active(*s)),
                            Fact::Dead(d, e) | Fact::Pair(d, e) => {
                                has(Fact::Member(*d)) && has(Fact::Member(*e))
                            }
                            _ => false,
                        })
                }
                Rule::Discharge(child) => {
                    if *child >= cert.scopes.len() {
                        false
                    } else {
                        let scope = &cert.scopes[*child];
                        scope.parent == Some(node.scope)
                            && premises.len() == 1
                            && premises[0].scope == *child
                            && premises[0].fact == Fact::False
                            && match &node.fact {
                                Fact::Member(v) => scope.assumptions == vec![Fact::Ban(*v)],
                                Fact::Ban(v) => scope.assumptions == vec![Fact::Member(*v)],
                                Fact::Pair(d, e) => {
                                    scope.assumptions == vec![Fact::Member(*d), Fact::Member(*e)]
                                }
                                _ => false,
                            }
                    }
                }
                Rule::Join {
                    cover,
                    scopes,
                    context,
                } => {
                    if *context > premises.len() {
                        false
                    } else {
                        let ctx = &premises[..*context];
                        let outcomes = &premises[*context..];
                        let cases = match cover {
                            Cover::Seeds => seed_cases(),
                            Cover::Factor(s) if (2..=2 * n).contains(s) => domain(n, *s)
                                .into_iter()
                                .filter(|&(d, e)| !ctx.iter().any(|p| p.fact == Fact::Dead(d, e)))
                                .map(|(d, e)| vec![Fact::Member(d), Fact::Member(e)])
                                .collect(),
                            _ => vec![],
                        };
                        let context_ok = ctx
                            .iter()
                            .all(|p| ancestor(&cert.scopes, p.scope, node.scope))
                            && match cover {
                                Cover::Factor(s) => ctx.iter().any(|p| p.fact == Fact::Active(*s)),
                                Cover::Seeds => true,
                            };
                        context_ok
                            && !cases.is_empty()
                            && cases.len() == scopes.len()
                            && scopes.len() == outcomes.len()
                            && scopes.iter().zip(cases).zip(outcomes).all(
                                |((&scope, assumptions), p)| {
                                    scope < cert.scopes.len()
                                        && cert.scopes[scope].parent == Some(node.scope)
                                        && cert.scopes[scope].assumptions == assumptions
                                        && p.scope == scope
                                        && (p.fact == node.fact || p.fact == Fact::False)
                                },
                            )
                    }
                }
                Rule::PrimeChain { candidate, steps } => {
                    let mut known: HashSet<_> = facts
                        .iter()
                        .filter_map(|f| {
                            if let Fact::Member(v) = f {
                                Some(*v)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let mut valid = all_local()
                        && known.contains(&2)
                        && (1..=n).contains(candidate)
                        && !steps.is_empty();
                    let mut terminal = false;
                    for &(a, h, p) in steps {
                        let prime = p >= 2
                            && p <= 2 * n
                            && (2..=p)
                                .take_while(|d| *d <= p / *d)
                                .all(|d| !p.is_multiple_of(d));
                        valid &= !terminal
                            && a <= n
                            && known.contains(&a)
                            && (h == 2 || h == *candidate)
                            && a.checked_add(h) == Some(p)
                            && prime;
                        terminal = p > n || has(Fact::Ban(p));
                        known.insert(p);
                    }
                    valid && terminal && node.fact == Fact::Ban(*candidate)
                }
            };
        if !ok {
            return Err(format!("invalid inference at node {id}: {:?}", node.rule));
        }
    }
    if !cert
        .nodes
        .get(cert.root)
        .is_some_and(|p| p.scope == 0 && p.fact == Fact::False)
    {
        return Err("not a complete root refutation".into());
    }
    Ok(external)
}
