/*
 * The headline property of a tidy drawing: no two node boxes overlap.
 *
 * Sweeps size regimes x depths x branching factors x orientations x centeredxy and
 * checks every pair of nodes in every tree.  With `--dump` it prints the coordinates
 * instead, which turns the same corpus into a differential oracle: build two revisions
 * of the library against this file and diff their dumps.  A pure translation of the
 * whole drawing shows up as a constant offset, anything else is a real change.
 *
 *   ./overlap [trials-per-shape]      default 300, ~57600 trees
 *   ./overlap --dump [trials]         coordinates on stdout
 */

#include "treegen.h"

int main(int argc, char **argv)
{
  int dump = argc > 1 && argv[1][0] == '-';
  int trials = argc > (dump ? 2 : 1) ? atoi(argv[dump ? 2 : 1]) : 300;

  tree_t *nodes[TG_MAXNODES];
  long collisions = 0, trees = 0;

  for (int regime = 0; regime < TG_REGIMES; regime++)
    for (int maxdepth = 1; maxdepth <= 6; maxdepth++)
      for (int maxkids = 2; maxkids <= 5; maxkids++)
        for (int vert = 0; vert < 2; vert++)
          for (int cent = 0; cent < 2; cent++)
            for (int k = 0; k < trials; k++)
            {
              unsigned long seed = 1000003UL * (unsigned long)k +
                                   7UL * (unsigned long)(regime * 100 + maxdepth * 10 + maxkids);
              tree_t *r = tg_build(seed, maxdepth, maxkids, regime);

              treeinput_t in = {.t = r, .vertically = vert, .centeredxy = cent, .x = 0, .y = 0};
              layout(&in);

              int n = tg_collect(r, nodes, TG_MAXNODES);
              trees++;

              if (dump)
              {
                printf("# regime=%d depth=%d kids=%d vert=%d cent=%d k=%d n=%d\n",
                       regime, maxdepth, maxkids, vert, cent, k, n);
                for (int i = 0; i < n; i++)
                  printf("%d %.17g %.17g\n", nodes[i]->idx, nodes[i]->x, nodes[i]->y);
              }
              else
              {
                for (int i = 0; i < n; i++)
                  for (int j = i + 1; j < n; j++)
                    if (tg_overlap(nodes[i], nodes[j]))
                    {
                      collisions++;
                      if (collisions <= 10)
                        fprintf(stderr,
                                "  overlap: regime=%d depth=%d kids=%d vert=%d cent=%d k=%d seed=%lu"
                                " -- #%d [%g,%g]x[%g,%g] vs #%d [%g,%g]x[%g,%g]\n",
                                regime, maxdepth, maxkids, vert, cent, k, seed,
                                nodes[i]->idx, tg_left(nodes[i]), tg_left(nodes[i]) + nodes[i]->w,
                                tg_top(nodes[i]), tg_top(nodes[i]) + nodes[i]->h,
                                nodes[j]->idx, tg_left(nodes[j]), tg_left(nodes[j]) + nodes[j]->w,
                                tg_top(nodes[j]), tg_top(nodes[j]) + nodes[j]->h);
                    }
              }

              free_tree(r);
            }

  if (dump)
    return 0;

  fprintf(stderr, "%s overlap: %ld overlapping pairs over %ld trees\n",
          collisions == 0 ? "PASS" : "FAIL", collisions, trees);
  return collisions != 0;
}
