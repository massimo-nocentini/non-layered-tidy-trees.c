
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include "non-layered-tidy-trees.h"

static void free_chain (chain_t *c) {
  if (c->nxt != NULL) free_chain (c->nxt);
  free (c);
}

static chain_t * update_chain (double min, int i, chain_t *init) {
  
  chain_t * ih = init;

  while(ih != NULL && min >= ih->low) {
    chain_t *nxt = ih->nxt;
    free (ih);  
    ih = nxt;
  }
  
  chain_t *out = (chain_t *) malloc (sizeof(chain_t));
  out->low = min;
  out->index = i;
  out->nxt = ih;

  return out;
}

// Process change and shift to add intermediate spacing to mod.
static void addChildSpacing(tree_t *t){
  double d = 0.0, modsumdelta = 0.0;
  for(int i = 0 ; i < t->cs; i++){
    d += t->c[i]->shift;
    modsumdelta += d + t->c[i]->change;
    t->c[i]->mod += modsumdelta;
  }
}

static double bottom(tree_t *t, int vertically) { 
  return vertically != 0 ? t->y + t->h : t->x + t->w; 
}

static void setExtremes(tree_t *t) {
  if(t->cs == 0){
    t->el = t; t->er = t;
    t->msel = t->mser = 0.0;
  } else {
    t->el = t->c[0]->el; t->msel = t->c[0]->msel;
    t->er = t->c[t->cs-1]->er; t->mser = t->c[t->cs-1]->mser;
  }
}

static void distributeExtra(tree_t *t, int i, int si, double dist) {
  if(si != i-1){ // Are there intermediate children?
    double nr = i - si;
    double ratio = dist / nr;
    t->c[si + 1]->shift += ratio;
    t->c[i]->shift -= ratio;
    t->c[i]->change -= dist - ratio;
  }
}

static void moveSubtree (tree_t *t, int i, int si, double dist) {
  t->c[i]->mod  += dist;
  t->c[i]->msel += dist;
  t->c[i]->mser += dist;
  distributeExtra(t, i, si, dist);
}

static tree_t * nextLeftContour (tree_t *t) {return t->cs==0 ? t->tl : t->c[0];}
static tree_t * nextRightContour(tree_t *t) {return t->cs==0 ? t->tr : t->c[t->cs-1];}

static void setLeftThread(tree_t *t, int i, tree_t *cl, double modsumcl) {
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
static void setRightThread(tree_t *t, int i, tree_t *sr, double modsumsr) {
  tree_t *ri = t->c[i]->er;
  ri->tr = sr;
  double diff = (modsumsr - sr->mod) - t->c[i]->mser;
  ri->mod += diff;
  ri->prelim -= diff;
  t->c[i]->er = t->c[i-1]->er; 
  t->c[i]->mser = t->c[i-1]->mser;
}

static void separate(tree_t *t, int vertically, int i, chain_t *init, void *userdata, contourpairs_t pairscb) {
  
  tree_t *sr = t->c[i-1];
  double mssr = sr->mod;

  tree_t *cl = t->c[i];
  double mscl = cl->mod;

  chain_t *ih = init;

  for (int iterations = 0; sr != NULL && cl != NULL; iterations++){
    
    if(bottom(sr, vertically) > ih->low) ih = ih->nxt;
    
    // How far to the left of the right side of sr is the left side of cl?
    double srd = vertically != 0 ? sr->w : sr->h;
    double dist = (mssr + sr->prelim + srd) - (mscl + cl->prelim);
    
    if(dist > 0.0 || (iterations == 0 && dist < 0.0)){
      mscl += dist;
      //assert (ih != NULL);
      moveSubtree (t,i,ih->index,dist);
    }

    if (pairscb != NULL) pairscb (sr, cl, dist, userdata);

    double sy = bottom(sr, vertically);
    double cy = bottom(cl, vertically);
    
    if(sy <= cy){
      sr = nextRightContour(sr);
      if(sr != NULL) mssr += sr->mod;
    }

    if(sy >= cy){
      cl = nextLeftContour(cl);
      if(cl != NULL) mscl += cl->mod;
    }
  }
  
  // Set threads and update extreme nodes.
  // In the first case, the current subtree must be taller than the left siblings.
  if(sr == NULL && cl != NULL) setLeftThread(t,i,cl,mscl);
  // In this case, the left siblings must be taller than the current tree.
  else if(sr != NULL && cl == NULL) setRightThread(t,i,sr,mssr);
}

static void positionRoot(tree_t *t, int vertically) {
  // Position root between children, taking into account their mod.
  int last = t->cs-1;
  double d = vertically != 0 ? t->c[last]->w - t->w : t->c[last]->h - t->h;
  t->prelim = (t->c[0]->prelim + t->c[0]->mod + t->c[last]->prelim + t->c[last]->mod + d) / 2.0;
}

static void firstWalk(tree_t *t, int vertically, int centeredxy, void *userdata, callback_t cb, contourpairs_t pairscb) {

  if(t->cs == 0){ setExtremes(t); }
  else {
    
    firstWalk(t->c[0], vertically, centeredxy, userdata, cb, pairscb);

    chain_t *ih = update_chain (bottom(t->c[0]->el, vertically), 0, NULL);
      
    for(int i = 1; i < t->cs; i++){
      
      firstWalk(t->c[i], vertically, centeredxy, userdata, cb, pairscb);
      
      double min = bottom(t->c[i]->er, vertically);
      
      separate(t, vertically, i, ih, userdata, pairscb);
      
      ih = update_chain (min,i,ih);
    }

    free_chain (ih);

    positionRoot(t, vertically);    
    setExtremes(t);
    
    if (cb != NULL) {

      double x = 0.0, y = 0.0;
      double d = t->prelim + t->mod;

      if (vertically != 0) {
        x = d;
        y = t->y;
      } else {
        x = t->x;
        y = d;
      }

      if (centeredxy != 0) {
        x += t->w / 2.0;
        y += t->h / 2.0;
      }

      cb (t, x, y, t->w, t->h, userdata);
    }
  }

}

static void secondWalk(tree_t *t, int vertically, int centeredxy, double modsum_init, void *userdata, callback_t cb) {
  double modsum = modsum_init + t->mod;

  double d = t->prelim + modsum;

  double xoffset = 0.0, yoffset = 0.0;

  if (centeredxy != 0) {
    xoffset = t->w / 2.0;
    yoffset = t->h / 2.0;
  }

  if (vertically != 0) {
    t->x = d + xoffset;
    t->y += yoffset;
  } else {
    t->x += xoffset;
    t->y = d + yoffset;
  }

  addChildSpacing(t);
  
  if (cb != NULL) cb (t, t->x, t->y, t->w, t->h, userdata);
  
  for(int i = 0 ; i < t->cs ; i++) secondWalk(t->c[i], vertically, centeredxy, modsum, userdata, cb);
}

static void setupWalk (tree_t *t, int level, int vertically) {
  t->level = level;
  double b = bottom (t, vertically);
  for(int i = 0; i < t->cs; i++) {
    tree_t *child = t->c[i];

    child->childno = i;
    child->p = t;
    
    if (vertically != 0) child->y += b; else child->x += b;
    
    setupWalk (child, level + 1, vertically);
  }
}

EXPORT void CallingConvention layout(tree_t *t, int vertically, int centeredxy, void *userdata, callback_t firstcb, callback_t secondcb, contourpairs_t pairscb){
  setupWalk (t, 0, vertically);
  firstWalk(t, vertically, centeredxy, userdata, firstcb, pairscb);
  secondWalk(t, vertically, centeredxy, 0.0, userdata, secondcb);
}
