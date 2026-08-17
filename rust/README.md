# A Rust port

`src/non-layered-tidy-trees.c` translated into plain Rust, with the `test/` suite translated
along with it, plus a second implementation of the same algorithm over struct-of-arrays
sweeps. The crate has no dependencies and no `unsafe` at all unless `--features simd` is
asked for, which adds the AVX2 kernels and nothing else.

```sh
cargo test                        # the whole suite, ~8 s (the overlap sweep dominates)
cargo test --release              # the same, ~5 s
NLTT_TRIALS=25 cargo test         # a quicker sweep over the same shapes
cargo doc --open                  # the API, with the C names alongside

cargo run --release --features simd --bin bench    # the numbers below
make -C ../test bench                              # the C rows of the same table
```

The port is intended to be *numerically* identical, not merely equivalent: the walks, the
sibling chain, the threading and the one behavioural fix in `separate` follow the C line by
line, in the same order, so the two produce bit-identical coordinates. That claim is
checked rather than asserted — see [the differential oracle](#the-differential-oracle),
which agrees on all 9 946 368 coordinates of the 115 200-tree corpus.

## The shape of the API

```rust
use non_layered_tidy_trees::{layout, Arena, LayoutInput};

let mut arena = Arena::new();
let root = arena.add_node(1, 30.0, 10.0, 0.0, false); // idx, w, h, margin, isdummy
let a = arena.add_node(2, 20.0, 10.0, 0.0, false);
let b = arena.add_node(3, 20.0, 30.0, 0.0, false);
arena.set_children(root, &[a, b]);

layout(&mut arena, &LayoutInput::new(root));

println!("{} {}", arena[root].x, arena[root].y);
```

| C | Rust |
|---|---|
| `tree_t` | `Node`, held by an `Arena` and named by a `NodeId` |
| `init_tree` / `free_tree` | `Arena::add_node` / dropping the arena |
| `t->c[i] = child` | `arena.set_children(t, &[child, ..])`, `arena.push_child(t, child)` |
| `treeinput_t` | `LayoutInput` (the data) plus `Callbacks` (the two hooks) |
| `layout(&input)` | `layout(&mut arena, &input)`, `layout_with(.., &mut cb)` |
| `walkcb` + `walkud` | `Callbacks::walk`, a `FnMut(&mut Arena, NodeId)` |
| `cpairscb` + `cpairsud` | `Callbacks::contour_pairs`, a `FnMut(&Arena, NodeId, NodeId, f64)` |
| `bottom(t, vertically)` | `arena.bottom(t, vertically)` |
| `layout_api`, `reifyflatchunks`, `flat_xy_into`, `flat_xywh_into` | same names, snake case |
| `maxbottom`, `maxbottombetween`, `fringemaxbottom_t` | `max_bottom`, `max_bottom_between`, `FringeMaxBottom` |
| `test/treegen.h` | the `treegen` module of the library |
| — | `flat::layout_flat`, `flat::Engine`, `flat::layout_api_flat`: the same algorithm over sweeps |

## What is not a literal translation

Everything in the "Differences with respect to the Java implementation" section of the
[root README](../README.md) still holds — horizontal layouting, centred coordinates, the
derived depth axis, per-node margins, the origin offset, the callbacks, the repeatability
of `layout`, the flat entry point. What changed *again*, going from C to Rust, is the shape
of the data rather than the arithmetic:

**Ids instead of pointers.** The threads (`tl`/`tr`), the extreme nodes (`el`/`er`) and the
parent links point at arbitrary nodes elsewhere in the tree, which is exactly the aliasing
that a tree of `Box`es cannot express. Nodes therefore live in an `Arena` and refer to each
other by index, and there is no `unsafe` in the crate.

**No `free_tree`.** Dropping the arena frees every node at once, without recursion — one
fewer stack-bounded walk than the C has.

**The sibling chain is a `Vec` used as a stack.** `update_chain` pops instead of freeing
links, and `separate` walks *down* the stack where the C follows `ih->nxt`. Running off the
end of the chain is a panic here rather than a null dereference; it cannot happen, for the
reason `tests/regression.rs::asymmetric_contours` pins down.

**Callbacks are closures, and they moved out of the input.** `void *walkud` and
`void *cpairsud` are captured state, so they have no counterpart; the hooks live in
`Callbacks` and `LayoutInput` is left as plain data that `layout` takes by shared
reference. The C test "`layout` does not write into its `treeinput_t`" therefore becomes a
property of the signature — it is still checked, in `tests/properties.rs::repeatability`.

**`vertically` and `centeredxy` are `bool`.** The C takes any nonzero `int`, and
`test/properties.c` checks that `2` and `-1` behave like `1`. That case cannot fail here
and has no port; what survives of it is the check that the flag reaches every node.

**`level` and `childno` are `usize`.** The C initializes them to `-1` and fills them in
during `setupWalk`; here they start at `0`. The only node that keeps the sentinel in C is
the root, whose `childno` `maxbottombetween` never reads, so the behaviour is the same.

**The generator is part of the library.** `test/treegen.h` becomes the `treegen` module,
because both the tests and the differential oracle need the very same corpus. Its PRNG is
bit-compatible with `tg_rand`, seeds included.

## The tests

Each C test file became one Rust test file, with the `PASS`/`FAIL` lines replaced by
assertions.

| C | Rust | What it pins down |
|---|---|---|
| `test/overlap.c` | `tests/overlap.rs` | no two node boxes overlap, over 4 regimes × depths 1–6 × 2–5 children × both orientations × centred/corner |
| `test/regression.c` | `tests/regression.rs` | the `first`-flag overlap in `separate`, and the shallow-left/deep-right contour shape |
| `test/properties.c` | `tests/properties.rs` | normalization, `x`/`y` as offsets, repeatability, centring as one rigid translation |
| `test/callbacks.c` | `tests/callbacks.rs` | the contour trace (compared against the one the C build prints), one `walk` call per node, a mutating callback that does not perturb normalization |
| `test/flatapi.c` | `tests/flatapi.rs` | `layout_api` and the three parallel array layouts |
| `test/depth.c` | `tests/depth.rs` | a chain of 10 000 levels; the walks run on a thread with a 64 MiB stack, since `cargo test` gives each test 2 MiB |
| — | `tests/flat.rs` | the sweeps against the recursion, bit for bit: 1 152 trees with their bookkeeping, a reused `Engine`, a 300 000-level chain, and `layout_api_flat` against `layout_api` |
| `overlap --dump` | `src/bin/dump.rs` | the differential oracle below |
| — | `src/lib.rs` unit tests | `max_bottom` / `max_bottom_between`, which the C suite does not cover; the expected values are the ones the C build prints |

`NLTT_TRIALS` sets the trees per shape (default 300, the C default) and `NLTT_DEPTH` the
depth of the chain (default 10 000). `cargo test --features simd` runs the same suite with
the vectorized kernels in place of the scalar ones; it has to pass identically.

## Benchmarks

One tree of `n` nodes, laid out repeatedly; the tree is built outside the timed region and
the first layout is a warm-up, so what is measured is `layout` itself. `test/bench.c` and
`src/bin/bench.rs` build the *same* trees — the two generators draw from the same PRNG in
the same order — and both print a checksum that is the XOR of every coordinate's bit
pattern. Every row below carries the same checksum for a given size: these are all the same
drawing, computed five different ways.

Xeon Gold 6238R at 2.2 GHz, clang 18 `-O2` (gcc 13 is within 5%), rustc 1.97 release,
ns/node, best of 7, averaged over both orientations. `-r` marks the rows that reuse their
mirror across calls.

**`layout`, over a tree already in memory:**

| impl | 1 000 | 10 000 | 100 000 | 1 000 000 |
|---|---:|---:|---:|---:|
| `c` | 56.5 | 69.8 | 70.3 | **106.4** |
| `rust-rec` (the port) | 69.3 | 83.1 | 84.8 | 135.5 |
| `rust-flat` | 67.1 | 86.0 | 107.7 | 210.1 |
| `rust-flat-r` | 56.6 | 81.5 | 101.2 | 171.4 |
| `rust-simd-r` | 61.9 | 87.0 | 99.9 | 171.4 |

**`layout_api`, arrays in and arrays out:**

| impl | 1 000 | 10 000 | 100 000 | 1 000 000 |
|---|---:|---:|---:|---:|
| `c-api` | 254.3 | 283.2 | 367.9 | 548.0 |
| `rust-api` (the port) | 235.9 | 264.6 | 497.2 | 657.1 |
| `rust-api-flat` | 130.5 | 143.6 | 166.2 | 356.7 |
| `rust-api-flat-r` | 120.0 | 137.0 | 152.5 | **256.9** |
| `rust-api-simd-r` | 127.2 | 142.5 | 157.1 | 261.4 |

### What the numbers say

**SIMD is worth nothing here, as `simd-plan.md` predicted.** `rust-simd-r` and
`rust-flat-r` are the same speed to within noise, in both tables, at every size. The phase
breakdown says why (`bench --phases`, 1 M nodes, µs):

| | build | setup | first | second | third | write |
|---|---:|---:|---:|---:|---:|---:|
| scalar | 67 895 | 5 690 | 57 536 | 9 421 | 1 147 | 26 956 |
| AVX2 | 67 468 | 6 258 | 56 302 | 9 033 | 1 075 | 27 215 |

The three vectorizable sweeps are `setup`, `second` and `third`: 16.3 ms of 168.6 ms, under
10% of the runtime — and they are streaming over arrays at roughly 13 GB/s, which is memory
bandwidth, not arithmetic. Four lanes of a bandwidth-bound loop are still one memory system.
`first` — the contour walk, the algorithm proper — is a third of the time on its own and
cannot be vectorized at any width.

**The restructuring is what pays, and only when the input is already flat.** Over an
`Arena` the mirror is a loss: `build` plus `write` is 56% of a flat layout, which is more
than the pointer chasing it removes, because a Rust `Arena` is already one contiguous
`Vec<Node>` — the locality that `simd-plan.md`'s step 2 was chasing in the C is something
the port had from the start. Over `layout_api` there is no tree to mirror in the first
place, the 2n nodes and their child lists never get allocated, and the same code is **2.1×
faster than the C** and 2.6× faster than the pointer-based Rust.

**The C is still the one to beat for `layout`.** The recursive port is ~25% behind it, which
is where bounds-checked indexing in the contour walk shows up; the C `tree_t` reaches a
sibling through one pointer where `Arena` indexes a `Vec` and then a `Vec` of children.

## The differential oracle

`test/overlap.c --dump` prints the corpus instead of checking it. `src/bin/dump.rs` does
the same, and `--check` compares against a dump instead of diffing it — C's `%.17g` and
Rust's shortest round-trip formatting spell the same `f64` differently, but both round-trip,
so parsing makes the comparison exact:

```sh
make -C ../test overlap
../test/overlap --dump 300 > golden.txt              # 62 MB, the C coordinates
cargo run --release --bin dump -- --check golden.txt 300
# PASS  9946368 coordinates over 115200 trees, recursive identical to golden.txt

cargo run --release --features simd --bin dump -- --flat --check golden.txt 300
cargo run --release --features simd --bin dump -- --simd --check golden.txt 300
```

All three paths pass: the sweeps and the AVX2 kernels reproduce the C build's coordinates
exactly, not approximately.

Use a smaller trial count (`--dump 10`, `--check golden.txt 10`) for a couple of MB.
