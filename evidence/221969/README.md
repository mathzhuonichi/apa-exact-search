# Complete computer-assisted exclusion of maximum 221969

## Theorem

There is no finite set `A` of positive integers satisfying both `max(A)=221969`
and `A+A subseteq A*A`, where `A*A` denotes the set of all products of two
members of `A`. Repetition is allowed in both sums and products.

This is a self-contained computer-assisted proof. Its only initial membership
premises are `1`, `2`, and the assumed maximum `221969`. The lemma below proves
that 1 and 2 must be present. No historical seed list, previously excluded
maximum, maximum-deletion lemma, SAT solver, or Rust search result is used.
It is not a proof-assistant formalization.

The complete finite derivation is in `proof.json.gz` and
`root-premises.json.gz`. The included `verify.py` independently checks it using
integer arithmetic and trial division. Both compressed files are ordinary
UTF-8 JSON when decompressed; all individual assumptions, sums, deductions,
case alternatives, and contradictions are retained.

## 1. The initial membership lemma

Write `n=221969`. Suppose such a set A exists, and let `m=min(A)`.

If `m>=3`, the mandatory sum `2m` is smaller than every product, since every
product is at least `m^2>2m`. This is impossible.

If `m=2`, there is an element greater than 2 because `max(A)=n>2`. Let b be
the least such element. A product is either `2*2=4` or is at least `2b`.
But `4<2+b<2b`. The mandatory sum `2+b` therefore has no product witness,
again impossible.

Consequently `m=1`, so 1 is a member. The sum `1+1=2` can only be witnessed
by the product `1*2`, so 2 is also a member. Finally n is a member by the
assumption on the maximum. Thus `{1,2,n}` is established without any search.

## 2. Exact inference rules

Every step in the certificate uses the following rules.

1. **Complete factor domain.** If a and b are mandatory, enumerate every
   pair `(d,e)` satisfying `d*e=a+b` and `1<=d<=e<=n`. At least one such pair
   must have both endpoints in A. Enumerating every divisor up to the integer
   square root gives the complete list.
2. **Excluded endpoints and incompatible pairs.** A pair cannot witness a sum
   when one endpoint has been proved absent, or when the simultaneous presence
   of its two endpoints has already been completely refuted at the current
   unconditional root.
3. **Common endpoint.** If an endpoint belongs to every surviving factor pair
   of a mandatory sum, that endpoint is mandatory. If no pair survives, there
   is a contradiction.
4. **Prime exclusion.** If a is mandatory and `a+e` is a prime greater than n,
   then e is absent. A prime greater than n has no product witness with both
   factors at most n.
5. **Prime-chain exclusion.** To disprove e, temporarily assume e is a member.
   Starting from mandatory values, repeatedly add either 2 or e when the sum
   is prime. Every resulting prime must be a member. Reaching a prime greater
   than n refutes the temporary assumption. The certificate records the
   complete chain, and the checker verifies every addition and primality test.
6. **Exhaustive witness cases.** Split a mandatory sum over its complete current
   live factor domain. For each option, assume both endpoints and apply the
   same rules. A contradictory option can be excluded. A fact derived in
   every surviving option is unconditional. A capped or unfinished option is
   retained as a survivor; it never counts as a refutation.
7. **Failed absence.** If assuming that a particular value is absent leads to
   a complete arithmetic contradiction, the value must be present. The only
   event of this type in this certificate establishes 4.

The soundness of rules 1-7 follows directly from the required inclusion
`A+A subseteq A*A`. By induction on the recorded derivation, every root fact
is a necessary condition on the same hypothetical set A. In particular,
conditional contradictions are not silently promoted to unconditional ones:
case coverage or explicit discharge of a single absence assumption is checked.

## 3. Starting the derivation

The prime sums `1+2=3`, `2+3=5`, and `2+5=7` force 3, 5, and 7.
The only bounded factor pair of `n+2=221971` is `(67,3313)`, so both factors
are mandatory. Since `n+74=222043` is prime, 74 is absent. The mandatory sum
`67+7=74` therefore forces 37 through `(2,37)`. Similarly `n+38=222007` is
prime, and `37+1=38` forces 19 through `(2,19)`.

This establishes exactly the initial ten-member root:

```text
1, 2, 3, 5, 7, 19, 37, 67, 3313, 221969
```

The checker derives this root itself. It then verifies all 26,114 initial
prime-chain exclusions in `root-premises.json.gz` against these ten members.
No additional seed membership is accepted from that file.

The first five exhaustive witness splits are on sums `19+1`, `47+7`,
`67+1`, `73+7`, and `71+1`. They establish the following additional members:

```text
11, 13, 17, 23, 41, 43, 47;
29, 31;
71, 73;
83;
79.
```

Further prime-chain exclusions allow the assumption `4 not in A` to be
completely refuted. The certificate retains all 62 local deductions of that
refutation. Subsequent exhaustive witness cases and common-endpoint
propagation derive the large mandatory set needed for the final contradiction.
The entire continuation is recorded; there is no unrecorded appeal to an
earlier seed theorem or an earlier campaign.

## 4. The final contradiction

The checked derivation forces both `221227` and `180181` to be members. Hence

```text
221227 + 180181 = 401408 = 2^13 * 7^2
```

must be a product of two members of A. The table lists **all 20 bounded factor
pairs**. In each row, the indicated endpoint e is excluded because the
mandatory partner c satisfies `c+e=q`, where q is the displayed prime and
`q>221969`. Each membership of c and each primality claim is checked in the
same finite derivation.

| Possible product | Excluded endpoint e | Mandatory partner c | Prime c+e |
| --- | ---: | ---: | ---: |
| 2 x 200704 | 200704 | 219979 | 420683 |
| 4 x 100352 | 100352 | 221219 | 321571 |
| 7 x 57344 | 57344 | 221219 | 278563 |
| 8 x 50176 | 50176 | 220513 | 270689 |
| 14 x 28672 | 28672 | 220471 | 249143 |
| 16 x 25088 | 25088 | 220889 | 245977 |
| 28 x 14336 | 14336 | 220973 | 235309 |
| 32 x 12544 | 12544 | 219979 | 232523 |
| 49 x 8192 | 8192 | 221069 | 229261 |
| 56 x 7168 | 7168 | 220681 | 227849 |
| 64 x 6272 | 6272 | 220859 | 227131 |
| 98 x 4096 | 98 | 221969 | 222067 |
| 112 x 3584 | 3584 | 221093 | 224677 |
| 128 x 3136 | 3136 | 221227 | 224363 |
| 196 x 2048 | 2048 | 220151 | 222199 |
| 224 x 1792 | 224 | 221969 | 222193 |
| 256 x 1568 | 1568 | 221219 | 222787 |
| 392 x 1024 | 392 | 221969 | 222361 |
| 448 x 896 | 896 | 221093 | 221989 |
| 512 x 784 | 784 | 221227 | 222011 |

Thus every possible product representation of 401408 is excluded, although
401408 is a mandatory sum. This contradicts `A+A subseteq A*A` and proves
the theorem.

## 5. Independent verification and completeness

From the repository root, run:

```text
python evidence/221969/verify.py
```

Or place the four proof files together and run `python verify.py`. No external
Python package, solver executable, network connection, or repository state is
required. Run without Python's `-O` option; the checker rejects that mode.

Expected output includes:

```text
independent_arithmetic_replay: PASS
result: NO
events: 29
arithmetic_steps: 101227
base_prime_chain_bans: 26114
members: 12981
```

The 101,227 steps are the recorded post-root arithmetic deductions; the 26,114
initial exclusion chains and the elementary starting-root derivation are also
checked. The checker recomputes full bounded factor domains by trial division,
independently of the producer's smallest-prime-factor table. It checks exact
case coverage, every common-endpoint deduction, every prime exclusion, every
failed assumption, and the final empty factor domain. Learned pair exclusions
are introduced only after their complete conditional contradiction has been
checked. Unknown or limited branches never supply a negative conclusion.

`verification.json` is the saved output of an actual successful run of this
packaged checker. `result.json` records the theorem's scope and the final
blocking table. These files document independently replayed computational
arithmetic evidence, not a formal Lean/Coq/Isabelle theorem. The source Markdown
and the arithmetic artifacts were checked; no visual-rendering claim is made.
