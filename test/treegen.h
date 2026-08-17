#ifndef TREEGEN_H
#define TREEGEN_H

/*
 * Shared helpers for the test programs: a deterministic generator of random trees
 * and a few geometric predicates.  Everything is `static inline` so that each test
 * is a single translation unit linked against the library sources directly.
 */

#include <stdio.h>
#include <stdlib.h>

#include "../src/non-layered-tidy-trees.h"

#define TG_MAXNODES 8192

/* A deterministic PRNG, so that a failing case is reproducible from its seed alone. */

static unsigned long tg_state = 1;

static inline void tg_srand(unsigned long s) { tg_state = s; }

static inline unsigned tg_rand(unsigned n)
{
  tg_state = tg_state * 6364136223846793005UL + 1442695040888963407UL;
  return (unsigned)((tg_state >> 33) % n);
}

/*
 * Size regimes.  `TG_COARSE` matters more than it looks: multiples of ten make
 * exactly-touching siblings (a contour pair with `dist == 0.0`) common, which is the
 * regime that exposed the `first` flag bug in `separate`.
 */
enum
{
  TG_COARSE = 0,  // multiples of ten, no margin
  TG_UNIFORM = 1, // every node identical
  TG_VARIED = 2,  // wide spread of sizes, with margins
  TG_TINY = 3,    // small integers
  TG_REGIMES = 4
};

static int tg_idx;

static inline tree_t *tg_build_rec(int depth, int maxdepth, int maxkids, int regime)
{
  int cs = depth >= maxdepth ? 0 : (int)tg_rand((unsigned)maxkids + 1);

  double w, h, m;
  switch (regime)
  {
  case TG_UNIFORM:
    w = 10.0; h = 10.0; m = 0.0;
    break;
  case TG_VARIED:
    w = 5 + tg_rand(40); h = 5 + tg_rand(30); m = tg_rand(6);
    break;
  case TG_TINY:
    w = 1 + tg_rand(4); h = 1 + tg_rand(4); m = tg_rand(2);
    break;
  case TG_COARSE:
  default:
    w = 10 * (1 + tg_rand(3)); h = 10 * (1 + tg_rand(3)); m = 0.0;
    break;
  }

  tree_t *t = init_tree(++tg_idx, w, h, m, cs, 0);
  for (int i = 0; i < cs; i++)
    t->c[i] = tg_build_rec(depth + 1, maxdepth, maxkids, regime);
  return t;
}

static inline tree_t *tg_build(unsigned long seed, int maxdepth, int maxkids, int regime)
{
  tg_srand(seed);
  tg_idx = 0;
  return tg_build_rec(0, maxdepth, maxkids, regime);
}

/* A chain of `depth` nodes, each the only child of the previous one. */
static inline tree_t *tg_chain(int idx, int depth, double w, double h)
{
  tree_t *t = init_tree(idx, w, h, 0.0, depth > 1 ? 1 : 0, 0);
  if (depth > 1)
    t->c[0] = tg_chain(idx + 1, depth - 1, w, h);
  return t;
}

static inline int tg_collect_rec(tree_t *t, tree_t **out, int n, int cap)
{
  if (n < cap)
    out[n++] = t;
  for (int i = 0; i < t->cs; i++)
    n = tg_collect_rec(t->c[i], out, n, cap);
  return n;
}

/* Gathers the nodes into `out` in preorder, returning how many were stored. */
static inline int tg_collect(tree_t *t, tree_t **out, int cap)
{
  return tg_collect_rec(t, out, 0, cap);
}

/*
 * Box edges.  `centeredxy` relates `x`/`y` to the middle of the node rather than to
 * its top left corner, so the edges have to be derived rather than read.
 */

static inline double tg_left(tree_t *t) { return t->centeredxy == 1 ? t->x - t->w / 2.0 : t->x; }
static inline double tg_top(tree_t *t) { return t->centeredxy == 1 ? t->y - t->h / 2.0 : t->y; }

/* Strict overlap: boxes that merely share an edge are tidy, boxes that share area are not. */
static inline int tg_overlap(tree_t *a, tree_t *b)
{
  const double e = 1e-9;
  double al = tg_left(a), bl = tg_left(b), at = tg_top(a), bt = tg_top(b);
  return (al + a->w - e > bl && bl + b->w - e > al) &&
         (at + a->h - e > bt && bt + b->h - e > at);
}

/* The smallest coordinate along the breadth axis, which `layout` normalizes. */
static inline double tg_min_breadth(tree_t **v, int n, int vertically)
{
  double m = vertically != 0 ? v[0]->x : v[0]->y;
  for (int i = 1; i < n; i++)
  {
    double c = vertically != 0 ? v[i]->x : v[i]->y;
    if (c < m)
      m = c;
  }
  return m;
}

/* The smallest edge along each axis, i.e. the corner of the drawing's bounding box. */
static inline void tg_min_edges(tree_t **v, int n, double *left, double *top)
{
  *left = tg_left(v[0]);
  *top = tg_top(v[0]);
  for (int i = 1; i < n; i++)
  {
    if (tg_left(v[i]) < *left)
      *left = tg_left(v[i]);
    if (tg_top(v[i]) < *top)
      *top = tg_top(v[i]);
  }
}

#endif /* TREEGEN_H */
