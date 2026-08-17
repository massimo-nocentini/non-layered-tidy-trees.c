//! The two hooks of `treeinput_t`, which live in `Callbacks` here.
//!
//! The port of `test/callbacks.c`.
//!
//! `contour_pairs` observes every pair of contour nodes `separate` compares, which is the
//! only window onto the contour walk from outside the library; the expected trace for a
//! hand-built tree is spelled out below, and it is the one the C build prints.
//!
//! `walk` observes every node during the second walk. It is handed the arena, so it may
//! move the node it is given, and the minimum used for normalization is read *before* it
//! runs -- so a callback that moves a node does not feed its own edit back into the
//! normalization. That is deliberate; this test documents it.

use non_layered_tidy_trees::treegen::{self, Regime};
use non_layered_tidy_trees::{layout, layout_with, Arena, Callbacks, LayoutInput, NodeId};

#[test]
fn contour_pairs() {
    /* root over a shallow-left/deep-right subtree and a deep chain */
    let mut arena = Arena::new();
    let root = arena.add_node(1, 10.0, 10.0, 0.0, false);
    let a = arena.add_node(2, 10.0, 10.0, 0.0, false);
    let shallow = arena.add_node(4, 10.0, 10.0, 0.0, false);
    let deep = treegen::chain(&mut arena, 5, 3, 10.0, 10.0);
    arena.set_children(a, &[shallow, deep]);
    let right = treegen::chain(&mut arena, 8, 4, 10.0, 10.0);
    arena.set_children(root, &[a, right]);

    let mut trace: Vec<String> = Vec::new();

    {
        let mut record = |a: &Arena, sr: NodeId, cl: NodeId, dist: f64| {
            trace.push(format!(
                "#{} (bottom {}) against #{} (bottom {}), dist {}",
                a[sr].idx,
                a.bottom(sr, true),
                a[cl].idx,
                a.bottom(cl, true),
                dist
            ));
        };
        let mut cb = Callbacks {
            contour_pairs: Some(&mut record),
            ..Default::default()
        };
        layout_with(&mut arena, &LayoutInput::new(root), &mut cb);
    }

    /* one pair inside A, then four while separating A from the right subtree */
    assert_eq!(
        trace,
        [
            "#4 (bottom 30) against #5 (bottom 30), dist 10",
            "#2 (bottom 20) against #8 (bottom 20), dist 15",
            "#5 (bottom 30) against #9 (bottom 30), dist 5",
            "#6 (bottom 40) against #10 (bottom 40), dist 0",
            "#7 (bottom 50) against #11 (bottom 50), dist 0",
        ],
        "the contour walk visits other pairs than the C build does"
    );
}

#[test]
fn walk_callback() {
    let (mut arena, root) = treegen::build(5, 4, 3, Regime::Varied);
    let n = treegen::collect(&arena, root).len();

    let mut visited = 0;
    {
        let mut counter = |_: &mut Arena, _: NodeId| visited += 1;
        let mut cb = Callbacks {
            walk: Some(&mut counter),
            ..Default::default()
        };
        layout_with(&mut arena, &LayoutInput::new(root), &mut cb);
    }

    assert_eq!(visited, n, "`walk` is not called exactly once per node");
}

#[test]
fn a_mutating_walk_callback_does_not_perturb_normalization() {
    /* three leaves under a root: children at 0, 10, 20 and the root centred at 10 */
    let mut arena = Arena::new();
    let root = arena.add_node(1, 10.0, 10.0, 0.0, false);
    let kids: Vec<NodeId> = (0..3)
        .map(|i| arena.add_node(i + 2, 10.0, 10.0, 0.0, false))
        .collect();
    arena.set_children(root, &kids);

    {
        // a callback that moves the node it was handed
        let mut mover = |a: &mut Arena, t: NodeId| {
            if a[t].idx == 1 {
                a[t].x -= 1000.0;
            }
        };
        let mut cb = Callbacks {
            walk: Some(&mut mover),
            ..Default::default()
        };
        layout_with(&mut arena, &LayoutInput::new(root), &mut cb);
    }

    assert_eq!(
        (arena[kids[0]].x, arena[kids[1]].x, arena[kids[2]].x),
        (0.0, 10.0, 20.0),
        "a coordinate-mutating `walk` callback perturbed normalization"
    );
    // the root is centred at 10 and the callback took 1000 off it; normalization had
    // already read the minimum, which the leftmost leaf holds, so nothing compensates.
    assert_eq!(
        arena[root].x, -990.0,
        "the callback's own edit was not kept"
    );
}

#[test]
fn no_callbacks_is_the_same_layout() {
    let (mut arena, root) = treegen::build(5, 4, 3, Regime::Varied);
    let input = LayoutInput::new(root);

    layout(&mut arena, &input);
    let plain: Vec<(f64, f64)> = treegen::collect(&arena, root)
        .iter()
        .map(|&id| (arena[id].x, arena[id].y))
        .collect();

    let mut seen = 0;
    {
        let mut counter = |_: &mut Arena, _: NodeId| seen += 1;
        let mut cb = Callbacks {
            walk: Some(&mut counter),
            ..Default::default()
        };
        layout_with(&mut arena, &input, &mut cb);
    }

    let observed: Vec<(f64, f64)> = treegen::collect(&arena, root)
        .iter()
        .map(|&id| (arena[id].x, arena[id].y))
        .collect();

    assert_eq!(plain, observed, "observing the walks changed the drawing");
    assert_eq!(seen, plain.len());
}
