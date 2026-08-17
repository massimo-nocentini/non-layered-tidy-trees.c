/*
 * `layout_api`, the flat entry point the FFI and WebAssembly bindings call.
 *
 * Its arguments are three parallel arrays and no documentation, so the layouts are
 * spelled out here:
 *
 *   wh        3n doubles: [w_0..w_n-1, h_0..h_n-1, margin_0..margin_n-1]
 *             on return the first 2n are overwritten with [x_0.., y_0..]
 *   whg       2n doubles: the width and height of the invisible "gap" node inserted
 *             above every node, which is what produces the spacing between levels
 *   children  n counts followed by the adjacency, one-based, grouped by parent
 *   rooti     one-based index of the root
 *
 * Run this under LeakSanitizer: `layout_api` owns everything it allocates.
 */

#include "treegen.h"

static int failures;

static void check(int ok, const char *what)
{
  printf("  %s  %s\n", ok ? "PASS" : "FAIL", what);
  if (!ok)
    failures++;
}

static int close_enough(double a, double b) { return (a - b) < 1e-9 && (b - a) < 1e-9; }

int main(void)
{
  printf("layout_api\n");

  /* a root with two leaves, every node 10x10, five units of vertical gap */
  int n = 3;
  double wh[9] = {10, 10, 10, /* h */ 10, 10, 10, /* margin */ 0, 0, 0};
  double whg[6] = {10, 10, 10, /* h */ 5, 5, 5};
  int children[5] = {2, 0, 0, /* adjacency */ 2, 3};

  layout_api(n, wh, whg, children, 1, 1, 0, 0.0, 0.0);

  for (int i = 0; i < n; i++)
    printf("    node %d: x=%g y=%g\n", i + 1, wh[i], wh[i + n]);

  check(close_enough(wh[0], 5) && close_enough(wh[3], 5), "root sits between its children, below the gap");
  check(close_enough(wh[1], 0) && close_enough(wh[2], 10), "the leaves are side by side");
  check(close_enough(wh[4], 20) && close_enough(wh[5], 20), "the leaves share a level");

  /* the same tree again, to prove nothing is retained between calls */
  double wh2[9] = {10, 10, 10, 10, 10, 10, 0, 0, 0};
  double whg2[6] = {10, 10, 10, 5, 5, 5};
  int children2[5] = {2, 0, 0, 2, 3};
  layout_api(n, wh2, whg2, children2, 1, 1, 0, 0.0, 0.0);

  int same = 1;
  for (int i = 0; i < 2 * n; i++)
    same = same && close_enough(wh[i], wh2[i]);
  check(same, "a second call gives the same answer");

  /* horizontal, centred, and offset */
  double wh3[9] = {10, 10, 10, 10, 10, 10, 0, 0, 0};
  double whg3[6] = {5, 5, 5, 10, 10, 10};
  int children3[5] = {2, 0, 0, 2, 3};
  layout_api(n, wh3, whg3, children3, 1, 0, 1, 100.0, 50.0);
  printf("    horizontal+centred+offset root: x=%g y=%g\n", wh3[0], wh3[3]);
  check(wh3[0] >= 100.0 && wh3[3] >= 50.0, "the offset is honoured in horizontal mode");

  return failures != 0;
}
