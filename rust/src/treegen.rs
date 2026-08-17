//! Test support: a deterministic generator of random trees and a few geometric predicates.
//!
//! This is the port of `test/treegen.h`. It lives in the library rather than beside the
//! tests because the differential oracle (`src/bin/dump.rs`) needs the very same corpus,
//! and because the generator has to stay bit-compatible with the C one: the trees are
//! identified by their seed alone, so the sequence of draws must not drift.

use crate::{Arena, NodeId};

/// A deterministic PRNG, so that a failing case is reproducible from its seed alone.
///
/// The same linear congruential generator as `tg_rand`, down to the discarded low bits.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Seeds the generator; `tg_srand`.
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// A pseudo random value in `0..n`; `tg_rand`.
    pub fn rand(&mut self, n: u64) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 33) % n
    }
}

/// Size regimes.
///
/// [`Regime::Coarse`] matters more than it looks: multiples of ten make exactly-touching
/// siblings (a contour pair with `dist == 0.0`) common, which is the regime that exposed
/// the `first` flag bug in `separate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    /// Multiples of ten, no margin.
    Coarse,
    /// Every node identical.
    Uniform,
    /// A wide spread of sizes, with margins.
    Varied,
    /// Small integers.
    Tiny,
}

/// The regimes in the order the C enumerates them, which the seeds depend on.
pub const REGIMES: [Regime; 4] = [
    Regime::Coarse,
    Regime::Uniform,
    Regime::Varied,
    Regime::Tiny,
];

impl Regime {
    /// The position of the regime in [`REGIMES`], the value of the C enumerator.
    pub fn index(self) -> usize {
        REGIMES.iter().position(|&r| r == self).unwrap()
    }

    fn sizes(self, rng: &mut Rng) -> (f64, f64, f64) {
        match self {
            Regime::Uniform => (10.0, 10.0, 0.0),
            Regime::Varied => {
                let w = (5 + rng.rand(40)) as f64;
                let h = (5 + rng.rand(30)) as f64;
                let m = rng.rand(6) as f64;
                (w, h, m)
            }
            Regime::Tiny => {
                let w = (1 + rng.rand(4)) as f64;
                let h = (1 + rng.rand(4)) as f64;
                let m = rng.rand(2) as f64;
                (w, h, m)
            }
            Regime::Coarse => {
                let w = (10 * (1 + rng.rand(3))) as f64;
                let h = (10 * (1 + rng.rand(3))) as f64;
                (w, h, 0.0)
            }
        }
    }
}

struct Builder<'a> {
    arena: &'a mut Arena,
    rng: Rng,
    idx: usize,
}

impl Builder<'_> {
    fn build(&mut self, depth: usize, maxdepth: usize, maxkids: usize, regime: Regime) -> NodeId {
        let cs = if depth >= maxdepth {
            0
        } else {
            self.rng.rand(maxkids as u64 + 1) as usize
        };

        let (w, h, m) = regime.sizes(&mut self.rng);

        self.idx += 1;
        let t = self.arena.add_node(self.idx, w, h, m, false);

        for _ in 0..cs {
            let c = self.build(depth + 1, maxdepth, maxkids, regime);
            self.arena.push_child(t, c);
        }

        t
    }
}

/// A random tree in a fresh arena; `tg_build`.
///
/// Nodes are labelled from 1 in preorder, as in the C.
pub fn build(seed: u64, maxdepth: usize, maxkids: usize, regime: Regime) -> (Arena, NodeId) {
    let mut arena = Arena::new();
    let root = build_into(&mut arena, seed, maxdepth, maxkids, regime);
    (arena, root)
}

/// A random tree added to an existing arena, labelled from 1 as `tg_build` does.
pub fn build_into(
    arena: &mut Arena,
    seed: u64,
    maxdepth: usize,
    maxkids: usize,
    regime: Regime,
) -> NodeId {
    let mut b = Builder {
        arena,
        rng: Rng::new(seed),
        idx: 0,
    };
    b.build(0, maxdepth, maxkids, regime)
}

/// A chain of `depth` nodes, each the only child of the previous one; `tg_chain`.
///
/// Built iteratively, where the C recurses: a chain is exactly the shape that would run the
/// builder out of stack long before the flat layout it is meant to exercise runs out of
/// anything. The nodes come out in the same order, labelled from `idx`.
pub fn chain(arena: &mut Arena, idx: usize, depth: usize, w: f64, h: f64) -> NodeId {
    let top = arena.add_node(idx, w, h, 0.0, false);

    let mut prev = top;
    for k in 1..depth {
        let node = arena.add_node(idx + k, w, h, 0.0, false);
        arena.push_child(prev, node);
        prev = node;
    }

    top
}

/// A tree of exactly `n` nodes in breadth-first order; `tg_bench_tree`.
///
/// The random trees of [`build`] are the wrong shape for a benchmark: their size is an
/// outcome rather than an input. Here every node but the root is handed to a parent in
/// breadth-first order, `1 + rand(maxkids)` at a time, so the size is exact and the shape
/// is still irregular.
///
/// The draws are the same as the C `tg_bench_tree`, in the same order, so both build the
/// very same tree from a given seed -- which is what makes the two benchmarks comparable.
pub fn bench_tree(arena: &mut Arena, n: usize, maxkids: usize, seed: u64) -> NodeId {
    assert!(n > 0, "a tree has at least a root");

    let mut rng = Rng::new(seed);

    let mut counts = vec![0usize; n];
    let mut remaining = n - 1;
    let mut i = 0;
    while remaining > 0 {
        let c = (1 + rng.rand(maxkids as u64) as usize).min(remaining);
        counts[i] = c;
        remaining -= c;
        i += 1;
    }

    let ids: Vec<NodeId> = (0..n)
        .map(|i| {
            let w = (10 + rng.rand(40)) as f64;
            let h = (10 + rng.rand(20)) as f64;
            let m = rng.rand(4) as f64;
            arena.add_node(i + 1, w, h, m, false)
        })
        .collect();

    let mut next = 1;
    for (i, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            arena.push_child(ids[i], ids[next]);
            next += 1;
        }
    }

    ids[0]
}

/// The same tree as [`bench_tree`], in the parallel arrays [`crate::layout_api`] takes.
///
/// Because the generator hands the nodes out in breadth-first order, the adjacency is
/// simply `2, 3, ..., n`: node 0 takes the first few, node 1 the next few, and so on.
///
/// Returns `(wh, whg, children)`; the C `tg_bench_arrays` builds the same three.
pub fn bench_arrays(n: usize, maxkids: usize, seed: u64) -> (Vec<f64>, Vec<f64>, Vec<usize>) {
    assert!(n > 0, "a tree has at least a root");

    let mut rng = Rng::new(seed);

    let mut children = vec![0usize; 2 * n - 1];
    let mut remaining = n - 1;
    let mut i = 0;
    while remaining > 0 {
        let c = (1 + rng.rand(maxkids as u64) as usize).min(remaining);
        children[i] = c;
        remaining -= c;
        i += 1;
    }

    let mut wh = vec![0.0; 3 * n];
    let mut whg = vec![0.0; 2 * n];
    for i in 0..n {
        wh[i] = (10 + rng.rand(40)) as f64;
        wh[i + n] = (10 + rng.rand(20)) as f64;
        wh[i + 2 * n] = rng.rand(4) as f64;
        whg[i] = wh[i];
        whg[i + n] = 5.0;
    }

    for i in 1..n {
        children[n + i - 1] = i + 1;
    }

    (wh, whg, children)
}

/// Gathers the nodes of the subtree in preorder; `tg_collect`.
pub fn collect(arena: &Arena, root: NodeId) -> Vec<NodeId> {
    arena.preorder(root)
}

/// The left edge of a box.
///
/// `centeredxy` relates `x`/`y` to the middle of the node rather than to its top left
/// corner, so the edges have to be derived rather than read.
pub fn left(arena: &Arena, t: NodeId) -> f64 {
    let n = &arena[t];
    if n.centeredxy {
        n.x - n.w / 2.0
    } else {
        n.x
    }
}

/// The top edge of a box; see [`left`].
pub fn top(arena: &Arena, t: NodeId) -> f64 {
    let n = &arena[t];
    if n.centeredxy {
        n.y - n.h / 2.0
    } else {
        n.y
    }
}

/// Strict overlap: boxes that merely share an edge are tidy, boxes that share area are not.
pub fn overlap(arena: &Arena, a: NodeId, b: NodeId) -> bool {
    const E: f64 = 1e-9;
    let (na, nb) = (&arena[a], &arena[b]);
    let (al, bl) = (left(arena, a), left(arena, b));
    let (at, bt) = (top(arena, a), top(arena, b));
    (al + na.w - E > bl && bl + nb.w - E > al) && (at + na.h - E > bt && bt + nb.h - E > at)
}

/// The smallest coordinate along the breadth axis, which `layout` normalizes.
pub fn min_breadth(arena: &Arena, nodes: &[NodeId], vertically: bool) -> f64 {
    nodes
        .iter()
        .map(|&id| if vertically { arena[id].x } else { arena[id].y })
        .fold(f64::INFINITY, |m, c| if c < m { c } else { m })
}

/// The smallest edge along each axis, i.e. the corner of the drawing's bounding box.
pub fn min_edges(arena: &Arena, nodes: &[NodeId]) -> (f64, f64) {
    nodes
        .iter()
        .fold((f64::INFINITY, f64::INFINITY), |(l, t), &id| {
            let (nl, nt) = (left(arena, id), top(arena, id));
            (if nl < l { nl } else { l }, if nt < t { nt } else { t })
        })
}
