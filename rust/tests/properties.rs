//! Invariants that hold for every input, checked over random trees.
//!
//! The port of `test/properties.c`:
//!
//! * **normalization** — the smallest breadth coordinate ends up at `input.x` / `input.y`,
//!   which is the counterpart of the reference's `thirdWalk(t, -minX)`
//! * **repeatability** — `layout` may be applied twice, to the same tree and through the
//!   same `LayoutInput`, without the result drifting or the input being written
//! * **centering** — `centeredxy` shifts coordinates to box centres without moving boxes
//!
//! The C also checks that `vertically` and `centeredxy` are truth values rather than
//! enumerations, i.e. that `2` and `-1` lay out exactly like `1`. Both are `bool` here, so
//! that case cannot fail and has no port; what remains of it is the check below that the
//! flag reaches every node.

use non_layered_tidy_trees::treegen::{self, Regime, REGIMES};
use non_layered_tidy_trees::{layout, LayoutInput};

fn close_enough(a: f64, b: f64) -> bool {
    (a - b) < 1e-9 && (b - a) < 1e-9
}

#[test]
fn normalization() {
    let mut bad = 0;
    let mut offbad = 0;
    let mut total = 0;
    let mut worst = 0.0f64;

    for regime in REGIMES {
        for vert in [false, true] {
            for cent in [false, true] {
                for k in 0..150u64 {
                    let (mut arena, root) = treegen::build(4242 + k, 4, 3, regime);
                    let input = LayoutInput {
                        root,
                        vertically: vert,
                        centeredxy: cent,
                        x: 0.0,
                        y: 0.0,
                    };
                    layout(&mut arena, &input);
                    let nodes = treegen::collect(&arena, root);

                    let m = treegen::min_breadth(&arena, &nodes, vert);
                    total += 1;
                    if !close_enough(m, 0.0) {
                        bad += 1;
                        if m < worst {
                            worst = m;
                        }
                    }

                    /* the same tree, asked to start somewhere else */
                    let (mut arena, root) = treegen::build(4242 + k, 4, 3, regime);
                    let off = LayoutInput {
                        root,
                        vertically: vert,
                        centeredxy: cent,
                        x: 100.0,
                        y: 7.0,
                    };
                    layout(&mut arena, &off);
                    let nodes = treegen::collect(&arena, root);
                    let expected = if vert { 100.0 } else { 7.0 };
                    if !close_enough(treegen::min_breadth(&arena, &nodes, vert), expected) {
                        offbad += 1;
                    }
                }
            }
        }
    }

    assert_eq!(
        bad, 0,
        "the drawing does not start at the origin in {bad} of {total} layouts \
         (worst deviation {worst})"
    );
    assert_eq!(
        offbad, 0,
        "`input.x` / `input.y` fail to offset the normalized origin in {offbad} of {total} layouts"
    );
}

#[test]
fn repeatability() {
    let (mut arena, root) = treegen::build(99, 4, 3, Regime::Varied);
    let input = LayoutInput::new(root);

    layout(&mut arena, &input);
    let nodes = treegen::collect(&arena, root);
    let first: Vec<(f64, f64)> = nodes.iter().map(|&id| (arena[id].x, arena[id].y)).collect();

    let before = input;

    layout(&mut arena, &input);
    layout(&mut arena, &input);

    let drift = nodes
        .iter()
        .zip(&first)
        .filter(|(&id, &(x, y))| arena[id].x != x || arena[id].y != y)
        .count();

    assert_eq!(
        drift,
        0,
        "repeated layouts of one tree moved {drift} of {} nodes",
        nodes.len()
    );
    assert_eq!(before, input, "`layout` wrote into its `LayoutInput`");
}

/// `centeredxy` must move the coordinates to the middle of each box without disturbing the
/// drawing. The boxes do not land in the same absolute place though: normalization works on
/// the coordinate it is given, so in centred mode it puts the leftmost *centre* at the
/// origin and the leftmost edge ends up half a node further left. The drawing is therefore
/// preserved up to one rigid translation along the breadth axis -- which is the property
/// worth pinning down, since a per-node discrepancy would be a real bug.
#[test]
fn centering() {
    let mut mismatch = 0;
    let mut depthmoved = 0;
    let mut unflagged = 0;

    for vert in [false, true] {
        for k in 0..100u64 {
            let (mut arena_a, root_a) = treegen::build(31 + k, 4, 3, Regime::Varied);
            let ia = LayoutInput {
                root: root_a,
                vertically: vert,
                centeredxy: false,
                x: 0.0,
                y: 0.0,
            };
            layout(&mut arena_a, &ia);
            let plain = treegen::collect(&arena_a, root_a);

            let (mut arena_b, root_b) = treegen::build(31 + k, 4, 3, Regime::Varied);
            let ib = LayoutInput {
                centeredxy: true,
                root: root_b,
                ..ia
            };
            layout(&mut arena_b, &ib);
            let cent = treegen::collect(&arena_b, root_b);

            let breadth0 = if vert {
                treegen::left(&arena_b, cent[0]) - treegen::left(&arena_a, plain[0])
            } else {
                treegen::top(&arena_b, cent[0]) - treegen::top(&arena_a, plain[0])
            };

            for (&p, &c) in plain.iter().zip(&cent) {
                let dl = treegen::left(&arena_b, c) - treegen::left(&arena_a, p);
                let dt = treegen::top(&arena_b, c) - treegen::top(&arena_a, p);
                let (breadth, depth) = if vert { (dl, dt) } else { (dt, dl) };

                if !close_enough(breadth, breadth0) {
                    mismatch += 1;
                }
                if !close_enough(depth, 0.0) {
                    depthmoved += 1;
                }

                /* the flag itself has to reach every node, both ways */
                if !arena_b[c].centeredxy || arena_a[p].centeredxy {
                    unflagged += 1;
                }
            }
        }
    }

    assert_eq!(
        mismatch, 0,
        "`centeredxy` moved {mismatch} nodes beyond one rigid translation"
    );
    assert_eq!(
        depthmoved, 0,
        "`centeredxy` moved {depthmoved} nodes along the depth axis"
    );
    assert_eq!(unflagged, 0, "`centeredxy` did not reach {unflagged} nodes");
}
