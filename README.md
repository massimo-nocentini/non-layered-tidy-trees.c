# non-layered-tidy-trees.c

[![Continuous integration](https://github.com/massimo-nocentini/non-layered-tidy-trees.c/actions/workflows/main.yml/badge.svg)](https://github.com/massimo-nocentini/non-layered-tidy-trees.c/actions/workflows/main.yml)

This repository contains a translation in C of the algorithm described in the publication

>van der Ploeg, A. (2014), Drawing non-layered tidy trees in linear time, Softw. Pract. Exper., 44, pages 1467– 1484, doi: [10.1002/spe.2213](https://doi.org/10.1002/spe.2213)

which is implemented in the companion https://github.com/cwi-swat/non-layered-tidy-trees repository. The result is a shared library that can be linked in arbitrary projects.

According to the issue https://github.com/Klortho/d3-flextree/issues/1, we take the same fix as done in https://github.com/Klortho/d3-flextree/blob/af196220927218bbe7ac6cad8e059f56430befb6/src/flextree.js#L242

We provide the [`src/Makefile`](https://github.com/massimo-nocentini/non-layered-tidy-trees.c/blob/master/src/Makefile) to compile and install the artifacts in the usual Unix locations for headers and libraries, and a [`test/`](test) suite that can be run with `make -C test check`.

A port in plain, `unsafe`-free Rust lives in [`rust/`](rust), together with the same test suite. It is numerically identical, not merely equivalent: `cargo run --bin dump -- --check` compares it against a dump of the C build over the whole corpus of [`test/overlap.c`](test/overlap.c), and the coordinates agree bit for bit.

## Differences with respect to the Java implementation

The core of the algorithm is a faithful translation. `firstWalk`, `separate`, the
threading in `setLeftThread`/`setRightThread`, the sibling chain, `moveSubtree` with
`distributeExtra` folded into it, `addChildSpacing` and `positionRoot` all follow the Java
line by line; `positionRoot` is written as `(… + (w_last - w)) / 2` where the Java has
`(… + w_last) / 2 - w / 2`, which is the same expression rearranged.

Everything below is what is *not* a literal translation.

### Layouting

**Horizontal layouting.** The `vertically` flag transposes the drawing. The paper's
algorithm works along one *breadth* axis and one *depth* axis; the Java pins them to `x`
and `y`, whereas here the pair is swapped wholesale — in `separate` the extent of a node
becomes its height rather than its width, `positionRoot` centres over heights, and
`bottom` measures along `x`. The Java `bottom` is simply `t.y + t.h`.

**Centered coordinates.** With `centeredxy`, `x` and `y` denote the middle of a node's box
instead of its top left corner. The Java has no such notion: it works in left edges and
widths throughout. Note that this interacts with the normalization below — the smallest
*coordinate* is moved to the origin, so in centred mode the leftmost *edge* ends up half a
node further left. The drawing itself is unaffected; it is translated as a whole.

**The depth axis is derived, not supplied.** The Java `Tree` constructor takes `y` per
node (`Tree(double w, double h, double y, Tree... c)`) and never modifies it: the caller
owns the depth axis entirely, and the algorithm only ever computes `x`. Here `setupWalk`
places every node directly below the bottom of its parent, so the depth axis follows from
the node heights. Spacing between levels is obtained by inserting an invisible node above
each node — which is exactly what the `whg` array of `layout_api` is for.

**Per-node margins.** `tree_t` carries a `margin` that widens the separation between
subtrees; the Java `Tree` has no such field and `dist` is exactly
`(mssr + sr.prelim + sr.w) - (mscl + cl.prelim)`. The margin is taken from the *left* node
of each contour pair, so it is one-sided by construction.

**An origin offset.** The Java normalizes the minimum coordinate to zero
(`if (minX != 0) thirdWalk(t, -minX)`). We do the same, and `input->x` and `input->y` then
offset that origin, so the drawing can be placed anywhere without a second pass over it.

**Callbacks.** `walkcb` observes every node as `secondWalk` finalizes it, and `cpairscb`
observes every pair of contour nodes `separate` compares — the only external window onto
the contour walk. The Java has no hooks.

**`layout` may be applied more than once to the same tree.** `setupWalk` clears the
accumulators *and the threads* that the walks leave behind. In Java each layout starts
from a freshly constructed tree, so the question never arises.

### C-specific concerns

**Ownership.** `init_tree`/`free_tree` replace the garbage collector, and the sibling chain
frees its own links as they are popped in `update_chain` — the Java simply drops the
reference.

**A shared library, not a class.** Everything that is not part of the published surface has
internal linkage, so a program linking this library does not inherit symbols named
`separate` or `firstWalk`; the header carries an include guard.

**A flat entry point.** `layout_api` takes the tree as parallel arrays rather than as
pointers, which is what makes the library usable from FFI bindings and from the
WebAssembly build. There is no Java counterpart. The array layouts are documented in
[`test/flatapi.c`](test/flatapi.c).

**Recursion depth.** Every walk recurses once per level, so the deepest tree that can be
laid out is bounded by the thread stack rather than by memory: on an 8 MB main-thread
stack, 60 000 levels are fine and 100 000 are not. The WebAssembly build defaults to a
64 KiB stack, which fails far earlier, and `layout_api` doubles the effective depth because
of the gap nodes.

### One behavioural fix

In `separate`, the guard that lets a subtree be pulled *closer* to its left sibling —
the [d3-flextree](https://github.com/Klortho/d3-flextree/issues/1) fix cited above — must
apply to the topmost pair of contour nodes only. The flag that tracks this is therefore
cleared on **every** iteration of the contour walk, as d3-flextree does. Clearing it only
on the iterations that actually move something instead makes it mean "no move has happened
yet", and the two readings part company as soon as the leading contour pairs are exactly
touching (`dist == 0.0`), which is the common case for uniformly sized nodes. The
correction then fires deep in the contour walk, where it is no longer sound:

```
            root (30x10)
           /            \
    A (20x10)            B (20x30)
        |               /        \
    A1 (10x20)   B1 (30x10)   B2 (20x10)
```

`A.prelim` is `-5` and `B.prelim` is `+15`, so the first contour pair has `dist == 0`: the
subtrees already touch, nothing moves. The second pair has `dist == -5`, and applying the
correction there drags `B` five units into `A`, which then overlap. Over the corpus in
[`test/overlap.c`](test/overlap.c) this produces 330 pairs of overlapping nodes across
115 200 trees, against none when the flag falls on every iteration. The case is pinned down
in [`test/regression.c`](test/regression.c).
