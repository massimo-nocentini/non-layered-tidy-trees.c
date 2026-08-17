# Tests

Each test is a single C file linked directly against `../src/non-layered-tidy-trees.c`,
so there is nothing to install and no framework to learn. Every program prints one
`PASS`/`FAIL` line per assertion and exits nonzero if anything failed.

```sh
make check            # build and run everything
make asan             # the same suite under AddressSanitizer + UBSan + LeakSanitizer
make check CC=gcc     # or any other compiler
make wasm             # after `make -C ../src emscripten`
```

## What each one covers

| File | What it pins down |
|---|---|
| `overlap.c` | The headline property: **no two node boxes overlap**. Sweeps 4 size regimes × depths 1–6 × 2–5 children × both orientations × centred/corner, and checks every pair of nodes in each of the 115 200 trees. |
| `regression.c` | Hand-built trees, small enough to follow on paper, that each caught a bug that was once live: the `first`-flag overlap in `separate`, and a shallow-left/deep-right contour shape that stresses the `ih` chain. |
| `properties.c` | Invariants over random trees: normalization to the origin, `input->x`/`input->y` as offsets, repeatability of `layout` on one tree and one `treeinput_t`, `vertically`/`centeredxy` as truth values rather than enumerations, and `centeredxy` preserving the drawing. |
| `callbacks.c` | `cpairscb` sees every contour pair `separate` compares (with the trace printed, since it is the only external window onto the contour walk); `walkcb` is called once per node with `walkud` intact, and a coordinate-mutating `walkcb` does not perturb normalization. |
| `flatapi.c` | `layout_api`, the entry point the FFI and WebAssembly bindings call. Documents the three parallel array layouts, which are otherwise undocumented, and checks that nothing is retained between calls. Run it under `make asan`: it must be leak clean. |
| `depth.c` | How deep a tree can be laid out. Defaults to a chain of 10 000 levels; pass a depth to probe further. |
| `wasm_smoke.js` | That the emscripten module actually exports `layout_api` and returns the same coordinates as the native build. Without an explicit `-sEXPORTED_FUNCTIONS`, `-O2` strips the whole library and the module loads happily while exporting nothing. |

`treegen.h` holds the shared bits: a deterministic PRNG (so a failure reproduces from its
seed alone), the random tree generator with its four size regimes, and the geometric
predicates. `TG_COARSE` matters more than it looks — sizes that are multiples of ten make
exactly-touching siblings common, and that is the regime under which the `first`-flag bug
appeared while everything else looked fine.

## The differential oracle

`overlap --dump` prints coordinates instead of checking them, which turns the same corpus
into a before/after comparison for refactoring:

```sh
make golden                     # freeze the current coordinates in golden.txt
# ... change the library ...
make check-golden               # byte-for-byte comparison
```

A behaviour-preserving refactoring leaves the dump byte for byte identical. A change that
translates the whole drawing — normalization, an offset — shows up as a constant
displacement shared by every node in a tree, which is worth checking for explicitly:
a *per-node* difference means the geometry itself moved, and that is a real change.

`golden.txt` is a local artifact, not committed: the full corpus is about 60 MB. Use
`make golden TRIALS=10` for a couple of MB, at the cost of coverage.
