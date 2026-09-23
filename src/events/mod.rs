//! Opt-in event engine. The historical engine owns entirely separate state.
mod engine;
mod parallel_root;
mod problem;
pub mod proof;
mod runner;
#[cfg(test)]
mod tests;

pub use runner::{Config, run};

pub const U: &[usize] = &[
    1, 2, 3, 5, 7, 11, 13, 17, 19, 23, 31, 43, 47, 53, 61, 71, 73, 83, 103, 109, 113,
];
pub const GROUPS: &[&[usize]] = &[&[29, 37, 41], &[89, 97, 101], &[59], &[67], &[79], &[107]];

pub fn seed_cases() -> Vec<Vec<proof::Fact>> {
    (0..GROUPS.len())
        .map(|i| {
            GROUPS
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .flat_map(|(_, g)| g.iter().copied())
                .map(proof::Fact::Member)
                .collect()
        })
        .collect()
}

/// Raw validation deliberately does not inspect any solver tables.
pub fn validate(n: usize, values: &[usize]) -> bool {
    use std::collections::HashSet;
    if values.is_empty()
        || values.iter().copied().max() != Some(n)
        || values.iter().any(|&v| v == 0 || v > n)
        || values.iter().collect::<HashSet<_>>().len() != values.len()
    {
        return false;
    }
    let mut products = HashSet::new();
    for (i, &a) in values.iter().enumerate() {
        for &b in &values[i..] {
            if let Some(p) = a.checked_mul(b) {
                products.insert(p);
            }
        }
    }
    values.iter().enumerate().all(|(i, &a)| {
        values[i..]
            .iter()
            .all(|&b| a.checked_add(b).is_some_and(|s| products.contains(&s)))
    })
}
