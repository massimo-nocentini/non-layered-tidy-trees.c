/*
 * Invariants that hold for every input, checked over random trees:
 *
 *   normalization  the smallest breadth coordinate ends up at `input->x` / `input->y`,
 *                  which is the C counterpart of the reference's `thirdWalk(t, -minX)`
 *   repeatability  `layout` may be applied twice, to the same tree and through the same
 *                  `treeinput_t`, without the result drifting or the input being written
 *   axes           `vertically` and `centeredxy` are truth values, not enumerations:
 *                  any nonzero value has to behave like 1
 *   centering      `centeredxy` shifts coordinates to box centres without moving boxes
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

static void normalization(void)
{
  tree_t *nodes[TG_MAXNODES];
  int bad = 0, offbad = 0, total = 0;
  double worst = 0.0;

  for (int regime = 0; regime < TG_REGIMES; regime++)
    for (int vert = 0; vert < 2; vert++)
      for (int cent = 0; cent < 2; cent++)
        for (int k = 0; k < 150; k++)
        {
          tree_t *r = tg_build(4242UL + (unsigned long)k, 4, 3, regime);
          treeinput_t in = {.t = r, .vertically = vert, .centeredxy = cent, .x = 0, .y = 0};
          layout(&in);
          int n = tg_collect(r, nodes, TG_MAXNODES);

          double m = tg_min_breadth(nodes, n, vert);
          total++;
          if (!close_enough(m, 0.0))
          {
            bad++;
            if (m < worst) worst = m;
          }
          free_tree(r);

          /* the same tree, asked to start somewhere else */
          r = tg_build(4242UL + (unsigned long)k, 4, 3, regime);
          treeinput_t off = {.t = r, .vertically = vert, .centeredxy = cent, .x = 100, .y = 7};
          layout(&off);
          n = tg_collect(r, nodes, TG_MAXNODES);
          if (!close_enough(tg_min_breadth(nodes, n, vert), vert ? 100.0 : 7.0))
            offbad++;
          free_tree(r);
        }

  printf("    (%d layouts checked, worst deviation %g)\n", total, worst);
  check(bad == 0, "the drawing starts at the origin");
  check(offbad == 0, "`input->x` / `input->y` offset the normalized origin");
}

static void repeatability(void)
{
  tree_t *r = tg_build(99UL, 4, 3, TG_VARIED);
  treeinput_t in = {.t = r, .vertically = 1, .centeredxy = 0, .x = 0, .y = 0};

  layout(&in);
  tree_t *nodes[TG_MAXNODES];
  int n = tg_collect(r, nodes, TG_MAXNODES);
  double *first = malloc(sizeof(double) * (size_t)n * 2);
  for (int i = 0; i < n; i++)
  {
    first[i] = nodes[i]->x;
    first[i + n] = nodes[i]->y;
  }

  double x0 = in.x, y0 = in.y;

  layout(&in);
  layout(&in);

  int drift = 0;
  for (int i = 0; i < n; i++)
    if (nodes[i]->x != first[i] || nodes[i]->y != first[i + n])
      drift++;

  check(drift == 0, "repeated layouts of one tree give identical coordinates");
  check(in.x == x0 && in.y == y0, "`layout` does not write into its `treeinput_t`");

  free(first);
  free_tree(r);
}

static void axes(void)
{
  tree_t *nodes_a[TG_MAXNODES], *nodes_b[TG_MAXNODES];
  int mismatch = 0;

  for (int regime = 0; regime < TG_REGIMES; regime++)
    for (int k = 0; k < 50; k++)
    {
      /* `2` and `-1` are truthy; they must lay out exactly like `1` */
      static const int truthy[] = {1, 2, -1};
      tree_t *a = tg_build(7UL + (unsigned long)k, 4, 3, regime);
      treeinput_t ia = {.t = a, .vertically = 1, .centeredxy = 1, .x = 0, .y = 0};
      layout(&ia);
      int n = tg_collect(a, nodes_a, TG_MAXNODES);

      for (unsigned v = 1; v < sizeof truthy / sizeof *truthy; v++)
      {
        tree_t *b = tg_build(7UL + (unsigned long)k, 4, 3, regime);
        treeinput_t ib = {.t = b, .vertically = truthy[v], .centeredxy = truthy[v], .x = 0, .y = 0};
        layout(&ib);
        tg_collect(b, nodes_b, TG_MAXNODES);
        for (int i = 0; i < n; i++)
          if (nodes_a[i]->x != nodes_b[i]->x || nodes_a[i]->y != nodes_b[i]->y)
            mismatch++;
        free_tree(b);
      }
      free_tree(a);
    }

  check(mismatch == 0, "any nonzero `vertically` / `centeredxy` behaves like 1");
}

/*
 * `centeredxy` must move the coordinates to the middle of each box without disturbing
 * the drawing.  The boxes do not land in the same absolute place though: normalization
 * works on the coordinate it is given, so in centred mode it puts the leftmost *centre*
 * at the origin and the leftmost edge ends up half a node further left.  The drawing is
 * therefore preserved up to one rigid translation along the breadth axis -- which is the
 * property worth pinning down, since a per-node discrepancy would be a real bug.
 */
static void centering(void)
{
  tree_t *plain[TG_MAXNODES], *cent[TG_MAXNODES];
  int mismatch = 0, depthmoved = 0;
  double sample = 0.0;

  for (int vert = 0; vert < 2; vert++)
    for (int k = 0; k < 100; k++)
    {
      tree_t *a = tg_build(31UL + (unsigned long)k, 4, 3, TG_VARIED);
      treeinput_t ia = {.t = a, .vertically = vert, .centeredxy = 0, .x = 0, .y = 0};
      layout(&ia);
      int n = tg_collect(a, plain, TG_MAXNODES);

      tree_t *b = tg_build(31UL + (unsigned long)k, 4, 3, TG_VARIED);
      treeinput_t ib = {.t = b, .vertically = vert, .centeredxy = 1, .x = 0, .y = 0};
      layout(&ib);
      tg_collect(b, cent, TG_MAXNODES);

      double breadth0 = vert ? tg_left(cent[0]) - tg_left(plain[0])
                             : tg_top(cent[0]) - tg_top(plain[0]);
      if (k == 0 && vert == 1)
        sample = breadth0;

      for (int i = 0; i < n; i++)
      {
        double dl = tg_left(cent[i]) - tg_left(plain[i]);
        double dt = tg_top(cent[i]) - tg_top(plain[i]);
        double breadth = vert ? dl : dt, depth = vert ? dt : dl;

        if (!close_enough(breadth, breadth0))
          mismatch++;
        if (!close_enough(depth, 0.0))
          depthmoved++;
      }

      free_tree(a);
      free_tree(b);
    }

  printf("    (centring shifts the sample drawing by %g along the breadth axis)\n", sample);
  check(mismatch == 0, "`centeredxy` preserves the drawing up to one rigid translation");
  check(depthmoved == 0, "`centeredxy` leaves the depth axis where it was");
}

int main(void)
{
  printf("properties\n");
  normalization();
  repeatability();
  axes();
  centering();
  return failures != 0;
}
