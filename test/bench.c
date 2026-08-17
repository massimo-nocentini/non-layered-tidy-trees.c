/*
 * Wall clock over one large tree, which is the workload `simd-plan.md` scopes for.
 *
 * Builds a tree of exactly `n` nodes with `tg_bench_tree` -- the same tree the Rust
 * benchmark builds from the same seed -- lays it out `reps` times and keeps the best run,
 * so that the number reported is the cost of `layout` alone: allocation, `free_tree` and
 * the tree building are all outside the timed region.
 *
 * The checksum is the XOR of the bit patterns of every coordinate, which is exact and
 * order independent: `rust/src/bin/bench.rs` prints the same column, and the two have to
 * agree node for node.
 *
 *   ./bench [reps]              default 5
 *   ./bench [reps] --deep [d]   also a chain of `d` levels; segfaults for large `d`,
 *                               which is the recursion limit this measures
 */

/* `clock_gettime` is POSIX, and `-std=c11` alone does not declare it. */
#define _POSIX_C_SOURCE 199309L

#include <time.h>
#include <string.h>

#include "treegen.h"

static double now_ms(void)
{
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double)ts.tv_sec * 1e3 + (double)ts.tv_nsec / 1e6;
}

static unsigned long long bits(double d)
{
  unsigned long long u;
  memcpy(&u, &d, sizeof u);
  return u;
}

static void row(const char *impl, int n, int vert, double ms, unsigned long long ck)
{
  printf("%-18s %9d %5d %11.2f %11.3f %016llx\n", impl, n, vert, ms * 1e6 / n, ms, ck);
}

static void header(void)
{
  printf("# %-16s %9s %5s %11s %11s %16s\n", "impl", "n", "vert", "ns/node", "ms", "checksum");
}

/*
 * The best of `reps` batches, each of enough iterations to outrun the clock.  The first
 * layout of a freshly built tree also first-touches its pages, so it is a warm-up rather
 * than a measurement; `layout` may be applied repeatedly to one tree, which is what makes
 * this legitimate.
 */
static double best_ms(treeinput_t *in, int reps)
{
  double t0 = now_ms();
  layout(in);
  double warm = now_ms() - t0;

  int inner = warm > 0.0 ? (int)(20.0 / warm) : 1000;
  inner = inner < 1 ? 1 : (inner > 1000 ? 1000 : inner);

  double best = 0.0;
  for (int r = 0; r < reps; r++)
  {
    double s = now_ms();
    for (int i = 0; i < inner; i++)
      layout(in);
    double dt = (now_ms() - s) / inner;
    if (r == 0 || dt < best)
      best = dt;
  }

  return best;
}

static void one(const char *impl, int n, int maxkids, int vert, int reps)
{
  tree_t **nodes = (tree_t **)malloc(sizeof(tree_t *) * (size_t)n);
  tree_t *root = tg_bench_tree(n, maxkids, 20240607UL, nodes);

  treeinput_t in = {.t = root, .vertically = vert, .centeredxy = 0, .x = 0, .y = 0};

  double best = best_ms(&in, reps);

  unsigned long long ck = 0;
  for (int i = 0; i < n; i++)
    ck ^= bits(nodes[i]->x) ^ bits(nodes[i]->y);

  row(impl, n, vert, best, ck);

  free_tree(root);
  free(nodes);
}

/*
 * `layout_api`, which takes its tree as arrays and gives the coordinates back in the same
 * ones -- so every iteration has to restore the inputs first.  That restore is timed too,
 * for every implementation alike, since it is what a caller laying out repeatedly would do.
 */
static void one_api(const char *impl, int n, int maxkids, int vert, int reps)
{
  double *wh = (double *)malloc(sizeof(double) * (size_t)(3 * n));
  double *whg = (double *)malloc(sizeof(double) * (size_t)(2 * n));
  double *pristine = (double *)malloc(sizeof(double) * (size_t)(3 * n));
  int *children = (int *)malloc(sizeof(int) * (size_t)(2 * n));

  tg_bench_arrays(n, maxkids, 20240607UL, wh, whg, children);
  memcpy(pristine, wh, sizeof(double) * (size_t)(3 * n));

  memcpy(wh, pristine, sizeof(double) * (size_t)(3 * n));
  double t0 = now_ms();
  layout_api(n, wh, whg, children, 1, vert, 0, 0.0, 0.0);
  double warm = now_ms() - t0;

  int inner = warm > 0.0 ? (int)(20.0 / warm) : 1000;
  inner = inner < 1 ? 1 : (inner > 1000 ? 1000 : inner);

  double best = 0.0;
  for (int r = 0; r < reps; r++)
  {
    double s = now_ms();
    for (int i = 0; i < inner; i++)
    {
      memcpy(wh, pristine, sizeof(double) * (size_t)(3 * n));
      layout_api(n, wh, whg, children, 1, vert, 0, 0.0, 0.0);
    }
    double dt = (now_ms() - s) / inner;
    if (r == 0 || dt < best)
      best = dt;
  }

  unsigned long long ck = 0;
  for (int i = 0; i < 2 * n; i++)
    ck ^= bits(wh[i]);

  row(impl, n, vert, best, ck);

  free(wh);
  free(whg);
  free(pristine);
  free(children);
}

int main(int argc, char **argv)
{
  int reps = argc > 1 && argv[1][0] != '-' ? atoi(argv[1]) : 5;

  int deep = 0;
  for (int i = 1; i < argc; i++)
    if (strcmp(argv[i], "--deep") == 0)
      deep = i + 1 < argc ? atoi(argv[i + 1]) : 10000;

  const int sizes[] = {1000, 10000, 100000, 1000000};

  header();

  for (unsigned s = 0; s < sizeof sizes / sizeof *sizes; s++)
    for (int vert = 1; vert >= 0; vert--)
      one("c", sizes[s], 4, vert, reps);

  for (unsigned s = 0; s < sizeof sizes / sizeof *sizes; s++)
    for (int vert = 1; vert >= 0; vert--)
      one_api("c-api", sizes[s], 4, vert, reps);

  if (deep > 0)
  {
    tree_t *root = tg_chain(1, deep, 10, 10);
    treeinput_t in = {.t = root, .vertically = 1, .centeredxy = 0, .x = 0, .y = 0};

    double best = best_ms(&in, reps);

    unsigned long long ck = 0;
    for (tree_t *t = root; t != NULL; t = t->cs > 0 ? t->c[0] : NULL)
      ck ^= bits(t->x) ^ bits(t->y);

    row("c-chain", deep, 1, best, ck);
    free_tree(root);
  }

  return 0;
}
