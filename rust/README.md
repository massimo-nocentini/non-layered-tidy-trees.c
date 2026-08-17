# A Rust port

`src/non-layered-tidy-trees.c` translated into plain, `unsafe`-free Rust, with the
`test/` suite translated along with it.

```sh
cargo test                        # the whole suite, ~8 s (the overlap sweep dominates)
cargo test --release              # the same, ~5 s
NLTT_TRIALS=25 cargo test         # a quicker sweep over the same shapes
cargo doc --open                  # the API, with the C names alongside
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
| `overlap --dump` | `src/bin/dump.rs` | the differential oracle below |
| — | `src/lib.rs` unit tests | `max_bottom` / `max_bottom_between`, which the C suite does not cover; the expected values are the ones the C build prints |

`NLTT_TRIALS` sets the trees per shape (default 300, the C default) and `NLTT_DEPTH` the
depth of the chain (default 10 000).

## The differential oracle

`test/overlap.c --dump` prints the corpus instead of checking it. `src/bin/dump.rs` does
the same, and `--check` compares against a dump instead of diffing it — C's `%.17g` and
Rust's shortest round-trip formatting spell the same `f64` differently, but both round-trip,
so parsing makes the comparison exact:

```sh
make -C ../test overlap
../test/overlap --dump 300 > golden.txt              # 62 MB, the C coordinates
cargo run --release --bin dump -- --check golden.txt 300
# PASS  9946368 coordinates over 115200 trees, identical to golden.txt
```

Use a smaller trial count (`--dump 10`, `--check golden.txt 10`) for a couple of MB.
