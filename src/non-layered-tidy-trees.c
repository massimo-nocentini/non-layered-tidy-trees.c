
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include "non-layered-tidy-trees.h"

/*
 * A linked list of the indexes of left siblings and their lowest vertical coordinate.
 */
typedef struct chain_s
{
  double low;
  int index;
  struct chain_s *nxt;
} chain_t;

void free_chain(chain_t *c)
{
  if (c->nxt != NULL)
    free_chain(c->nxt);
  free(c);
}

chain_t *update_chain(double min, int i, chain_t *init)
{

  chain_t *ih = init;

  while (ih != NULL && min >= ih->low)
  {
    chain_t *nxt = ih->nxt;
    free(ih);
    ih = nxt;
  }

  chain_t *out = (chain_t *)malloc(sizeof(chain_t));
  out->low = min;
  out->index = i;
  out->nxt = ih;

  return out;
}

// Process change and shift to add intermediate spacing to mod.
void addChildSpacing(tree_t *t)
{
  double d = 0.0, modsumdelta = 0.0;
  for (int i = 0; i < t->cs; i++)
  {
    d += t->c[i]->shift;
    modsumdelta += d + t->c[i]->change;
    t->c[i]->mod += modsumdelta;
  }
}

double bottom(tree_t *t, int vertically)
{
  return vertically != 0 ? t->y + (t->centeredxy == 1 ? t->h / 2.0 : t->h) : t->x + (t->centeredxy == 1 ? t->w / 2.0 : t->w);
}

void moveSubtree(tree_t *t, int i, int si, double dist)
{
  t->c[i]->mod += dist;
  t->c[i]->msel += dist;
  t->c[i]->mser += dist;

  if (si != i - 1) // Are there intermediate children?
  {
    double nr = i - si;
    double ratio = dist / nr;
    t->c[si + 1]->shift += ratio;
    t->c[i]->shift -= ratio;
    t->c[i]->change -= dist - ratio;
  }
}

tree_t *nextLeftContour(tree_t *t) { return t->cs == 0 ? t->tl : t->c[0]; }

tree_t *nextRightContour(tree_t *t) { return t->cs == 0 ? t->tr : t->c[t->cs - 1]; }

void setLeftThread(tree_t *t, int i, tree_t *cl, double modsumcl)
{
  tree_t *li = t->c[0]->el;
  li->tl = cl;

  // Change mod so that the sum of modifier after following thread is correct.
  double diff = (modsumcl - cl->mod) - t->c[0]->msel;
  li->mod += diff;

  // Change preliminary x coordinate so that the node does not move.
  li->prelim -= diff;

  // Update extreme node and its sum of modifiers.
  t->c[0]->el = t->c[i]->el;
  t->c[0]->msel = t->c[i]->msel;
}

// Symmetrical to `setLeftThread`.
void setRightThread(tree_t *t, int i, tree_t *sr, double modsumsr)
{
  tree_t *ri = t->c[i]->er;
  ri->tr = sr;
  double diff = (modsumsr - sr->mod) - t->c[i]->mser;
  ri->mod += diff;
  ri->prelim -= diff;
  t->c[i]->er = t->c[i - 1]->er;
  t->c[i]->mser = t->c[i - 1]->mser;
}

void separate(treeinput_t *input, tree_t *t, int i, chain_t *init)
{

  tree_t *sr = t->c[i - 1];
  double mssr = sr->mod;

  tree_t *cl = t->c[i];
  double mscl = cl->mod;

  chain_t *ih = init;

  for (int iterations = 0; sr != NULL && cl != NULL; iterations++)
  {

    if (bottom(sr, input->vertically) > ih->low)
      ih = ih->nxt;

    // How far to the left of the right side of sr is the left side of cl?
    double srd = input->vertically != 0 ? sr->w : sr->h;
    double dist = (mssr + sr->prelim + srd + sr->margin) - (mscl + cl->prelim);

    if (dist > 0.0 || (iterations == 0 && dist < 0.0))
    {
      mscl += dist;
      moveSubtree(t, i, ih->index, dist);
    }

    if (input->cpairscb != NULL)
      input->cpairscb(sr, cl, dist, input->cpairsud);

    double sy = bottom(sr, input->vertically);
    double cy = bottom(cl, input->vertically);

    if (sy <= cy)
    {
      sr = nextRightContour(sr);
      if (sr != NULL)
        mssr += sr->mod;
    }

    if (sy >= cy)
    {
      cl = nextLeftContour(cl);
      if (cl != NULL)
        mscl += cl->mod;
    }
  }

  // Set threads and update extreme nodes.
  // In the first case, the current subtree must be taller than the left siblings.
  if (sr == NULL && cl != NULL)
    setLeftThread(t, i, cl, mscl);
  // In this case, the left siblings must be taller than the current tree.
  else if (sr != NULL && cl == NULL)
    setRightThread(t, i, sr, mssr);
}

void positionRoot(tree_t *t, int vertically)
{
  // Position root between children, taking into account their mod.
  int last = t->cs - 1;
  double d = vertically != 0 ? t->c[last]->w - t->w : t->c[last]->h - t->h;
  t->prelim = (t->c[0]->prelim + t->c[0]->mod + t->c[last]->prelim + t->c[last]->mod + d) / 2.0;
}

void firstWalk(treeinput_t *input, tree_t *t)
{
  if (t->cs == 0)
  {
    // setting extremes
    t->el = t;
    t->er = t;
    t->msel = t->mser = 0.0;
  }
  else
  {
    firstWalk(input, t->c[0]);

    chain_t *ih = update_chain(bottom(t->c[0]->el, input->vertically), 0, NULL);

    for (int i = 1; i < t->cs; i++)
    {

      firstWalk(input, t->c[i]);

      double min = bottom(t->c[i]->er, input->vertically);

      separate(input, t, i, ih);

      ih = update_chain(min, i, ih);
    }

    free_chain(ih);

    positionRoot(t, input->vertically);

    // setting extremes
    t->el = t->c[0]->el;
    t->msel = t->c[0]->msel;
    t->er = t->c[t->cs - 1]->er;
    t->mser = t->c[t->cs - 1]->mser;
  }
}

void secondWalk(treeinput_t *input, tree_t *t, double modsum_init)
{

  double modsum = modsum_init + t->mod; // keep it for the recursive call at the end.
  double d = t->prelim + modsum;

  double xoffset, yoffset;

  if (input->centeredxy == 1)
  {
    xoffset = t->w / 2.0;
    yoffset = t->h / 2.0;
    t->centeredxy = 1;
  }
  else
  {
    xoffset = 0.0;
    yoffset = 0.0;
    t->centeredxy = 0;
  }

  if (input->vertically == 1)
  {
    t->x = d + xoffset;
    t->y += yoffset;
  }
  else
  {
    t->x += xoffset;
    t->y = d + yoffset;
  }

  addChildSpacing(t);

  if (input->walkcb != NULL)
    input->walkcb(t, input->walkud);

  for (int i = 0; i < t->cs; i++)
    secondWalk(input, t->c[i], modsum);
}

void setupWalk(treeinput_t *input, tree_t *t, int level)
{

  t->level = level;
  t->centeredxy = 0; // initially the algorithm requires top-left wise coordinates.

  int nextlevel = level + 1;

  double b = bottom(t, input->vertically);

  for (int i = 0; i < t->cs; i++)
  {
    tree_t *child = t->c[i];
    child->childno = i;
    child->p = t;

    if (input->vertically == 1)
      child->y += b;
    else
      child->x += b;

    setupWalk(input, child, nextlevel);
  }
}

void thirdWalk(treeinput_t *input, tree_t *t, double dx, double dy)
{
  t->x -= dx;
  t->y -= dy;

  for (int i = 0; i < t->cs; i++)
    thirdWalk(input, t->c[i], dx, dy);
}

void layout(treeinput_t *input)
{
  setupWalk(input, input->t, 0);
  firstWalk(input, input->t);
  secondWalk(input, input->t, 0.0);

  if (input->x != 0 && input->y != 0)
  {
    double dx, dy;

    dx = input->t->x - input->x;
    dy = input->t->y - input->y;

    thirdWalk(input, input->t, dx, dy);
  }
}

void free_tree(tree_t *t)
{
  t->el = t->er = t->tl = t->tr = t->p = NULL; // disconnect some links.

  for (int i = 0; i < t->cs; i++)
    free_tree(t->c[i]);

  free(t->c);
  free(t);
}

tree_t *init_tree(int idx, double w, double h, double m, int cs, int isdummy)
{
  tree_t *t = (tree_t *)malloc(sizeof(tree_t));

  t->idx = idx;
  t->w = w;
  t->h = h;
  t->margin = m;
  t->isdummy = isdummy;

  t->x = 0.0;
  t->y = 0.0;

  t->prelim = 0.0;
  t->mod = 0.0;
  t->shift = 0.0;
  t->change = 0.0;
  t->msel = 0.0;
  t->mser = 0.0;

  t->tl = NULL;
  t->tr = NULL;
  t->el = NULL;
  t->er = NULL;

  t->p = NULL;

  t->cs = cs;
  t->c = cs == 0 ? NULL : (tree_t **)malloc(sizeof(tree_t *) * cs);

  /* Those two will be computed by the algorithm. */
  t->level = -1;
  t->childno = -1;
  t->centeredxy = -1;

  return t;
}

void flat_xywh_into(tree_t *t, double *array)
{

  for (int i = 0; i < t->cs; i++)
    flat_xywh_into(t->c[i], array);

  int idx = (t->idx - 1) * 4;

  array[idx] = t->x;
  array[idx + 1] = t->y;
  array[idx + 2] = t->w;
  array[idx + 3] = t->h;
}

void flat_xy_into(int n, tree_t **nodes, double *xy)
{

  tree_t *node;

  for (int i = 0; i < n; i++)
  {
    node = nodes[i];

    xy[node->idx - 1] = node->x;
    xy[node->idx - 1 + n] = node->y;
  }
}

double maxbottom(tree_t *t, tree_t *to, int vertically, int *found)
{

  double b;
  double m = bottom(t, vertically);

  for (int i = 0; !*found && i < t->cs; i++)
  {
    tree_t *child = t->c[i];
    if (child == to)
    {
      *found = 1;
      return m;
    }
    b = maxbottom(child, to, vertically, found);
    m = b > m ? b : m;
  }

  return m;
}

void maxbottombetween(tree_t *from, tree_t *to, fringemaxbottom_t *ud)
{

  double b;
  tree_t *p = from->p;

  if (p == NULL)
    return;

  int found = 0;

  for (int i = from->childno + 1; !found && i < p->cs; i++)
  {

    tree_t *child = p->c[i];

    if (child == to)
    {
      found = 1;
      break;
    }

    b = maxbottom(child, to, ud->vertically, &found);

    ud->bottom = b > ud->bottom ? b : ud->bottom;
  }

  if (!found)
    maxbottombetween(p, to, ud);
}


tree_t **reifyflatchunks(int n, double *wh, double *whg, int *children, int rooti)
{

  int i, j;

  tree_t *node; // auxiliary variable to reference newly allocated memory locations.

  tree_t **nodes = (tree_t **)malloc(sizeof(tree_t *) * n * 2);

  for (i = 0; i < n; i++)
  {
    node = init_tree(i + 1, wh[i], wh[i + n], wh[i + 2 * n], children[i], 0);
    nodes[i] = node; // the node with the content.

    node = init_tree(i + 1 + n, whg[i], whg[i + n], 0.0, 1, 1);
    node->c[0] = nodes[i];
    nodes[i + n] = node; // the node that separates.
  }

  int nedges = n;

  for (i = 0; i < n; i++)
    for (node = nodes[i], j = 0; j < node->cs; j++, nedges++)
      node->c[j] = nodes[children[nedges] - 1 + n];

  return nodes;
}