/*
 * Hand-built trees that pin down bugs which were actually shipped at some point.
 * Each case is small enough to reason about on paper, and each one failed before the
 * corresponding fix landed.
 */

#include "treegen.h"

static int failures;

static void check(int ok, const char *what)
{
  printf("  %s  %s\n", ok ? "PASS" : "FAIL", what);
  if (!ok)
    failures++;
}

/*
 *              root (30x10)
 *             /            \
 *      A (20x10)            B (20x30)
 *          |               /        \
 *      A1 (10x20)   B1 (30x10)   B2 (20x10)
 *
 * `A.prelim` is -5 and `B.prelim` is +15, so the first contour pair (A against B) has
 * `dist == 0.0` exactly: the two subtrees already touch.  The second pair (A1 against B)
 * has `dist == -5`.  Applying the "pull the subtree closer" correction there -- which is
 * what happens when `separate` clears its `first` flag only on iterations that move
 * something -- drags B five units into A.
 */
static void first_flag_overlap(void)
{
  tree_t *root = init_tree(1, 30, 10, 0, 2, 0);
  tree_t *A = init_tree(2, 20, 10, 0, 1, 0);
  tree_t *A1 = init_tree(3, 10, 20, 0, 0, 0);
  tree_t *B = init_tree(4, 20, 30, 0, 2, 0);
  tree_t *B1 = init_tree(5, 30, 10, 0, 0, 0);
  tree_t *B2 = init_tree(6, 20, 10, 0, 0, 0);

  root->c[0] = A; root->c[1] = B;
  A->c[0] = A1;
  B->c[0] = B1; B->c[1] = B2;

  treeinput_t in = {.t = root, .vertically = 1, .centeredxy = 0, .x = 0, .y = 0};
  layout(&in);

  check(!tg_overlap(A, B), "sibling subtrees do not overlap (separate's `first` flag)");

  tree_t *nodes[TG_MAXNODES];
  int n = tg_collect(root, nodes, TG_MAXNODES);
  check(tg_min_breadth(nodes, n, 1) == 0.0, "the same tree is normalized to x = 0");

  free_tree(root);
}

/*
 * The first child's left contour is shallow while its right contour is deep, and the
 * second child is deep enough that the contour walk keeps going.  This is the shape that
 * would run the `ih` chain off its end if `firstWalk` seeded it with a value that did not
 * dominate the subtree's depth -- it does not, because threading makes `bottom(el)` and
 * `bottom(er)` both equal to the deepest point of the subtree.
 */
static void asymmetric_contours(void)
{
  tree_t *root = init_tree(1, 10, 10, 0, 2, 0);
  tree_t *A = init_tree(2, 10, 10, 0, 2, 0);
  A->c[0] = init_tree(4, 10, 10, 0, 0, 0); // shallow left contour
  A->c[1] = tg_chain(5, 3, 10, 10);        // deep right contour
  root->c[0] = A;
  root->c[1] = tg_chain(8, 4, 10, 10);

  treeinput_t in = {.t = root, .vertically = 1, .centeredxy = 0, .x = 0, .y = 0};
  layout(&in); // used to be able to dereference a NULL chain link

  tree_t *nodes[TG_MAXNODES];
  int n = tg_collect(root, nodes, TG_MAXNODES);
  int overlapping = 0;
  for (int i = 0; i < n; i++)
    for (int j = i + 1; j < n; j++)
      overlapping += tg_overlap(nodes[i], nodes[j]);

  check(overlapping == 0, "asymmetric contours lay out without overlaps");
  free_tree(root);
}

int main(void)
{
  printf("regression cases\n");
  first_flag_overlap();
  asymmetric_contours();
  return failures != 0;
}
