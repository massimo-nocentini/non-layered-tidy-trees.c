//! A struct-of-arrays mirror of the tree, laid out by sweeps instead of by recursion.
//!
//! [`layout_flat`] computes exactly what [`crate::layout`] computes — the same arithmetic
//! in the same order, checked bit for bit against the C build over the whole corpus — but
//! it gets there differently:
//!
//! * the tree is copied into a transient mirror in **breadth-first order**, so the children
//!   of a node are a contiguous index range and every node of a given depth is adjacent;
//! * every walk becomes a sweep over that mirror, forwards or backwards, with no recursion
//!   at all — which also removes the stack-depth ceiling the recursive path has;
//! * `bottom` is precomputed once into `bot[]`, which takes two branches out of the
//!   innermost contour step;
//! * the elementwise sweeps go through [`Kernels`], which dispatches to AVX2 when the crate
//!   is built with `--features simd` and the CPU has it.
//!
//! This is steps 2 to 4 of `simd-plan.md`, whose verdict is worth repeating: `separate` is
//! the hot routine, it is a serial contour walk, and it cannot be vectorized at any width.
//! What is vectorized here is the minority of the runtime that *is* elementwise. See
//! `rust/README.md` for what that turns out to be worth.
//!
//! # Order of visits
//!
//! A sweep visits independent subtrees in a different order than the recursion does. The
//! arithmetic per node is unchanged, so the coordinates are identical; what is not
//! preserved is the order a `walk` callback would see, which is why this path takes no
//! callbacks. Use [`crate::layout_with`] when you need them.

use crate::{Arena, LayoutInput, NodeId};

/// The absence of a node, where the pointer version has `NULL`.
const NONE: i32 = -1;

/// Lays out the tree with the flat sweeps, writing `x` and `y` back into the arena.
///
/// Equivalent to [`crate::layout`] in every observable way except the order in which nodes
/// are visited internally, which no caller can see: it takes no callbacks.
pub fn layout_flat(arena: &mut Arena, input: &LayoutInput) {
    Engine::new().layout(arena, input);
}

/// [`layout_flat`], with the elementwise sweeps vectorized where the CPU allows it.
///
/// Falls back to the scalar kernels when the target has no AVX2, so the result is the same
/// everywhere — the kernels are elementwise, and widening an elementwise operation does not
/// change a single bit of it.
#[cfg(feature = "simd")]
pub fn layout_flat_simd(arena: &mut Arena, input: &LayoutInput) {
    Engine::with_simd().layout(arena, input);
}

/// A reusable mirror, for laying out repeatedly.
///
/// [`layout_flat`] allocates the mirror on every call, which for a large tree is more work
/// than the layout itself: a million nodes is a hundred megabytes of arrays to fault in and
/// hand back. An `Engine` keeps them between calls, so only the first layout pays. The
/// results are identical either way.
///
/// ```
/// use non_layered_tidy_trees::{flat::Engine, Arena, LayoutInput};
///
/// let mut arena = Arena::new();
/// let root = arena.add_node(1, 10.0, 10.0, 0.0, false);
/// let leaf = arena.add_node(2, 10.0, 10.0, 0.0, false);
/// arena.set_children(root, &[leaf]);
///
/// let mut engine = Engine::new();
/// let input = LayoutInput::new(root);
///
/// engine.layout(&mut arena, &input);
/// engine.layout(&mut arena, &input); // no allocation this time
///
/// assert_eq!((arena[leaf].x, arena[leaf].y), (0.0, 10.0));
/// ```
#[derive(Debug, Default)]
pub struct Engine {
    flat: Flat,
    kernels: Kernels,
}

impl Engine {
    /// An engine using the scalar kernels.
    pub fn new() -> Engine {
        Engine {
            flat: Flat::default(),
            kernels: Kernels::scalar(),
        }
    }

    /// An engine using AVX2 where the CPU has it.
    #[cfg(feature = "simd")]
    pub fn with_simd() -> Engine {
        Engine {
            flat: Flat::default(),
            kernels: Kernels::detect(),
        }
    }

    /// Which kernels this engine dispatches to.
    pub fn kernels(&self) -> Kernels {
        self.kernels
    }

    /// Lays out the tree, reusing whatever this engine already allocated.
    pub fn layout(&mut self, arena: &mut Arena, input: &LayoutInput) {
        run(&mut self.flat, arena, input, self.kernels)
    }

    /// [`layout_api_flat`], reusing whatever this engine already allocated.
    #[allow(clippy::too_many_arguments)]
    pub fn layout_api(
        &mut self,
        n: usize,
        wh: &mut [f64],
        whg: &[f64],
        children: &[usize],
        rooti: usize,
        vertically: bool,
        centeredxy: bool,
        x: f64,
        y: f64,
    ) {
        assert!(
            (1..=n).contains(&rooti),
            "`rooti` is a one-based index into {n} nodes"
        );

        self.flat
            .rebuild_from_arrays(n, wh, whg, children, rooti, vertically);

        let minbreadth = sweeps(&mut self.flat, vertically, centeredxy, x, y, self.kernels);
        let _ = minbreadth;

        self.flat.write_back_arrays(n, wh, vertically);
    }

    /// [`Engine::layout`], reporting how long each sweep took.
    ///
    /// A benchmarking aid: `simd-plan.md` asks for the share of runtime each phase holds,
    /// and this is what answers it. See `src/bin/bench.rs`.
    pub fn profile(&mut self, arena: &mut Arena, input: &LayoutInput) -> Phases {
        let mut phases = Phases::default();
        run_profiled(&mut self.flat, arena, input, self.kernels, &mut phases);
        phases
    }
}

/// How long each phase of a flat layout took; see [`Engine::profile`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Phases {
    /// Building the mirror: allocation, and one copy in.
    pub build: std::time::Duration,
    /// The depth axis sweep.
    pub setup: std::time::Duration,
    /// The contour walk, which is the algorithm proper.
    pub first: std::time::Duration,
    /// The modifier sums, the coordinates and the child spacing.
    pub second: std::time::Duration,
    /// Normalizing the drawing to the origin.
    pub third: std::time::Duration,
    /// Copying the coordinates back into the arena.
    pub write_back: std::time::Duration,
}

impl Phases {
    /// The phases as `(name, duration)`, in the order they run.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, std::time::Duration)> {
        [
            ("build", self.build),
            ("setup", self.setup),
            ("first", self.first),
            ("second", self.second),
            ("third", self.third),
            ("write", self.write_back),
        ]
        .into_iter()
    }

    /// The sum of the phases.
    pub fn total(&self) -> std::time::Duration {
        self.iter().map(|(_, d)| d).sum()
    }
}

fn run(flat: &mut Flat, arena: &mut Arena, input: &LayoutInput, k: Kernels) {
    flat.rebuild(arena, input.root, input.vertically);

    sweeps(
        flat,
        input.vertically,
        input.centeredxy,
        input.x,
        input.y,
        k,
    );

    flat.write_back(arena, input.vertically, input.centeredxy);
}

/// The three sweeps and the normalization, over a mirror that is already filled.
fn sweeps(flat: &mut Flat, vertically: bool, centeredxy: bool, x: f64, y: f64, k: Kernels) -> f64 {
    flat.setup_sweep(k);
    flat.first_sweep();
    let minbreadth = flat.second_sweep(centeredxy, k);

    // The counterpart of `third_walk`: the depth axis is offset by the input alone, the
    // breadth axis is normalized to the origin first.
    let (in_breadth, in_depth) = if vertically { (x, y) } else { (y, x) };

    let dbreadth = minbreadth - in_breadth;
    let ddepth = -in_depth;

    if dbreadth != 0.0 || ddepth != 0.0 {
        flat.third_sweep(dbreadth, ddepth, k);
    }

    minbreadth
}

/// [`crate::layout_api`], without ever building a tree of nodes.
///
/// The flat API is handed its tree as arrays already, so the mirror can be filled straight
/// from them: no [`Arena`], no `Node`, no per-node child list, and no copy back through
/// one. This is the case the struct-of-arrays rewrite was worth doing for -- see the
/// benchmark table in `rust/README.md`.
///
/// The arguments are exactly [`crate::layout_api`]'s, and so is the result, bit for bit.
///
/// # Panics
///
/// If the arrays are shorter than the layouts [`crate::layout_api`] documents, or if
/// `rooti` is out of range.
#[allow(clippy::too_many_arguments)]
pub fn layout_api_flat(
    n: usize,
    wh: &mut [f64],
    whg: &[f64],
    children: &[usize],
    rooti: usize,
    vertically: bool,
    centeredxy: bool,
    x: f64,
    y: f64,
) {
    Engine::new().layout_api(n, wh, whg, children, rooti, vertically, centeredxy, x, y);
}

/// [`run`], with a stopwatch between the phases; see [`Engine::profile`].
fn run_profiled(
    flat: &mut Flat,
    arena: &mut Arena,
    input: &LayoutInput,
    k: Kernels,
    phases: &mut Phases,
) {
    use std::time::Instant;

    let t = Instant::now();
    flat.rebuild(arena, input.root, input.vertically);
    phases.build = t.elapsed();

    let t = Instant::now();
    flat.setup_sweep(k);
    phases.setup = t.elapsed();

    let t = Instant::now();
    flat.first_sweep();
    phases.first = t.elapsed();

    let t = Instant::now();
    let minbreadth = flat.second_sweep(input.centeredxy, k);
    phases.second = t.elapsed();

    let (in_breadth, in_depth) = if input.vertically {
        (input.x, input.y)
    } else {
        (input.y, input.x)
    };

    let dbreadth = minbreadth - in_breadth;
    let ddepth = -in_depth;

    let t = Instant::now();
    if dbreadth != 0.0 || ddepth != 0.0 {
        flat.third_sweep(dbreadth, ddepth, k);
    }
    phases.third = t.elapsed();

    let t = Instant::now();
    flat.write_back(arena, input.vertically, input.centeredxy);
    phases.write_back = t.elapsed();
}

/// The tree, transposed: one array per field, in breadth-first order.
///
/// The breadth axis is the one the algorithm positions along and the depth axis is the one
/// it stacks levels along; which of `x`/`y` each is depends on `vertically`, and the
/// mapping happens once on the way in and once on the way out rather than at every use.
#[derive(Debug, Default)]
struct Flat {
    n: usize,

    /// Extent along the breadth axis: `w` when laying out vertically.
    bext: Vec<f64>,
    /// Extent along the depth axis: `h` when laying out vertically.
    dext: Vec<f64>,
    margin: Vec<f64>,

    breadth: Vec<f64>,
    depth: Vec<f64>,
    /// `bottom()`, precomputed: `depth + dext` while the walks need it.
    bot: Vec<f64>,

    prelim: Vec<f64>,
    modifier: Vec<f64>,
    shift: Vec<f64>,
    change: Vec<f64>,
    msel: Vec<f64>,
    mser: Vec<f64>,
    modsum: Vec<f64>,

    tl: Vec<i32>,
    tr: Vec<i32>,
    el: Vec<i32>,
    er: Vec<i32>,

    parent: Vec<i32>,
    childno: Vec<u32>,
    first_child: Vec<u32>,
    child_count: Vec<u32>,

    /// Level `l` spans `levels[l]..levels[l + 1]`.
    levels: Vec<usize>,
    /// Where each node came from, when the mirror was built from an [`Arena`].
    id: Vec<NodeId>,
    /// What each node is, when the mirror was built from flat arrays: `i` for the content
    /// node `i`, `i + n` for the gap node above it, which has no output slot of its own.
    src: Vec<i32>,
    /// Where each node's children start in the adjacency; scratch for the flat arrays.
    off: Vec<usize>,
    /// How many content nodes the mirror reached.
    content: usize,
}

/// `v` becomes `n` copies of `zero`, keeping whatever it already allocated.
fn reset<T: Copy>(v: &mut Vec<T>, n: usize, zero: T) {
    v.clear();
    v.resize(n, zero);
}

impl Flat {
    /// Fills the mirror from the arena, reusing every allocation it already holds.
    fn rebuild(&mut self, arena: &Arena, root: NodeId, vertically: bool) {
        let f = self;

        f.id.clear();
        f.src.clear();
        f.bext.clear();
        f.dext.clear();
        f.margin.clear();
        f.parent.clear();
        f.childno.clear();
        f.first_child.clear();
        f.child_count.clear();
        f.levels.clear();
        f.levels.extend([0, 1]);

        f.push(arena, root, NONE, 0, vertically);

        // Breadth-first, one level at a time: parents are always visited before their
        // children, so the children of a node are the contiguous range they were pushed
        // into and the nodes of a level are adjacent.
        let mut start = 0;
        let mut end = 1;
        while start < end {
            for i in start..end {
                f.first_child[i] = f.id.len() as u32;
                let children = arena.children(f.id[i]);
                f.child_count[i] = children.len() as u32;
                for (k, &c) in children.iter().enumerate() {
                    f.push(arena, c, i as i32, k as u32, vertically);
                }
            }
            start = end;
            end = f.id.len();
            if start < end {
                f.levels.push(end);
            }
        }

        // Every accumulator starts at zero and every thread at `NONE`, which is what the
        // recursive path's `setup_walk` spends a whole traversal restoring.
        let n = f.id.len();
        f.n = n;
        reset(&mut f.breadth, n, 0.0);
        reset(&mut f.depth, n, 0.0);
        reset(&mut f.bot, n, 0.0);
        reset(&mut f.prelim, n, 0.0);
        reset(&mut f.modifier, n, 0.0);
        reset(&mut f.shift, n, 0.0);
        reset(&mut f.change, n, 0.0);
        reset(&mut f.msel, n, 0.0);
        reset(&mut f.mser, n, 0.0);
        reset(&mut f.modsum, n, 0.0);
        reset(&mut f.tl, n, NONE);
        reset(&mut f.tr, n, NONE);
        reset(&mut f.el, n, NONE);
        reset(&mut f.er, n, NONE);
    }

    /// Fills the mirror straight from the arrays of the flat API, gap nodes included.
    fn rebuild_from_arrays(
        &mut self,
        n: usize,
        wh: &[f64],
        whg: &[f64],
        children: &[usize],
        rooti: usize,
        vertically: bool,
    ) {
        assert!(
            wh.len() >= 3 * n,
            "`wh` holds w, h and margin for {n} nodes"
        );
        assert!(whg.len() >= 2 * n, "`whg` holds w and h for {n} gap nodes");
        assert!(children.len() >= n, "`children` starts with {n} counts");

        let f = self;

        f.id.clear();
        f.src.clear();
        f.bext.clear();
        f.dext.clear();
        f.margin.clear();
        f.parent.clear();
        f.childno.clear();
        f.first_child.clear();
        f.child_count.clear();
        f.levels.clear();
        f.levels.extend([0, 1]);

        // The adjacency follows the `n` counts, grouped by parent.
        f.off.clear();
        f.off.reserve(n);
        let mut acc = n;
        for &count in &children[..n] {
            f.off.push(acc);
            acc += count;
        }
        assert!(
            children.len() >= acc,
            "the adjacency is shorter than the counts promise"
        );

        f.content = 0;
        f.push_from_arrays(n, (rooti - 1 + n) as i32, wh, whg, NONE, 0, vertically);

        let mut start = 0;
        let mut end = 1;
        while start < end {
            for m in start..end {
                f.first_child[m] = f.src.len() as u32;
                let s = f.src[m] as usize;

                if s < n {
                    // A content node: its children are the gap nodes above theirs.
                    let count = children[s];
                    f.child_count[m] = count as u32;
                    for k in 0..count {
                        let c = children[f.off[s] + k] - 1;
                        f.push_from_arrays(
                            n,
                            (c + n) as i32,
                            wh,
                            whg,
                            m as i32,
                            k as u32,
                            vertically,
                        );
                    }
                } else {
                    // A gap node: exactly the content node it separates.
                    f.child_count[m] = 1;
                    f.push_from_arrays(n, (s - n) as i32, wh, whg, m as i32, 0, vertically);
                }
            }
            start = end;
            end = f.src.len();
            if start < end {
                f.levels.push(end);
            }
        }

        let total = f.src.len();
        f.n = total;
        reset(&mut f.breadth, total, 0.0);
        reset(&mut f.depth, total, 0.0);
        reset(&mut f.bot, total, 0.0);
        reset(&mut f.prelim, total, 0.0);
        reset(&mut f.modifier, total, 0.0);
        reset(&mut f.shift, total, 0.0);
        reset(&mut f.change, total, 0.0);
        reset(&mut f.msel, total, 0.0);
        reset(&mut f.mser, total, 0.0);
        reset(&mut f.modsum, total, 0.0);
        reset(&mut f.tl, total, NONE);
        reset(&mut f.tr, total, NONE);
        reset(&mut f.el, total, NONE);
        reset(&mut f.er, total, NONE);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_from_arrays(
        &mut self,
        n: usize,
        src: i32,
        wh: &[f64],
        whg: &[f64],
        parent: i32,
        childno: u32,
        vertically: bool,
    ) {
        let s = src as usize;

        let (w, h, margin) = if s < n {
            self.content += 1;
            (wh[s], wh[s + n], wh[s + 2 * n])
        } else {
            (whg[s - n], whg[s - n + n], 0.0)
        };

        let (bext, dext) = if vertically { (w, h) } else { (h, w) };

        self.src.push(src);
        self.bext.push(bext);
        self.dext.push(dext);
        self.margin.push(margin);
        self.parent.push(parent);
        self.childno.push(childno);
        self.first_child.push(0);
        self.child_count.push(0);
    }

    /// Writes `[x_0.., y_0..]` over the first `2n` of `wh`, as `flat_xy_into` does.
    fn write_back_arrays(&self, n: usize, wh: &mut [f64], vertically: bool) {
        // A node the root cannot reach is not part of the drawing, and the pointer version
        // leaves it at the origin rather than at whatever the input said.
        if self.content < n {
            for v in wh[..2 * n].iter_mut() {
                *v = 0.0;
            }
        }

        for m in 0..self.n {
            let s = self.src[m] as usize;
            if s >= n {
                continue;
            }

            let (b, d) = (self.breadth[m], self.depth[m]);
            let (x, y) = if vertically { (b, d) } else { (d, b) };

            wh[s] = x;
            wh[s + n] = y;
        }
    }

    fn push(&mut self, arena: &Arena, id: NodeId, parent: i32, childno: u32, vertically: bool) {
        let node = &arena[id];
        let (bext, dext) = if vertically {
            (node.w, node.h)
        } else {
            (node.h, node.w)
        };

        self.id.push(id);
        self.bext.push(bext);
        self.dext.push(dext);
        self.margin.push(node.margin);
        self.parent.push(parent);
        self.childno.push(childno);
        self.first_child.push(0);
        self.child_count.push(0);
    }

    fn children(&self, i: usize) -> std::ops::Range<usize> {
        let f = self.first_child[i] as usize;
        f..f + self.child_count[i] as usize
    }

    /// `setup_walk`: the depth axis follows from the node extents, one level below the next.
    fn setup_sweep(&mut self, k: Kernels) {
        self.depth[0] = 0.0;
        self.bot[0] = self.dext[0];

        for i in 0..self.n {
            let c = self.children(i);
            if c.is_empty() {
                continue;
            }
            let b = self.bot[i];

            // Every child of a node sits directly below it, so this is a fill over a
            // contiguous range and an elementwise add over the same one.
            k.fill(&mut self.depth[c.clone()], b);
            k.add_scalar(&mut self.bot[c.clone()], &self.dext[c], b);
        }
    }

    /// `first_walk`: a reverse sweep, deepest level first.
    ///
    /// Nodes of one level own disjoint subtrees, so their `separate`/`position_root` work is
    /// independent of one another; visiting them in index order rather than in the order the
    /// recursion would is the only thing that changes.
    fn first_sweep(&mut self) {
        let mut chain: Vec<ChainLink> = Vec::new();

        for i in (0..self.n).rev() {
            let c = self.children(i);

            if c.is_empty() {
                self.el[i] = i as i32;
                self.er[i] = i as i32;
                self.msel[i] = 0.0;
                self.mser[i] = 0.0;
                continue;
            }

            let first = c.start;
            let last = c.end - 1;

            chain.clear();
            update_chain(&mut chain, self.bot[self.el[first] as usize], 0);

            for k in 1..c.len() {
                let ci = first + k;
                let min = self.bot[self.er[ci] as usize];

                self.separate(i, k, &chain);

                update_chain(&mut chain, min, k);
            }

            // `position_root`: between the children, taking their modifiers into account.
            let d = self.bext[last] - self.bext[i];
            self.prelim[i] = (self.prelim[first]
                + self.modifier[first]
                + self.prelim[last]
                + self.modifier[last]
                + d)
                / 2.0;

            self.el[i] = self.el[first];
            self.msel[i] = self.msel[first];
            self.er[i] = self.er[last];
            self.mser[i] = self.mser[last];
        }
    }

    /// The contour walk, which is where the time goes and which stays scalar.
    fn separate(&mut self, t: usize, k: usize, chain: &[ChainLink]) {
        let base = self.first_child[t] as usize;
        let i = base + k;

        let mut sr = i as i32 - 1;
        let mut mssr = self.modifier[i - 1];

        let mut cl = i as i32;
        let mut mscl = self.modifier[i];

        let mut ih = chain.len() - 1;
        let mut first = true;

        while sr != NONE && cl != NONE {
            let (s, c) = (sr as usize, cl as usize);

            if self.bot[s] > chain[ih].low {
                ih = ih.checked_sub(1).expect(
                    "the sibling chain is seeded with a value that dominates the subtree depth",
                );
            }

            let dist =
                (mssr + self.prelim[s] + self.bext[s] + self.margin[s]) - (mscl + self.prelim[c]);

            if (first && dist < 0.0) || dist > 0.0 {
                mscl += dist;
                self.move_subtree(t, k, chain[ih].index, dist);
            }

            first = false;

            let (sy, cy) = (self.bot[s], self.bot[c]);

            if sy <= cy {
                sr = self.next_right_contour(s);
                if sr != NONE {
                    mssr += self.modifier[sr as usize];
                }
            }

            if sy >= cy {
                cl = self.next_left_contour(c);
                if cl != NONE {
                    mscl += self.modifier[cl as usize];
                }
            }
        }

        if sr == NONE && cl != NONE {
            self.set_left_thread(base, k, cl as usize, mscl);
        } else if sr != NONE && cl == NONE {
            self.set_right_thread(base, k, sr as usize, mssr);
        }
    }

    fn next_left_contour(&self, i: usize) -> i32 {
        if self.child_count[i] == 0 {
            self.tl[i]
        } else {
            self.first_child[i] as i32
        }
    }

    fn next_right_contour(&self, i: usize) -> i32 {
        if self.child_count[i] == 0 {
            self.tr[i]
        } else {
            (self.first_child[i] + self.child_count[i] - 1) as i32
        }
    }

    fn move_subtree(&mut self, t: usize, k: usize, si: usize, dist: f64) {
        let base = self.first_child[t] as usize;
        let i = base + k;

        self.modifier[i] += dist;
        self.msel[i] += dist;
        self.mser[i] += dist;

        if si + 1 != k {
            // Are there intermediate children?
            let nr = (k - si) as f64;
            let ratio = dist / nr;
            self.shift[base + si + 1] += ratio;
            self.shift[i] -= ratio;
            self.change[i] -= dist - ratio;
        }
    }

    fn set_left_thread(&mut self, base: usize, k: usize, cl: usize, modsumcl: f64) {
        let i = base + k;
        let li = self.el[base] as usize;

        self.tl[li] = cl as i32;

        let diff = (modsumcl - self.modifier[cl]) - self.msel[base];
        self.modifier[li] += diff;
        self.prelim[li] -= diff;

        self.el[base] = self.el[i];
        self.msel[base] = self.msel[i];
    }

    fn set_right_thread(&mut self, base: usize, k: usize, sr: usize, modsumsr: f64) {
        let i = base + k;
        let ri = self.er[i] as usize;

        self.tr[ri] = sr as i32;

        let diff = (modsumsr - self.modifier[sr]) - self.mser[i];
        self.modifier[ri] += diff;
        self.prelim[ri] -= diff;

        self.er[i] = self.er[i - 1];
        self.mser[i] = self.mser[i - 1];
    }

    /// `second_walk`: a forward sweep, level by level, returning the smallest breadth.
    ///
    /// Within a level the modifier sum is a gather from the parents, which stays scalar;
    /// the coordinate that follows from it is elementwise, and so is the minimum.
    fn second_sweep(&mut self, centeredxy: bool, k: Kernels) -> f64 {
        self.modsum[0] = self.modifier[0];

        let mut best = f64::INFINITY;

        for l in 0..self.levels.len() - 1 {
            let level = self.levels[l]..self.levels[l + 1];

            if l > 0 {
                for i in level.clone() {
                    self.modsum[i] = self.modsum[self.parent[i] as usize] + self.modifier[i];
                }
            }

            k.add(
                &mut self.breadth[level.clone()],
                &self.prelim[level.clone()],
                &self.modsum[level.clone()],
            );

            if centeredxy {
                k.add_half(&mut self.breadth[level.clone()], &self.bext[level.clone()]);
                k.add_half(&mut self.depth[level.clone()], &self.dext[level.clone()]);
            }

            best = best.min(k.min(&self.breadth[level.clone()]));

            // `add_child_spacing`, the second-order prefix scan that has to stay scalar:
            // any parallel scan reassociates the additions and changes the last ULP. It
            // writes into the *next* level's modifiers, which is why it runs last.
            for i in level {
                let c = self.children(i);
                let mut d = 0.0;
                let mut modsumdelta = 0.0;
                for j in c {
                    d += self.shift[j];
                    modsumdelta += d + self.change[j];
                    self.modifier[j] += modsumdelta;
                }
            }
        }

        best
    }

    /// `third_walk`: one flat pass over everything, which is as elementwise as it gets.
    fn third_sweep(&mut self, dbreadth: f64, ddepth: f64, k: Kernels) {
        k.sub_scalar(&mut self.breadth, dbreadth);
        k.sub_scalar(&mut self.depth, ddepth);
    }

    fn write_back(&self, arena: &mut Arena, vertically: bool, centeredxy: bool) {
        for l in 0..self.levels.len() - 1 {
            for i in self.levels[l]..self.levels[l + 1] {
                let node = &mut arena[self.id[i]];

                if vertically {
                    node.x = self.breadth[i];
                    node.y = self.depth[i];
                } else {
                    node.x = self.depth[i];
                    node.y = self.breadth[i];
                }

                node.level = l;
                node.centeredxy = centeredxy;

                // The recursive path fills these in for children only, leaving whatever
                // the root already carried; do the same rather than clearing it.
                if self.parent[i] != NONE {
                    node.parent = Some(self.id[self.parent[i] as usize]);
                    node.childno = self.childno[i] as usize;
                }
            }
        }
    }
}

/// A link of the chain of left siblings and their lowest depth coordinate.
#[derive(Debug, Clone, Copy)]
struct ChainLink {
    low: f64,
    index: usize,
}

fn update_chain(chain: &mut Vec<ChainLink>, min: f64, i: usize) {
    while let Some(top) = chain.last() {
        if min >= top.low {
            chain.pop();
        } else {
            break;
        }
    }
    chain.push(ChainLink { low: min, index: i });
}

/// The elementwise sweeps, scalar or vectorized.
///
/// Every kernel here is elementwise or a `min` reduction, which is why widening them
/// cannot change a result: elementwise operations do not reassociate, and `min` is exact
/// under any association. The prefix scan of `add_child_spacing` is deliberately not here.
#[derive(Debug, Clone, Copy, Default)]
pub struct Kernels {
    simd: bool,
}

impl Kernels {
    /// Plain loops, which the compiler is free to vectorize on its own.
    pub fn scalar() -> Kernels {
        Kernels { simd: false }
    }

    /// AVX2 where the CPU has it, plain loops otherwise.
    #[cfg(feature = "simd")]
    pub fn detect() -> Kernels {
        #[cfg(target_arch = "x86_64")]
        let simd = simd::available();
        #[cfg(not(target_arch = "x86_64"))]
        let simd = false;

        Kernels { simd }
    }

    /// Whether the vectorized kernels are the ones in use.
    pub fn is_simd(self) -> bool {
        self.simd
    }

    /// `dst[i] = v`
    fn fill(self, dst: &mut [f64], v: f64) {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if self.simd {
            // SAFETY: `simd` is only set when the CPU reports AVX2.
            unsafe { simd::fill(dst, v) };
            return;
        }
        for d in dst {
            *d = v;
        }
    }

    /// `dst[i] = v + src[i]`
    fn add_scalar(self, dst: &mut [f64], src: &[f64], v: f64) {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if self.simd {
            // SAFETY: `simd` is only set when the CPU reports AVX2.
            unsafe { simd::add_scalar(dst, src, v) };
            return;
        }
        for (d, s) in dst.iter_mut().zip(src) {
            *d = v + *s;
        }
    }

    /// `dst[i] = a[i] + b[i]`
    fn add(self, dst: &mut [f64], a: &[f64], b: &[f64]) {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if self.simd {
            // SAFETY: `simd` is only set when the CPU reports AVX2.
            unsafe { simd::add(dst, a, b) };
            return;
        }
        for (d, (x, y)) in dst.iter_mut().zip(a.iter().zip(b)) {
            *d = *x + *y;
        }
    }

    /// `dst[i] += src[i] / 2`
    fn add_half(self, dst: &mut [f64], src: &[f64]) {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if self.simd {
            // SAFETY: `simd` is only set when the CPU reports AVX2.
            unsafe { simd::add_half(dst, src) };
            return;
        }
        for (d, s) in dst.iter_mut().zip(src) {
            *d += *s / 2.0;
        }
    }

    /// `dst[i] -= v`
    fn sub_scalar(self, dst: &mut [f64], v: f64) {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if self.simd {
            // SAFETY: `simd` is only set when the CPU reports AVX2.
            unsafe { simd::sub_scalar(dst, v) };
            return;
        }
        for d in dst {
            *d -= v;
        }
    }

    /// The smallest element, or `+inf` for an empty slice.
    fn min(self, v: &[f64]) -> f64 {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if self.simd {
            // SAFETY: `simd` is only set when the CPU reports AVX2.
            return unsafe { simd::min(v) };
        }
        v.iter()
            .fold(f64::INFINITY, |m, &c| if c < m { c } else { m })
    }
}

/// AVX2 kernels: four doubles at a time, with a scalar tail.
///
/// Every one of these is an elementwise operation or a `min` reduction, so the result is
/// bit-identical to the scalar loop above it whatever the lane count.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
mod simd {
    use std::arch::x86_64::*;

    pub fn available() -> bool {
        is_x86_feature_detected!("avx2")
    }

    const LANES: usize = 4;

    #[target_feature(enable = "avx2")]
    pub unsafe fn fill(dst: &mut [f64], v: f64) {
        let w = _mm256_set1_pd(v);
        let chunks = dst.len() / LANES;
        for c in 0..chunks {
            _mm256_storeu_pd(dst.as_mut_ptr().add(c * LANES), w);
        }
        for d in &mut dst[chunks * LANES..] {
            *d = v;
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn add_scalar(dst: &mut [f64], src: &[f64], v: f64) {
        let n = dst.len().min(src.len());
        let w = _mm256_set1_pd(v);
        let chunks = n / LANES;
        for c in 0..chunks {
            let s = _mm256_loadu_pd(src.as_ptr().add(c * LANES));
            _mm256_storeu_pd(dst.as_mut_ptr().add(c * LANES), _mm256_add_pd(w, s));
        }
        for i in chunks * LANES..n {
            *dst.get_unchecked_mut(i) = v + *src.get_unchecked(i);
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn add(dst: &mut [f64], a: &[f64], b: &[f64]) {
        let n = dst.len().min(a.len()).min(b.len());
        let chunks = n / LANES;
        for c in 0..chunks {
            let x = _mm256_loadu_pd(a.as_ptr().add(c * LANES));
            let y = _mm256_loadu_pd(b.as_ptr().add(c * LANES));
            _mm256_storeu_pd(dst.as_mut_ptr().add(c * LANES), _mm256_add_pd(x, y));
        }
        for i in chunks * LANES..n {
            *dst.get_unchecked_mut(i) = *a.get_unchecked(i) + *b.get_unchecked(i);
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn add_half(dst: &mut [f64], src: &[f64]) {
        let n = dst.len().min(src.len());
        let half = _mm256_set1_pd(0.5);
        let chunks = n / LANES;
        for c in 0..chunks {
            let d = _mm256_loadu_pd(dst.as_ptr().add(c * LANES));
            let s = _mm256_loadu_pd(src.as_ptr().add(c * LANES));
            // `* 0.5` and `/ 2.0` are the same operation on a binary float: an exact
            // exponent decrement, with no rounding to disagree about.
            _mm256_storeu_pd(
                dst.as_mut_ptr().add(c * LANES),
                _mm256_add_pd(d, _mm256_mul_pd(s, half)),
            );
        }
        for i in chunks * LANES..n {
            *dst.get_unchecked_mut(i) += *src.get_unchecked(i) / 2.0;
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn sub_scalar(dst: &mut [f64], v: f64) {
        let w = _mm256_set1_pd(v);
        let chunks = dst.len() / LANES;
        for c in 0..chunks {
            let d = _mm256_loadu_pd(dst.as_ptr().add(c * LANES));
            _mm256_storeu_pd(dst.as_mut_ptr().add(c * LANES), _mm256_sub_pd(d, w));
        }
        for d in &mut dst[chunks * LANES..] {
            *d -= v;
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn min(v: &[f64]) -> f64 {
        let chunks = v.len() / LANES;

        let mut acc = _mm256_set1_pd(f64::INFINITY);
        for c in 0..chunks {
            acc = _mm256_min_pd(acc, _mm256_loadu_pd(v.as_ptr().add(c * LANES)));
        }

        let mut lanes = [0.0f64; LANES];
        _mm256_storeu_pd(lanes.as_mut_ptr(), acc);

        let mut m = f64::INFINITY;
        for &c in lanes.iter().chain(&v[chunks * LANES..]) {
            if c < m {
                m = c;
            }
        }
        m
    }
}
