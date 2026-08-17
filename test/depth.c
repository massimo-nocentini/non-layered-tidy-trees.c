/*
 * Recursion depth.  `setupWalk`, `firstWalk`, `secondWalk`, `thirdWalk` and `free_tree`
 * all recurse once per level, so the deepest tree that can be laid out is bounded by the
 * thread's stack rather than by memory.  Measured on an 8 MB main-thread stack: 60 000
 * levels is fine, 100 000 segfaults.  Emscripten's default stack is 64 KiB, which fails
 * in the low hundreds -- and `layout_api` doubles the effective depth, because it puts a
 * gap node above every node.
 *
 * The default here is deliberately well inside the limit; pass a depth to probe further.
 *
 *   ./depth [levels]        default 10000
 */

#include "treegen.h"

int main(int argc, char **argv)
{
  int depth = argc > 1 ? atoi(argv[1]) : 10000;

  tree_t *root = tg_chain(1, depth, 10, 10);

  treeinput_t in = {.t = root, .vertically = 1, .centeredxy = 0, .x = 0, .y = 0};
  layout(&in);

  /* every level sits directly below the previous one, so the last node is at 10*(d-1) */
  tree_t *last = root;
  while (last->cs > 0)
    last = last->c[0];

  int ok = last->y == 10.0 * (depth - 1);
  printf("  %s  a chain of %d levels lays out (deepest node at y=%g)\n",
         ok ? "PASS" : "FAIL", depth, last->y);

  free_tree(root);
  return !ok;
}
