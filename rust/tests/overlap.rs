//! The headline property of a tidy drawing: no two node boxes overlap.
//!
//! The port of `test/overlap.c`. Sweeps size regimes x depths x branching factors x
//! orientations x centeredxy and checks every pair of nodes in every tree.
//!
//! The default is the C one, 300 trees per shape and 115 200 trees in all; `NLTT_TRIALS`
//! changes it in either direction:
//!
//! ```sh
//! NLTT_TRIALS=25 cargo test --test overlap     # a quicker sweep
//! ```
//!
//! The `--dump` half of the C program, which turns the same corpus into a differential
//! oracle, lives in `src/bin/dump.rs`.

use non_layered_tidy_trees::treegen::{self, REGIMES};
use non_layered_tidy_trees::{layout, LayoutInput};

fn trials() -> usize {
    std::env::var("NLTT_TRIALS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300)
}

#[test]
fn no_pair_of_boxes_overlaps() {
    let trials = trials();

    let mut collisions = 0;
    let mut trees = 0;
    let mut reported = String::new();

    for regime in REGIMES {
        for maxdepth in 1..=6 {
            for maxkids in 2..=5 {
                for vert in [false, true] {
                    for cent in [false, true] {
                        for k in 0..trials {
                            let seed = 1000003 * k as u64
                                + 7 * (regime.index() * 100 + maxdepth * 10 + maxkids) as u64;

                            let (mut arena, root) = treegen::build(seed, maxdepth, maxkids, regime);

                            let input = LayoutInput {
                                root,
                                vertically: vert,
                                centeredxy: cent,
                                x: 0.0,
                                y: 0.0,
                            };
                            layout(&mut arena, &input);

                            let nodes = treegen::collect(&arena, root);
                            trees += 1;

                            for i in 0..nodes.len() {
                                for j in i + 1..nodes.len() {
                                    if treegen::overlap(&arena, nodes[i], nodes[j]) {
                                        collisions += 1;
                                        if collisions <= 10 {
                                            let (a, b) = (&arena[nodes[i]], &arena[nodes[j]]);
                                            let (al, at) = (
                                                treegen::left(&arena, nodes[i]),
                                                treegen::top(&arena, nodes[i]),
                                            );
                                            let (bl, bt) = (
                                                treegen::left(&arena, nodes[j]),
                                                treegen::top(&arena, nodes[j]),
                                            );
                                            reported.push_str(&format!(
                                                "\n  overlap: regime={} depth={maxdepth} kids={maxkids} \
                                                 vert={vert} cent={cent} k={k} seed={seed} \
                                                 -- #{} [{al},{}]x[{at},{}] vs #{} [{bl},{}]x[{bt},{}]",
                                                regime.index(),
                                                a.idx, al + a.w, at + a.h,
                                                b.idx, bl + b.w, bt + b.h,
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    assert_eq!(
        collisions, 0,
        "{collisions} overlapping pairs over {trees} trees{reported}"
    );
}
