//! `layout_api`, the flat entry point the C exposes to FFI and WebAssembly bindings.
//!
//! The port of `test/flatapi.c`. Its arguments are three parallel arrays and, in the C, no
//! documentation, so the layouts are spelled out here as well:
//!
//! * `wh` — `3n` values, `[w_0..w_n-1, h_0..h_n-1, margin_0..margin_n-1]`; on return the
//!   first `2n` are overwritten with `[x_0.., y_0..]`
//! * `whg` — `2n` values, the width and height of the invisible "gap" node inserted above
//!   every node, which is what produces the spacing between levels
//! * `children` — `n` counts followed by the adjacency, one-based, grouped by parent
//! * `rooti` — the one-based index of the root
//!
//! The C test asks to be run under LeakSanitizer, since `layout_api` owns everything it
//! allocates. Here the arena is a local that drops on return, so the property is the type
//! system's rather than the test's.

use non_layered_tidy_trees::{flat_xywh_into, layout, layout_api, reify_flat_chunks, LayoutInput};

fn close_enough(a: f64, b: f64) -> bool {
    (a - b) < 1e-9 && (b - a) < 1e-9
}

/// a root with two leaves, every node 10x10, five units of vertical gap
fn two_leaves() -> (usize, [f64; 9], [f64; 6], [usize; 5]) {
    (
        3,
        [
            10.0, 10.0, 10.0, /* h */ 10.0, 10.0, 10.0, /* margin */ 0.0, 0.0, 0.0,
        ],
        [10.0, 10.0, 10.0, /* h */ 5.0, 5.0, 5.0],
        [2, 0, 0, /* adjacency */ 2, 3],
    )
}

#[test]
fn a_root_with_two_leaves() {
    let (n, mut wh, whg, children) = two_leaves();

    layout_api(n, &mut wh, &whg, &children, 1, true, false, 0.0, 0.0);

    assert!(
        close_enough(wh[0], 5.0) && close_enough(wh[3], 5.0),
        "the root does not sit between its children, below the gap: ({}, {})",
        wh[0],
        wh[3]
    );
    assert!(
        close_enough(wh[1], 0.0) && close_enough(wh[2], 10.0),
        "the leaves are not side by side: {} and {}",
        wh[1],
        wh[2]
    );
    assert!(
        close_enough(wh[4], 20.0) && close_enough(wh[5], 20.0),
        "the leaves do not share a level: {} and {}",
        wh[4],
        wh[5]
    );
}

#[test]
fn nothing_is_retained_between_calls() {
    let (n, mut wh, whg, children) = two_leaves();
    layout_api(n, &mut wh, &whg, &children, 1, true, false, 0.0, 0.0);

    /* the same tree again, to prove nothing is retained between calls */
    let (n2, mut wh2, whg2, children2) = two_leaves();
    layout_api(n2, &mut wh2, &whg2, &children2, 1, true, false, 0.0, 0.0);

    assert_eq!(
        &wh[..2 * n],
        &wh2[..2 * n],
        "a second call gives a different answer"
    );
}

#[test]
fn horizontal_centred_and_offset() {
    let n = 3;
    let mut wh = [10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 0.0, 0.0, 0.0];
    let whg = [5.0, 5.0, 5.0, 10.0, 10.0, 10.0];
    let children = [2, 0, 0, 2, 3];

    layout_api(n, &mut wh, &whg, &children, 1, false, true, 100.0, 50.0);

    assert!(
        wh[0] >= 100.0 && wh[3] >= 50.0,
        "the offset is not honoured in horizontal mode: root at ({}, {})",
        wh[0],
        wh[3]
    );
}

/// The gap nodes are what `layout_api` adds on top of `layout`; laying out the reified
/// arena by hand has to give the very same coordinates.
#[test]
fn reified_by_hand() {
    let (n, wh, whg, children) = two_leaves();
    let expected = {
        let mut wh = wh;
        layout_api(n, &mut wh, &whg, &children, 1, true, false, 0.0, 0.0);
        wh
    };

    let (mut arena, nodes) = reify_flat_chunks(n, &wh, &whg, &children);
    let root = nodes[n]; // the gap node above node 1
    layout(&mut arena, &LayoutInput::new(root));

    /* `flat_xywh_into` writes 4 values per node, indexed by `idx - 1` */
    let mut xywh = vec![0.0; 4 * 2 * n];
    flat_xywh_into(&arena, root, &mut xywh);

    for i in 0..n {
        assert_eq!(xywh[i * 4], expected[i], "node {} moved along x", i + 1);
        assert_eq!(
            xywh[i * 4 + 1],
            expected[i + n],
            "node {} moved along y",
            i + 1
        );
        assert_eq!(xywh[i * 4 + 2], wh[i], "node {} changed width", i + 1);
        assert_eq!(xywh[i * 4 + 3], wh[i + n], "node {} changed height", i + 1);
    }
}
