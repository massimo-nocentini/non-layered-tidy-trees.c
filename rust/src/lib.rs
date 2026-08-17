//! Drawing non-layered tidy trees in linear time.
//!
//! A port of `src/non-layered-tidy-trees.c` in this repository, which is itself a
//! translation of the algorithm described in
//!
//! > van der Ploeg, A. (2014), *Drawing non-layered tidy trees in linear time*,
//! > Softw. Pract. Exper., 44, pages 1467–1484, doi:
//! > [10.1002/spe.2213](https://doi.org/10.1002/spe.2213)
//!
//! The walks, the sibling chain, the threading and the one behavioural fix in
//! [`separate`](fn@layout) follow the C sources line by line, so both produce
//! bit-identical coordinates; what changed is the shape of the data, not the arithmetic:
//!
//! * nodes live in an [`Arena`] and refer to each other by [`NodeId`] rather than by
//!   pointer, which is what makes the threads (`tl`/`tr`), the extreme nodes (`el`/`er`)
//!   and the parent links expressible without `unsafe`;
//! * the sibling chain is a `Vec` used as a stack instead of a hand-freed linked list;
//! * the two callbacks are closures, so `walkud`/`cpairsud` are captured state rather
//!   than `void *`, and they live in [`Callbacks`] instead of in the input struct — which
//!   leaves [`LayoutInput`] plain data that [`layout`] takes by shared reference;
//! * `free_tree` has no counterpart: dropping the [`Arena`] frees everything, without
//!   recursion.
//!
//! ```
//! use non_layered_tidy_trees::{layout, Arena, LayoutInput};
//!
//! let mut arena = Arena::new();
//! let root = arena.add_node(1, 10.0, 10.0, 0.0, false);
//! let a = arena.add_node(2, 10.0, 10.0, 0.0, false);
//! let b = arena.add_node(3, 10.0, 10.0, 0.0, false);
//! arena.set_children(root, &[a, b]);
//!
//! layout(&mut arena, &LayoutInput::new(root));
//!
//! assert_eq!((arena[a].x, arena[a].y), (0.0, 10.0));
//! assert_eq!((arena[b].x, arena[b].y), (10.0, 10.0));
//! assert_eq!((arena[root].x, arena[root].y), (5.0, 0.0)); // centred over its children
//! ```

use std::ops::{Index, IndexMut};

pub mod flat;
pub mod treegen;

/// A handle on a node held by an [`Arena`].
///
/// Ids are only meaningful for the arena that produced them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(usize);

impl NodeId {
    /// The position of the node in its arena, in creation order.
    pub fn index(self) -> usize {
        self.0
    }
}

/// A node of a hierarchy that acts as a tree, the counterpart of `tree_t`.
///
/// `w`, `h` and `margin` are the input; `x` and `y` are the output. Everything else is
/// either bookkeeping filled in by [`layout`] (`level`, `childno`, `centeredxy`,
/// `parent`) or private state of the algorithm.
#[derive(Debug, Clone)]
pub struct Node {
    /// A caller-chosen label, one-based for the flat API (see [`layout_api`]).
    pub idx: usize,
    /// Distance from the root, set by [`layout`]; the root is at level 0.
    pub level: usize,
    /// Position among the parent's children, set by [`layout`]; 0 for the root.
    pub childno: usize,
    /// Whether `x`/`y` denote the middle of the box rather than its top left corner.
    ///
    /// Set by [`layout`] from [`LayoutInput::centeredxy`]; `false` until then.
    pub centeredxy: bool,
    /// Marks a node inserted only to take up space, as [`reify_flat_chunks`] does.
    pub isdummy: bool,
    /// Width of the box.
    pub w: f64,
    /// Height of the box.
    pub h: f64,
    /// Extra separation kept between this node and the sibling subtree to its right.
    pub margin: f64,
    /// Breadth or depth coordinate, depending on [`LayoutInput::vertically`].
    pub x: f64,
    /// Depth or breadth coordinate, depending on [`LayoutInput::vertically`].
    pub y: f64,
    /// The parent, set by [`layout`]; `None` for the root.
    pub parent: Option<NodeId>,

    children: Vec<NodeId>,

    prelim: f64,
    /// `mod` in the C sources, which is a keyword here.
    modifier: f64,
    shift: f64,
    change: f64,
    /// Left and right thread.
    tl: Option<NodeId>,
    tr: Option<NodeId>,
    /// Extreme left and right nodes.
    el: Option<NodeId>,
    er: Option<NodeId>,
    /// Sum of modifiers at the extreme nodes.
    msel: f64,
    mser: f64,
}

impl Node {
    fn new(idx: usize, w: f64, h: f64, margin: f64, isdummy: bool) -> Self {
        Node {
            idx,
            level: 0,
            childno: 0,
            centeredxy: false,
            isdummy,
            w,
            h,
            margin,
            x: 0.0,
            y: 0.0,
            parent: None,
            children: Vec::new(),
            prelim: 0.0,
            modifier: 0.0,
            shift: 0.0,
            change: 0.0,
            tl: None,
            tr: None,
            el: None,
            er: None,
            msel: 0.0,
            mser: 0.0,
        }
    }

    /// The children, in left to right order.
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

/// The nodes of one or more trees, owning them for as long as the arena lives.
///
/// This replaces `init_tree`/`free_tree`: nodes are added with [`Arena::add_node`], wired
/// together with [`Arena::set_children`], and freed all at once when the arena is dropped.
#[derive(Debug, Clone, Default)]
pub struct Arena {
    nodes: Vec<Node>,
}

impl Index<NodeId> for Arena {
    type Output = Node;

    fn index(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }
}

impl IndexMut<NodeId> for Arena {
    fn index_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.0]
    }
}

impl Arena {
    /// An empty arena.
    pub fn new() -> Self {
        Arena { nodes: Vec::new() }
    }

    /// An empty arena with room for `capacity` nodes.
    pub fn with_capacity(capacity: usize) -> Self {
        Arena {
            nodes: Vec::with_capacity(capacity),
        }
    }

    /// Adds a childless node, the counterpart of `init_tree`.
    ///
    /// `idx` is a label carried through to the output and is not interpreted, except by
    /// the flat API where it has to be the one-based position of the node.
    pub fn add_node(&mut self, idx: usize, w: f64, h: f64, margin: f64, isdummy: bool) -> NodeId {
        self.nodes.push(Node::new(idx, w, h, margin, isdummy));
        NodeId(self.nodes.len() - 1)
    }

    /// Appends one child to `parent`.
    pub fn push_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent.0].children.push(child);
    }

    /// Replaces the children of `parent`, in left to right order.
    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) {
        self.nodes[parent.0].children.clear();
        self.nodes[parent.0].children.extend_from_slice(children);
    }

    /// The children of `t`, in left to right order.
    pub fn children(&self, t: NodeId) -> &[NodeId] {
        &self.nodes[t.0].children
    }

    /// How many nodes the arena holds.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the arena holds no node at all.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The nodes, in creation order.
    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    /// The ids of the subtree rooted at `t`, in preorder.
    pub fn preorder(&self, t: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.preorder_into(t, &mut out);
        out
    }

    fn preorder_into(&self, t: NodeId, out: &mut Vec<NodeId>) {
        out.push(t);
        for i in 0..self.nchildren(t) {
            self.preorder_into(self.child(t, i), out);
        }
    }

    /// The far edge of a node along the depth axis, the counterpart of `bottom`.
    ///
    /// That is the bottom of the box when laying out vertically and its right hand side
    /// when laying out horizontally.
    pub fn bottom(&self, t: NodeId, vertically: bool) -> f64 {
        let n = &self.nodes[t.0];
        if vertically {
            n.y + if n.centeredxy { n.h / 2.0 } else { n.h }
        } else {
            n.x + if n.centeredxy { n.w / 2.0 } else { n.w }
        }
    }

    fn child(&self, t: NodeId, i: usize) -> NodeId {
        self.nodes[t.0].children[i]
    }

    fn nchildren(&self, t: NodeId) -> usize {
        self.nodes[t.0].children.len()
    }
}

/// What to lay out, and how; the data half of `treeinput_t`.
///
/// `vertically` and `centeredxy` are booleans here, where the C takes any nonzero `int`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutInput {
    /// The root of the tree to lay out.
    pub root: NodeId,
    /// Whether the depth axis is `y` (top down) rather than `x` (left to right).
    pub vertically: bool,
    /// Whether `x`/`y` should denote the middle of each box rather than its top left corner.
    pub centeredxy: bool,
    /// Where the normalized drawing starts along `x`.
    pub x: f64,
    /// Where the normalized drawing starts along `y`.
    pub y: f64,
}

impl LayoutInput {
    /// A top down drawing of `root`, in top-left coordinates, starting at the origin.
    pub fn new(root: NodeId) -> Self {
        LayoutInput {
            root,
            vertically: true,
            centeredxy: false,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// Called once per node as the second walk finalizes it; the counterpart of `callback_t`.
pub type WalkFn<'a> = dyn FnMut(&mut Arena, NodeId) + 'a;

/// Called for every contour pair `separate` compares; the counterpart of `contourpairs_t`.
pub type ContourPairsFn<'a> = dyn FnMut(&Arena, NodeId, NodeId, f64) + 'a;

/// The two hooks of `treeinput_t`, as closures.
///
/// `walk` observes every node as `secondWalk` finalizes it — it is handed the arena, so it
/// may move the node it is given, which does not perturb normalization. `contour_pairs`
/// observes every pair of contour nodes `separate` compares, together with their `dist`;
/// it is the only external window onto the contour walk.
#[derive(Default)]
pub struct Callbacks<'a> {
    /// Called once per node, in preorder, during the second walk.
    pub walk: Option<&'a mut WalkFn<'a>>,
    /// Called for every contour pair `(sr, cl, dist)` compared while separating siblings.
    pub contour_pairs: Option<&'a mut ContourPairsFn<'a>>,
}

/// A link of the chain of left siblings and their lowest depth coordinate.
///
/// The C keeps a hand-freed linked list; here the chain is a stack, so `nxt` is the
/// element below and the head is the last one.
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

/// Process change and shift to add intermediate spacing to mod.
fn add_child_spacing(a: &mut Arena, t: NodeId) {
    let mut d = 0.0;
    let mut modsumdelta = 0.0;
    for i in 0..a.nchildren(t) {
        let c = a.child(t, i);
        d += a[c].shift;
        modsumdelta += d + a[c].change;
        a[c].modifier += modsumdelta;
    }
}

fn move_subtree(a: &mut Arena, t: NodeId, i: usize, si: usize, dist: f64) {
    let ci = a.child(t, i);
    a[ci].modifier += dist;
    a[ci].msel += dist;
    a[ci].mser += dist;

    if si + 1 != i {
        // Are there intermediate children?
        let nr = (i - si) as f64;
        let ratio = dist / nr;
        let cs = a.child(t, si + 1);
        a[cs].shift += ratio;
        a[ci].shift -= ratio;
        a[ci].change -= dist - ratio;
    }
}

fn next_left_contour(a: &Arena, t: NodeId) -> Option<NodeId> {
    if a.nchildren(t) == 0 {
        a[t].tl
    } else {
        Some(a.child(t, 0))
    }
}

fn next_right_contour(a: &Arena, t: NodeId) -> Option<NodeId> {
    let n = a.nchildren(t);
    if n == 0 {
        a[t].tr
    } else {
        Some(a.child(t, n - 1))
    }
}

fn set_left_thread(a: &mut Arena, t: NodeId, i: usize, cl: NodeId, modsumcl: f64) {
    let c0 = a.child(t, 0);
    let ci = a.child(t, i);
    let li = a[c0]
        .el
        .expect("the leftmost child has an extreme left node");
    a[li].tl = Some(cl);

    // Change mod so that the sum of modifier after following thread is correct.
    let diff = (modsumcl - a[cl].modifier) - a[c0].msel;
    a[li].modifier += diff;

    // Change preliminary x coordinate so that the node does not move.
    a[li].prelim -= diff;

    // Update extreme node and its sum of modifiers.
    a[c0].el = a[ci].el;
    a[c0].msel = a[ci].msel;
}

/// Symmetrical to [`set_left_thread`].
fn set_right_thread(a: &mut Arena, t: NodeId, i: usize, sr: NodeId, modsumsr: f64) {
    let ci = a.child(t, i);
    let cp = a.child(t, i - 1);
    let ri = a[ci].er.expect("the i-th child has an extreme right node");
    a[ri].tr = Some(sr);
    let diff = (modsumsr - a[sr].modifier) - a[ci].mser;
    a[ri].modifier += diff;
    a[ri].prelim -= diff;
    a[ci].er = a[cp].er;
    a[ci].mser = a[cp].mser;
}

fn separate(
    a: &mut Arena,
    input: &LayoutInput,
    cb: &mut Callbacks,
    t: NodeId,
    i: usize,
    chain: &[ChainLink],
) {
    let mut sr = Some(a.child(t, i - 1));
    let mut mssr = a[a.child(t, i - 1)].modifier;

    let mut cl = Some(a.child(t, i));
    let mut mscl = a[a.child(t, i)].modifier;

    // The head of the chain is its last element; `ih->nxt` is the element below.
    let mut ih = chain.len() - 1;

    let mut first = true;

    while let (Some(s), Some(c)) = (sr, cl) {
        if a.bottom(s, input.vertically) > chain[ih].low {
            ih = ih.checked_sub(1).expect(
                "the sibling chain is seeded with a value that dominates the subtree depth",
            );
        }

        // How far to the left of the right side of sr is the left side of cl?
        let srd = if input.vertically { a[s].w } else { a[s].h };
        let dist = (mssr + a[s].prelim + srd + a[s].margin) - (mscl + a[c].prelim);

        // Pulling the subtree closer is only sound for the topmost pair of contour
        // nodes, hence `first` has to fall on every iteration, moving or not.
        if (first && dist < 0.0) || dist > 0.0 {
            mscl += dist;
            move_subtree(a, t, i, chain[ih].index, dist);
        }

        first = false;

        if let Some(f) = cb.contour_pairs.as_mut() {
            f(a, s, c, dist);
        }

        let sy = a.bottom(s, input.vertically);
        let cy = a.bottom(c, input.vertically);

        if sy <= cy {
            sr = next_right_contour(a, s);
            if let Some(n) = sr {
                mssr += a[n].modifier;
            }
        }

        if sy >= cy {
            cl = next_left_contour(a, c);
            if let Some(n) = cl {
                mscl += a[n].modifier;
            }
        }
    }

    // Set threads and update extreme nodes.
    // In the first case, the current subtree must be taller than the left siblings.
    match (sr, cl) {
        (None, Some(c)) => set_left_thread(a, t, i, c, mscl),
        // In this case, the left siblings must be taller than the current tree.
        (Some(s), None) => set_right_thread(a, t, i, s, mssr),
        _ => {}
    }
}

fn position_root(a: &mut Arena, t: NodeId, vertically: bool) {
    // Position root between children, taking into account their mod.
    let last = a.nchildren(t) - 1;
    let first = a.child(t, 0);
    let last = a.child(t, last);
    let d = if vertically {
        a[last].w - a[t].w
    } else {
        a[last].h - a[t].h
    };
    a[t].prelim =
        (a[first].prelim + a[first].modifier + a[last].prelim + a[last].modifier + d) / 2.0;
}

fn first_walk(a: &mut Arena, input: &LayoutInput, cb: &mut Callbacks, t: NodeId) {
    if a.nchildren(t) == 0 {
        // setting extremes
        a[t].el = Some(t);
        a[t].er = Some(t);
        a[t].msel = 0.0;
        a[t].mser = 0.0;
    } else {
        let c0 = a.child(t, 0);
        first_walk(a, input, cb, c0);

        let mut chain = Vec::new();
        let el = a[c0].el.expect("the first walk sets the extreme left node");
        update_chain(&mut chain, a.bottom(el, input.vertically), 0);

        for i in 1..a.nchildren(t) {
            let ci = a.child(t, i);
            first_walk(a, input, cb, ci);

            let er = a[ci]
                .er
                .expect("the first walk sets the extreme right node");
            let min = a.bottom(er, input.vertically);

            separate(a, input, cb, t, i, &chain);

            update_chain(&mut chain, min, i);
        }

        position_root(a, t, input.vertically);

        // setting extremes
        let last = a.child(t, a.nchildren(t) - 1);
        a[t].el = a[c0].el;
        a[t].msel = a[c0].msel;
        a[t].er = a[last].er;
        a[t].mser = a[last].mser;
    }
}

fn second_walk(
    a: &mut Arena,
    input: &LayoutInput,
    cb: &mut Callbacks,
    t: NodeId,
    modsum_init: f64,
) -> f64 {
    // keep it for the recursive call at the end.
    let modsum = modsum_init + a[t].modifier;
    let d = a[t].prelim + modsum;

    let (xoffset, yoffset) = if input.centeredxy {
        (a[t].w / 2.0, a[t].h / 2.0)
    } else {
        (0.0, 0.0)
    };
    a[t].centeredxy = input.centeredxy;

    let mut best_min;

    if input.vertically {
        a[t].x = d + xoffset;
        a[t].y += yoffset;
        best_min = a[t].x;
    } else {
        a[t].x += xoffset;
        a[t].y = d + yoffset;
        best_min = a[t].y;
    }

    add_child_spacing(a, t);

    if let Some(f) = cb.walk.as_mut() {
        f(a, t);
    }

    for i in 0..a.nchildren(t) {
        let c = a.child(t, i);
        let current_min = second_walk(a, input, cb, c, modsum);
        best_min = if current_min < best_min {
            current_min
        } else {
            best_min
        };
    }

    best_min
}

fn setup_walk(a: &mut Arena, input: &LayoutInput, t: NodeId, level: usize) {
    a[t].level = level;
    // initially the algorithm requires top-left wise coordinates.
    a[t].centeredxy = false;

    // Clear everything the three walks accumulate into, threads included, so that
    // `layout` can be applied more than once to the same tree; the root starts the
    // depth axis over at the origin, `input.x` and `input.y` place it elsewhere.
    a[t].prelim = 0.0;
    a[t].modifier = 0.0;
    a[t].shift = 0.0;
    a[t].change = 0.0;
    a[t].msel = 0.0;
    a[t].mser = 0.0;
    a[t].tl = None;
    a[t].tr = None;
    a[t].el = None;
    a[t].er = None;

    if level == 0 {
        if input.vertically {
            a[t].y = 0.0;
        } else {
            a[t].x = 0.0;
        }
    }

    let nextlevel = level + 1;

    let b = a.bottom(t, input.vertically);

    for i in 0..a.nchildren(t) {
        let child = a.child(t, i);
        a[child].childno = i;
        a[child].parent = Some(t);

        if input.vertically {
            a[child].y = b;
        } else {
            a[child].x = b;
        }

        setup_walk(a, input, child, nextlevel);
    }
}

fn third_walk(a: &mut Arena, t: NodeId, dx: f64, dy: f64) {
    a[t].x -= dx;
    a[t].y -= dy;

    for i in 0..a.nchildren(t) {
        let c = a.child(t, i);
        third_walk(a, c, dx, dy);
    }
}

/// Lays out the tree described by `input`, writing `x` and `y` into every node.
///
/// The drawing is normalized so that the smallest coordinate along the breadth axis ends
/// up at `input.x` / `input.y`. Applying it twice to the same tree is well defined and
/// gives the same coordinates, and `input` is never written to.
pub fn layout(arena: &mut Arena, input: &LayoutInput) {
    layout_with(arena, input, &mut Callbacks::default());
}

/// [`layout`], with the two observation hooks of `treeinput_t`.
pub fn layout_with(arena: &mut Arena, input: &LayoutInput, cb: &mut Callbacks) {
    let root = input.root;

    setup_walk(arena, input, root, 0);
    first_walk(arena, input, cb, root);
    let minbreadth = second_walk(arena, input, cb, root, 0.0);

    // `third_walk` subtracts, so subtracting the minimum breadth coordinate is the
    // counterpart of the reference's `thirdWalk(t, -minX)`: it normalizes the drawing
    // to start at the origin, which `input.x` and `input.y` then offset.
    let dx = (if input.vertically { minbreadth } else { 0.0 }) - input.x;
    let dy = (if input.vertically { 0.0 } else { minbreadth }) - input.y;

    if dx != 0.0 || dy != 0.0 {
        third_walk(arena, root, dx, dy);
    }
}

/// Writes `[x, y, w, h]` per node into `array`, indexed by `idx - 1`.
///
/// `array` has to hold `4 * n` values, where `n` is the number of nodes of the subtree.
pub fn flat_xywh_into(arena: &Arena, t: NodeId, array: &mut [f64]) {
    for i in 0..arena.nchildren(t) {
        flat_xywh_into(arena, arena.child(t, i), array);
    }

    let n = &arena[t];
    let idx = (n.idx - 1) * 4;

    array[idx] = n.x;
    array[idx + 1] = n.y;
    array[idx + 2] = n.w;
    array[idx + 3] = n.h;
}

/// Writes the coordinates of `nodes` into `xy` as `[x_0.., y_0..]`, indexed by `idx - 1`.
///
/// `xy` has to hold `2 * nodes.len()` values.
pub fn flat_xy_into(arena: &Arena, nodes: &[NodeId], xy: &mut [f64]) {
    let n = nodes.len();

    for &id in nodes {
        let node = &arena[id];
        xy[node.idx - 1] = node.x;
        xy[node.idx - 1 + n] = node.y;
    }
}

/// The deepest point of the subtree rooted at `t`, stopping at `to`.
///
/// `found` is set as soon as `to` is met among the descendants, which stops the walk.
pub fn max_bottom(arena: &Arena, t: NodeId, to: NodeId, vertically: bool, found: &mut bool) -> f64 {
    let mut m = arena.bottom(t, vertically);

    let mut i = 0;
    while !*found && i < arena.nchildren(t) {
        let child = arena.child(t, i);
        if child == to {
            *found = true;
            return m;
        }
        let b = max_bottom(arena, child, to, vertically, found);
        m = if b > m { b } else { m };
        i += 1;
    }

    m
}

/// The state threaded through [`max_bottom_between`], the counterpart of `fringemaxbottom_t`.
#[derive(Debug, Clone, Copy)]
pub struct FringeMaxBottom {
    /// The deepest point met so far; the caller seeds it and reads it back.
    pub bottom: f64,
    /// Whether the depth axis is `y` rather than `x`.
    pub vertically: bool,
}

/// The deepest point of everything that lies between `from` and `to` in the drawing.
///
/// Walks the siblings to the right of `from`, then climbs to the parent and repeats, until
/// `to` is met.
pub fn max_bottom_between(arena: &Arena, from: NodeId, to: NodeId, ud: &mut FringeMaxBottom) {
    let p = match arena[from].parent {
        Some(p) => p,
        None => return,
    };

    let mut found = false;

    let mut i = arena[from].childno + 1;
    while !found && i < arena.nchildren(p) {
        let child = arena.child(p, i);

        if child == to {
            found = true;
            break;
        }

        let b = max_bottom(arena, child, to, ud.vertically, &mut found);

        ud.bottom = if b > ud.bottom { b } else { ud.bottom };
        i += 1;
    }

    if !found {
        max_bottom_between(arena, p, to, ud);
    }
}

/// Builds the arena of the flat description used by [`layout_api`].
///
/// Every node gets an invisible "gap" node inserted above it, which is what produces the
/// spacing between levels: node `i` is `nodes[i]` and its gap node is `nodes[i + n]`, so
/// the root of the resulting tree is `nodes[rooti - 1 + n]`.
///
/// # Panics
///
/// If the arrays are shorter than the layouts documented in [`layout_api`].
pub fn reify_flat_chunks(
    n: usize,
    wh: &[f64],
    whg: &[f64],
    children: &[usize],
) -> (Arena, Vec<NodeId>) {
    assert!(
        wh.len() >= 3 * n,
        "`wh` holds w, h and margin for {n} nodes"
    );
    assert!(whg.len() >= 2 * n, "`whg` holds w and h for {n} gap nodes");
    assert!(children.len() >= n, "`children` starts with {n} counts");

    let mut arena = Arena::with_capacity(2 * n);
    let mut nodes = Vec::with_capacity(2 * n);

    for i in 0..n {
        // the node with the content.
        nodes.push(arena.add_node(i + 1, wh[i], wh[i + n], wh[i + 2 * n], false));
    }

    for i in 0..n {
        // the node that separates.
        let gap = arena.add_node(i + 1 + n, whg[i], whg[i + n], 0.0, true);
        arena.push_child(gap, nodes[i]);
        nodes.push(gap);
    }

    let mut nedges = n;

    for i in 0..n {
        for _ in 0..children[i] {
            let c = children[nedges];
            nedges += 1;
            arena.push_child(nodes[i], nodes[c - 1 + n]);
        }
    }

    (arena, nodes)
}

/// Lays out a tree given as parallel arrays, the counterpart of `layout_api`.
///
/// The array layouts, which the C leaves undocumented:
///
/// * `wh` — `3n` values, `[w_0..w_n-1, h_0..h_n-1, margin_0..margin_n-1]`; on return the
///   first `2n` are overwritten with `[x_0.., y_0..]`
/// * `whg` — `2n` values, the width and height of the invisible gap node inserted above
///   every node, which is what produces the spacing between levels
/// * `children` — `n` counts followed by the adjacency, one-based, grouped by parent
/// * `rooti` — the one-based index of the root
///
/// # Panics
///
/// If the arrays are shorter than that, or if `rooti` is out of range.
#[allow(clippy::too_many_arguments)]
pub fn layout_api(
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

    let (mut arena, nodes) = reify_flat_chunks(n, wh, whg, children);

    let root = nodes[rooti - 1 + n];

    let input = LayoutInput {
        root,
        vertically,
        centeredxy,
        x,
        y,
    };

    layout(&mut arena, &input);

    flat_xy_into(&arena, &nodes[..n], wh);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fringe helpers, which the C test suite does not cover.
    ///
    /// ```text
    ///                 root (10x10)
    ///          /            |          \
    ///     A (10x10)     B (10x10)     C (10x10)
    ///      /     \          |
    /// A1(10x10) A2(10x30)  B1 (10x50)
    /// ```
    ///
    /// Laid out top down, the bottoms are root 10, A/B/C 20, A1 30, A2 50, B1 70. The
    /// expected values below are the ones the C build prints for the same tree.
    fn fringe() -> (Arena, [NodeId; 7]) {
        let mut arena = Arena::new();

        let root = arena.add_node(1, 10.0, 10.0, 0.0, false);
        let a = arena.add_node(2, 10.0, 10.0, 0.0, false);
        let b = arena.add_node(3, 10.0, 10.0, 0.0, false);
        let c = arena.add_node(4, 10.0, 10.0, 0.0, false);
        let a1 = arena.add_node(5, 10.0, 10.0, 0.0, false);
        let a2 = arena.add_node(6, 10.0, 30.0, 0.0, false);
        let b1 = arena.add_node(7, 10.0, 50.0, 0.0, false);

        arena.set_children(root, &[a, b, c]);
        arena.set_children(a, &[a1, a2]);
        arena.set_children(b, &[b1]);

        layout(&mut arena, &LayoutInput::new(root));

        (arena, [root, a, b, c, a1, a2, b1])
    }

    #[test]
    fn bottoms() {
        let (arena, [root, a, b, c, a1, a2, b1]) = fringe();
        let bottoms: Vec<f64> = [root, a, b, c, a1, a2, b1]
            .iter()
            .map(|&id| arena.bottom(id, true))
            .collect();
        assert_eq!(bottoms, [10.0, 20.0, 20.0, 20.0, 30.0, 50.0, 70.0]);
    }

    #[test]
    fn max_bottom_stops_at_its_target() {
        let (arena, [root, a, _b, c, _a1, a2, b1]) = fringe();

        let mut found = false;
        assert_eq!(max_bottom(&arena, root, c, true, &mut found), 70.0);
        assert!(found);

        // the walk stops as soon as `to` is a child, before looking any deeper
        let mut found = false;
        assert_eq!(max_bottom(&arena, a, a2, true, &mut found), 30.0);
        assert!(found);

        let mut found = false;
        assert_eq!(max_bottom(&arena, root, b1, true, &mut found), 50.0);
        assert!(found);
    }

    #[test]
    fn max_bottom_of_a_subtree_that_does_not_hold_its_target() {
        let (arena, [_root, _a, b, c, ..]) = fringe();

        let mut found = false;
        assert_eq!(max_bottom(&arena, b, c, true, &mut found), 70.0);
        assert!(!found, "`c` is not below `b`");
    }

    #[test]
    fn max_bottom_between_climbs_until_it_meets_its_target() {
        let (arena, [root, a, b, c, a1, ..]) = fringe();

        let mut ud = FringeMaxBottom {
            bottom: 0.0,
            vertically: true,
        };
        max_bottom_between(&arena, a1, c, &mut ud);
        assert_eq!(ud.bottom, 70.0, "A2 is passed on the way up, then B");

        let mut ud = FringeMaxBottom {
            bottom: 0.0,
            vertically: true,
        };
        max_bottom_between(&arena, a, c, &mut ud);
        assert_eq!(ud.bottom, 70.0, "B lies between A and C");

        let mut ud = FringeMaxBottom {
            bottom: 0.0,
            vertically: true,
        };
        max_bottom_between(&arena, a, b, &mut ud);
        assert_eq!(ud.bottom, 0.0, "nothing lies between two adjacent siblings");

        let mut ud = FringeMaxBottom {
            bottom: 0.0,
            vertically: true,
        };
        max_bottom_between(&arena, root, c, &mut ud);
        assert_eq!(ud.bottom, 0.0, "the root has no sibling to walk");
    }

    #[test]
    fn setup_walk_fills_in_the_bookkeeping() {
        let (arena, [root, a, b, c, a1, a2, b1]) = fringe();

        let levels: Vec<usize> = [root, a, b, c, a1, a2, b1]
            .iter()
            .map(|&id| arena[id].level)
            .collect();
        assert_eq!(levels, [0, 1, 1, 1, 2, 2, 2]);

        let childnos: Vec<usize> = [a, b, c, a1, a2, b1]
            .iter()
            .map(|&id| arena[id].childno)
            .collect();
        assert_eq!(childnos, [0, 1, 2, 0, 1, 0]);

        assert_eq!(arena[root].parent, None);
        assert_eq!(arena[a2].parent, Some(a));
        assert_eq!(arena[b1].parent, Some(b));
    }
}
