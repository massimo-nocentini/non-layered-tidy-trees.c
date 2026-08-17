# SIMD plan

## Context

`src/non-layered-tidy-trees.c` implements van der Ploeg's linear-time non-layered tidy
tree algorithm. The question is whether SIMD can speed it up. Measured facts about the
code as it stands today:

- **No loop in the file vectorizes.** `clang -O3 -Rpass-analysis=loop-vectorize` rejects
  all 12 loops: recursion (`call instruction cannot be vectorized`), pointer-chasing
  loops with multiple early exits (`Cannot vectorize early exit loop`), and scan
  dependencies (`value that could not be identified as reduction`).
- **Every node is a separate 168-byte `malloc`.** A 10k-node tree is 10k allocations
  scattered across the heap, and each node spans ~2.6 cache lines.
- **The hot loop is `separate()` (`:121`), and it is inherently serial** — a dependency
  chain of `mssr`/`mscl` accumulators, contour pointer chasing, and data-dependent exits.

Scope for this plan: the workload is **one large tree (10k+ nodes)**; output must stay
**bit-identical** to today; churn is **internals only** (`tree_t`, `layout`, `layout_api`
keep their current shape).

## Verdict

**SIMD is not where the speedup is.** With doubles, the portable vector width is 2 lanes
(NEON on arm64, SSE2 baseline on x86, wasm `simd128`) — 4 only with opt-in AVX2. The
algorithm is memory-latency- and branch-bound, not ALU-bound, and its hottest routine
cannot be vectorized at any width.

What *can* deliver a large win is the restructuring that SIMD would require anyway:
**arena-allocated struct-of-arrays + level-sweep iteration instead of recursion.** That
turns scattered 168-byte nodes into contiguous `double[]` arrays, and turns five
recursive traversals into flat sweeps. SIMD then applies naturally to the elementwise
sweeps, worth an estimated single-digit percent on top.

The restructuring also removes the recursion-depth limit (measured: segfault between
60k and 100k depth natively; a 64 KiB wasm stack fails in the low hundreds).

Recommendation: do steps 0–3 for the real win, then step 4 for SIMD, and treat step 4 as
optional polish rather than the goal.

## Where SIMD can and cannot apply

| Routine | Vectorizable? | Why |
|---|---|---|
| `separate` `:121` — contour walk | **No** | Serial accumulator chain, pointer chasing, data-dependent exits. The algorithm's real work. |
| `addChildSpacing` `:43` | **No**, under bit-exactness | Second-order prefix scan; any parallel scan reassociates FP additions and changes the last ULP. |
| `update_chain` `:22` / `free_chain` `:15` | No | Linked-list traversal. |
| `setupWalk` depth assignment `:289` | **Yes** | In level order, each parent writes one value to a *contiguous* run of children — a fill, no gather. |
| `bottom()` precomputation | **Yes** | `bot[i] = y[i] + h[i]` elementwise per level; also removes two branches per contour step. |
| `secondWalk` coordinate finalize `:236` | **Yes** | `x[i] = prelim[i] + modsum[i] (+ w[i]/2)` elementwise within a level. |
| `thirdWalk` translate `:298` | **Yes** | Pure elementwise `x -= dx`, `y -= dy` over all nodes. |
| `flat_xy_into` / `flat_xywh_into` | **Yes** | Becomes a contiguous copy once coordinates live in arrays. |
| `maxbottom` `:405` | Yes in principle | `fmax` reassociation *is* bit-exact, but subtree ranges are not contiguous in level order. Leave scalar. |

Bit-exactness holds for every "yes" above: they are elementwise operations, not
reassociated reductions.

## Steps

### 0. Regression oracle and benchmark first

Nothing below is safe without an exact oracle, and nothing should be optimized without a
baseline. Add a `test/` directory built by a new `make test` target:

- `test/sweep.c` — shape/size sweep (4 size regimes × depths 1–6 × 2–5 children × both
  orientations ≈ 57 600 trees), printing every coordinate and asserting **no two node
  boxes strictly overlap**.
- `test/golden.c` — dumps coordinates for a fixed seeded corpus. Freeze the output of
  today's build as `test/golden.txt`; every later step must reproduce it byte-for-byte.
- `test/bench.c` — wall-clock over trees of 1k/10k/100k/1M nodes, both orientations,
  reporting ns/node. Also a deep-chain case to track the recursion limit.
- Run under `-fsanitize=address,undefined` in CI (`.github/workflows/main.yml`).

Record the baseline profile before touching anything: confirm by measurement what share
of runtime is `separate` vs. the four traversals vs. `malloc`/`free`.

### 1. Build flags (free, do it first, measure)

`src/Makefile` currently uses `-O2` for all targets.

- `-O3` on the native targets — GCC does not enable full loop vectorization at `-O2`.
- `-msimd128` on the emscripten target; SIMD is otherwise unavailable in wasm.
- Do **not** put `-march=native` in the shipped shared-library targets. Add an opt-in
  `NATIVE_CFLAGS` (e.g. `-march=x86-64-v3`) for packagers who want AVX2.

Expect little from this step on today's code — no loop vectorizes — but it is the
baseline the later steps need, and it costs nothing.

### 2. Arena + struct-of-arrays mirror (the actual win)

Keep the public API exactly as it is. Inside `layout()`, build a transient flat mirror of
the caller's `tree_t` graph, run the algorithm on the mirror, write coordinates back.

- One `aligned_alloc(64, …)` per array, or a single arena carved into `restrict`ed
  spans. No per-node allocation on the hot path.
- **Level (BFS) order**, which makes the children of a node a *contiguous index range* —
  every "for each child" loop in the file becomes a linear scan, and every node of a
  given depth is adjacent.
- Arrays: `w, h, margin, x, y, prelim, mod, shift, change, msel, mser, bot` as
  `double[]`; `parent, firstChild, childCount, tl, tr, el, er` as `int32_t[]` index
  arrays (indices, not pointers — half the width, and they vectorize).
- `layout_api` builds this mirror **directly** from its `wh`/`whg`/`children` arrays,
  skipping `reifyflatchunks` and the pointer tree entirely. `reifyflatchunks` stays
  exported and working for existing callers.

Cost: two O(n) copies. Benefit: the whole algorithm stops chasing pointers. This is
where the multiples are, and it is what makes step 4 possible at all.

### 3. Replace recursion with level sweeps

With level order in hand, each recursive walk becomes an iteration over levels:

- `setupWalk` → forward sweep, level 0 → deepest.
- `firstWalk` → **reverse** sweep, deepest → 0. Nodes at the same level own disjoint
  subtrees, so their `separate`/`positionRoot` work is independent — the arithmetic per
  node is unchanged, only the visit order across independent subtrees differs, so
  results stay bit-identical.
- `secondWalk` → forward sweep. Per level: `modsum[i] = modsum[parent[i]] + mod[i]`, then
  write coordinates, then run `addChildSpacing` for that level (it writes into the *next*
  level's `mod`, so the ordering constraint is preserved). Keep `addChildSpacing`'s scan
  scalar — it is the one place where vectorizing would change results.
- `thirdWalk` → a single flat pass.

This removes the stack-overflow ceiling entirely and is worth doing on its own merits.
Keep `separate` scalar and byte-for-byte equivalent; only its data access changes.

### 4. SIMD the flat sweeps

Only now is there anything to vectorize. Use **portable GCC/Clang vector extensions**
(`typedef double v2df __attribute__((vector_size(16)));`) rather than per-ISA
intrinsics: one code path that lowers to NEON, SSE2 and wasm `simd128`, no runtime
dispatch, no `#ifdef` thicket. Add `restrict` and `__builtin_assume_aligned` so the
autovectorizer can also do the job unaided, and verify with `-Rpass=loop-vectorize` that
the target loops now report as vectorized.

Vectorize, in this order: `thirdWalk` translate; `bot[]` precomputation; the
`secondWalk` coordinate finalize; the `setupWalk` depth fill; `flat_xy_into`. Leave
`separate`, `addChildSpacing` and the chain routines scalar.

Honest expectation: these sweeps are a minority of runtime and the lane count is 2 on the
dominant targets, so this step is worth single-digit percent — after step 2 has already
made the sweeps cache-friendly.

### 5. Optional, only if the benchmark justifies it

- Multi-thread the reverse `firstWalk` sweep — nodes within a level are independent. This
  is real parallelism (cores, not lanes) and likely dwarfs step 4, but it adds a
  threading dependency to a currently dependency-free library.
- Hot-field packing for `separate`: a contour-walk-local struct of `{bot, w, prelim, mod,
  margin, next}` fits one cache line.

## Verification

Every step must satisfy all of:

1. `make test` — golden coordinates byte-identical to the frozen `test/golden.txt`.
2. Zero overlapping node pairs across the 57 600-tree sweep, both orientations.
3. ASan + UBSan + LeakSanitizer clean on the sweep.
4. `clang -std=c11 -O2 -Wall -Wextra -Wshadow -Wmissing-prototypes` with no warnings, and
   the same for `gcc`.
5. `make emscripten` still exports `layout_api`/`malloc`/`free`, and the Node smoke test
   returns coordinates matching the native build.
6. `make bench` shows a documented improvement; record ns/node per step in this file so
   the claims stay honest.
7. Deep-chain case that segfaults today (100k depth) completes after step 3.

## Non-goals

- Cross-tree SIMD batching — ruled out by the one-large-tree workload.
- `float` instead of `double` — would double the lane count but breaks the API and precision.
- `-ffast-math` / reassociated scans — breaks bit-exactness.
- Any change to `tree_t`, `layout`, or `layout_api` signatures.
