/*
 * The two callbacks in `treeinput_t`.
 *
 *   `cpairscb` observes every pair of contour nodes `separate` compares, which is the
 *   only window onto the contour walk from outside the library; the expected trace for
 *   a hand-built tree is spelled out below.
 *
 *   `walkcb` observes every node during `secondWalk`.  It is handed a mutable `tree_t *`,
 *   and the minimum used for normalization is now read *before* it runs -- so a callback
 *   that moves a node no longer feeds its own edit back into the normalization.  That is
 *   deliberate; this test documents it.
 */

#include "treegen.h"

static int failures;

static void check(int ok, const char *what)
{
  printf("  %s  %s\n", ok ? "PASS" : "FAIL", what);
  if (!ok)
    failures++;
}

static int pairs;

static void trace(tree_t *sr, tree_t *cl, double dist, void *ud)
{
  (void)ud;
  pairs++;
  printf("    contour pair: #%d (bottom %g) against #%d (bottom %g), dist %g\n",
         sr->idx, bottom(sr, 1), cl->idx, bottom(cl, 1), dist);
}

static void contour_pairs(void)
{
  /* root over a shallow-left/deep-right subtree and a deep chain */
  tree_t *root = init_tree(1, 10, 10, 0, 2, 0);
  tree_t *A = init_tree(2, 10, 10, 0, 2, 0);
  A->c[0] = init_tree(4, 10, 10, 0, 0, 0);
  A->c[1] = tg_chain(5, 3, 10, 10);
  root->c[0] = A;
  root->c[1] = tg_chain(8, 4, 10, 10);

  pairs = 0;
  treeinput_t in = {.t = root, .vertically = 1, .centeredxy = 0, .cpairscb = trace, .x = 0, .y = 0};
  layout(&in);

  /* one pair inside A, then four while separating A from the right subtree */
  check(pairs == 5, "cpairscb sees every contour pair (5 for this tree)");
  free_tree(root);
}

static int visited;

static void counter(tree_t *t, void *ud)
{
  (void)t;
  (*(int *)ud)++;
  visited++;
}

static void mover(tree_t *t, void *ud)
{
  (void)ud;
  if (t->idx == 1)
    t->x -= 1000.0; // a callback that moves the node it was handed
}

static void walk_callback(void)
{
  tree_t *r = tg_build(5UL, 4, 3, TG_VARIED);
  tree_t *nodes[TG_MAXNODES];
  int n = tg_collect(r, nodes, TG_MAXNODES);

  int through_ud = 0;
  visited = 0;
  treeinput_t in = {.t = r, .vertically = 1, .centeredxy = 0,
                    .walkcb = counter, .walkud = &through_ud, .x = 0, .y = 0};
  layout(&in);

  check(visited == n, "walkcb is called once per node");
  check(through_ud == n, "walkud is passed through untouched");
  free_tree(r);

  /* three leaves under a root: children at 0, 10, 20 and the root centred at 10 */
  tree_t *m = init_tree(1, 10, 10, 0, 3, 0);
  for (int i = 0; i < 3; i++)
    m->c[i] = init_tree(i + 2, 10, 10, 0, 0, 0);

  treeinput_t im = {.t = m, .vertically = 1, .centeredxy = 0, .walkcb = mover, .x = 0, .y = 0};
  layout(&im);

  check(m->c[0]->x == 0.0 && m->c[1]->x == 10.0 && m->c[2]->x == 20.0,
        "a coordinate-mutating walkcb does not perturb normalization");
  free_tree(m);
}

int main(void)
{
  printf("callbacks\n");
  contour_pairs();
  walk_callback();
  return failures != 0;
}
