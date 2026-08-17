//! Recursion depth.
//!
//! The port of `test/depth.c`. `setup_walk`, `first_walk`, `second_walk` and `third_walk`
//! all recurse once per level, so the deepest tree that can be laid out is bounded by the
//! thread's stack rather than by memory. Dropping the arena, unlike `free_tree`, does not
//! recurse at all.
//!
//! `cargo test` gives each test thread 2 MiB by default, so the walk runs on a thread with
//! a stack of its own -- 64 MiB here, which is comfortable for far more than the 10 000
//! levels tested. Raise the depth with `NLTT_DEPTH` to probe further:
//!
//! ```sh
//! NLTT_DEPTH=100000 cargo test --release --test depth
//! ```

use non_layered_tidy_trees::treegen;
use non_layered_tidy_trees::{layout, Arena, LayoutInput};

const STACK: usize = 64 << 20;

#[test]
fn a_deep_chain_lays_out() {
    let depth: usize = std::env::var("NLTT_DEPTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10000);

    let worker = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || {
            let mut arena = Arena::new();
            let root = treegen::chain(&mut arena, 1, depth, 10.0, 10.0);

            layout(&mut arena, &LayoutInput::new(root));

            /* every level sits directly below the previous one, so the last node is at 10*(d-1) */
            let mut last = root;
            while let Some(&c) = arena.children(last).first() {
                last = c;
            }

            (arena[last].y, arena[last].level)
        })
        .expect("spawning the deep-walk thread");

    let (y, level) = worker.join().expect("laying out a deep chain");

    assert_eq!(
        y,
        10.0 * (depth - 1) as f64,
        "the deepest node of a chain of {depth} levels is at y={y}"
    );
    assert_eq!(
        level,
        depth - 1,
        "the deepest node is not at level {}",
        depth - 1
    );
}
