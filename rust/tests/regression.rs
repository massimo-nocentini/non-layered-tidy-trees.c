//! Hand-built trees that pin down bugs which were actually shipped at some point.
//!
//! The port of `test/regression.c`. Each case is small enough to reason about on paper,
//! and each one failed before the corresponding fix landed.

use non_layered_tidy_trees::treegen;
use non_layered_tidy_trees::{layout, Arena, LayoutInput};

/// ```text
///              root (30x10)
///             /            \
///      A (20x10)            B (20x30)
///          |               /        \
///      A1 (10x20)   B1 (30x10)   B2 (20x10)
/// ```
///
/// `A.prelim` is -5 and `B.prelim` is +15, so the first contour pair (A against B) has
/// `dist == 0.0` exactly: the two subtrees already touch. The second pair (A1 against B)
/// has `dist == -5`. Applying the "pull the subtree closer" correction there -- which is
/// what happens when `separate` clears its `first` flag only on iterations that move
/// something -- drags B five units into A.
#[test]
fn first_flag_overlap() {
    let mut arena = Arena::new();

    let root = arena.add_node(1, 30.0, 10.0, 0.0, false);
    let a = arena.add_node(2, 20.0, 10.0, 0.0, false);
    let a1 = arena.add_node(3, 10.0, 20.0, 0.0, false);
    let b = arena.add_node(4, 20.0, 30.0, 0.0, false);
    let b1 = arena.add_node(5, 30.0, 10.0, 0.0, false);
    let b2 = arena.add_node(6, 20.0, 10.0, 0.0, false);

    arena.set_children(root, &[a, b]);
    arena.set_children(a, &[a1]);
    arena.set_children(b, &[b1, b2]);

    layout(&mut arena, &LayoutInput::new(root));

    assert!(
        !treegen::overlap(&arena, a, b),
        "sibling subtrees overlap (separate's `first` flag): \
         A at ({}, {}), B at ({}, {})",
        arena[a].x,
        arena[a].y,
        arena[b].x,
        arena[b].y
    );

    let nodes = treegen::collect(&arena, root);
    assert_eq!(
        treegen::min_breadth(&arena, &nodes, true),
        0.0,
        "the same tree is not normalized to x = 0"
    );
}

/// The first child's left contour is shallow while its right contour is deep, and the
/// second child is deep enough that the contour walk keeps going. This is the shape that
/// would run the sibling chain off its end if `first_walk` seeded it with a value that did
/// not dominate the subtree's depth -- it does not, because threading makes `bottom(el)`
/// and `bottom(er)` both equal to the deepest point of the subtree.
#[test]
fn asymmetric_contours() {
    let mut arena = Arena::new();

    let root = arena.add_node(1, 10.0, 10.0, 0.0, false);
    let a = arena.add_node(2, 10.0, 10.0, 0.0, false);
    let shallow = arena.add_node(4, 10.0, 10.0, 0.0, false); // shallow left contour
    let deep = treegen::chain(&mut arena, 5, 3, 10.0, 10.0); // deep right contour
    arena.set_children(a, &[shallow, deep]);

    let right = treegen::chain(&mut arena, 8, 4, 10.0, 10.0);
    arena.set_children(root, &[a, right]);

    // used to be able to dereference a NULL chain link
    layout(&mut arena, &LayoutInput::new(root));

    let nodes = treegen::collect(&arena, root);
    let mut overlapping = 0;
    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            if treegen::overlap(&arena, nodes[i], nodes[j]) {
                overlapping += 1;
            }
        }
    }

    assert_eq!(
        overlapping, 0,
        "asymmetric contours lay out with {overlapping} overlapping pairs"
    );
}
