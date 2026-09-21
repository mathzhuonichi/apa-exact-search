use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Witness {
    pub d: usize,
    pub e: usize,
    pub s: usize,
}

#[derive(Debug)]
pub struct Csr {
    offsets: Vec<usize>,
    ids: Vec<usize>,
}

impl Csr {
    fn build(
        buckets: usize,
        entries: impl Iterator<Item = (usize, usize)> + Clone,
    ) -> Result<Self, String> {
        let mut offsets = vec![0usize; buckets + 1];
        for (b, _) in entries.clone() {
            offsets[b + 1] = offsets[b + 1].checked_add(1).ok_or("index overflow")?;
        }
        for i in 1..offsets.len() {
            offsets[i] = offsets[i]
                .checked_add(offsets[i - 1])
                .ok_or("index overflow")?;
        }
        let mut ids = vec![0; offsets[buckets]];
        let mut next = offsets.clone();
        for (b, id) in entries {
            ids[next[b]] = id;
            next[b] += 1;
        }
        Ok(Self { offsets, ids })
    }
    pub fn get(&self, bucket: usize) -> &[usize] {
        &self.ids[self.offsets[bucket]..self.offsets[bucket + 1]]
    }
}

#[derive(Debug)]
pub struct Problem {
    pub n: usize,
    pub limit: usize,
    pub witness: Vec<Witness>,
    pub offsets: Vec<usize>,
    pub endpoint: Csr,
    pub endpoint_sum: Csr,
    pub prime: Vec<bool>,
}

impl Problem {
    pub fn new(n: usize) -> Result<Self, String> {
        if !(1..=1_000_000).contains(&n) {
            return Err("maximum must be in [1, 1000000]".into());
        }
        let limit = n.checked_mul(2).ok_or("maximum overflow")?;
        let pairs = || {
            (1..=limit)
                .take_while(move |d| *d <= limit / *d)
                .flat_map(move |d| {
                    (d.max(2usize.div_ceil(d))..=n.min(limit / d)).map(move |e| Witness {
                        d,
                        e,
                        s: d * e,
                    })
                })
        };
        let mut offsets = vec![0usize; limit + 2];
        for w in pairs() {
            offsets[w.s + 1] = offsets[w.s + 1].checked_add(1).ok_or("witness overflow")?;
        }
        for i in 1..offsets.len() {
            offsets[i] = offsets[i]
                .checked_add(offsets[i - 1])
                .ok_or("offset overflow")?;
        }
        let mut witness = vec![Witness { d: 0, e: 0, s: 0 }; offsets[limit + 1]];
        let mut next = offsets.clone();
        for w in pairs() {
            witness[next[w.s]] = w;
            next[w.s] += 1;
        }
        let endpoint = Csr::build(
            n + 1,
            witness.iter().enumerate().flat_map(|(id, w)| {
                [(w.d, id), (w.e, id)]
                    .into_iter()
                    .take(if w.d == w.e { 1 } else { 2 })
            }),
        )?;
        let endpoint_sum = Csr::build(
            limit + 1,
            witness.iter().enumerate().map(|(id, w)| (w.d + w.e, id)),
        )?;
        let mut prime = vec![true; limit + 1];
        prime[0] = false;
        prime[1] = false;
        for p in 2..=limit {
            if prime[p] && p <= limit / p {
                for q in (p * p..=limit).step_by(p) {
                    prime[q] = false;
                }
            }
        }
        Ok(Self {
            n,
            limit,
            witness,
            offsets,
            endpoint,
            endpoint_sum,
            prime,
        })
    }
    pub fn domain(&self, s: usize) -> Range<usize> {
        self.offsets[s]..self.offsets[s + 1]
    }
    pub fn pair_id(&self, d: usize, e: usize) -> Option<usize> {
        let s = d.checked_mul(e)?;
        if s > self.limit || s < 2 {
            return None;
        }
        let range = self.domain(s);
        self.witness[range.clone()]
            .binary_search_by_key(&(d, e), |w| (w.d, w.e))
            .ok()
            .map(|i| range.start + i)
    }
}
