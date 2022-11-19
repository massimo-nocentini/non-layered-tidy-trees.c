
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include "non-layered-tidy-trees.h"

static chain_t * updateIYL(double min, int i, chain_t *init) {
  
  chain_t * ih = init;
  while(ih != NULL && min >= ih->low) ih = ih->nxt;
  
  chain_t *out = (chain_t *) malloc (sizeof(chain_t));
  out->low = min;
  out->index = i;
  out->nxt = ih;

  return out;
}

// ^{\normalfont Process change and shift to add intermediate spacing to mod.}^
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
  // ^{\normalfont Are there intermediate children?}^
  if(si != i-1){
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

static void setLeftThread(tree_t *   t, int i, tree_t *     cl, double modsumcl) {
  tree_t *li = t->c[0]->el;
  li->tl = cl;
  // ^{\normalfont Change mod so that the sum of modifier after following thread is correct.}^
  double diff = (modsumcl - cl->mod) - t->c[0]->msel ;
  li->mod += diff;
  // ^{\normalfont Change preliminary x coordinate so that the node does not move.}^
  li->prelim -= diff;
  // ^{\normalfont Update extreme node and its sum of modifiers.}^
  t->c[0]->el = t->c[i]->el; t->c[0]->msel = t->c[i]->msel;
}

// ^{\normalfont Symmetrical to setLeftThread.}^
static void setRightThread(tree_t *t, int i, tree_t *sr, double modsumsr) {
  tree_t *ri = t->c[i]->er;
  ri->tr = sr;
  double diff = (modsumsr - sr->mod) - t->c[i]->mser;
  ri->mod += diff;
  ri->prelim -= diff;
  t->c[i]->er = t->c[i-1]->er; 
  t->c[i]->mser = t->c[i-1]->mser;
}

static void seperate(tree_t *t, int vertically, int i, chain_t *init ){
  
  tree_t *sr = t->c[i-1];
  double mssr = sr->mod;

  tree_t *cl = t->c[i];
  double mscl = cl->mod;

  chain_t *ih = init;
  while(sr != NULL && cl != NULL){
    
    if(bottom(sr, vertically) > ih->low) ih = ih->nxt;
    
    // ^{\normalfont How far to the left of the right side of sr is the left side of cl?}^
    double srd = vertically != 0 ? sr->w : sr->h;
    double dist = (mssr + sr->prelim + srd) - (mscl + cl->prelim);
    
    if(dist > 0.0){
      mscl+=dist;
      assert (ih != NULL);
      moveSubtree (t,i,ih->index,dist);
    }

    double sy = bottom(sr, vertically), cy = bottom(cl, vertically);
    
    if(sy <= cy){
      sr = nextRightContour(sr);
      if(sr!=NULL) mssr+=sr->mod;
    }
    if(sy >= cy){
      cl = nextLeftContour(cl);
      if(cl!=NULL) mscl+=cl->mod;
    }
  }
  // ^{\normalfont Set threads and update extreme nodes.}^
  // ^{\normalfont In the first case, the current subtree must be taller than the left siblings.}^
  if(sr == NULL && cl != NULL) setLeftThread(t,i,cl, mscl);
  // ^{\normalfont In this case, the left siblings must be taller than the current subtree_t *.}^
  else if(sr != NULL && cl == NULL) setRightThread(t,i,sr,mssr);
}

static void positionRoot(tree_t *t, int vertically) {
  // ^{\normalfont Position root between children, taking into account their mod.}^
  int last = t->cs-1;
  double d = vertically != 0 ? t->c[last]->w - t->w : t->c[last]->h - t->h;
  t->prelim = (t->c[0]->prelim + t->c[0]->mod + t->c[last]->mod + t->c[last]->prelim + d) / 2.0;
}

static void firstWalk(tree_t *t, int vertically, void *userdata, callback_t cb) {

  if(t->cs == 0){ setExtremes(t); }
  else {
    firstWalk(t->c[0], vertically, userdata, cb);

    chain_t *ih = updateIYL(bottom(t->c[0]->el, vertically), 0, NULL);
      
    for(int i = 1; i < t->cs; i++){
      
      firstWalk(t->c[i], vertically, userdata, cb);
      
      double min = bottom(t->c[i]->er, vertically);
      
      seperate(t, vertically, i, ih);
      
      ih = updateIYL(min,i,ih);
    }

    positionRoot(t, vertically);    
    setExtremes(t);
    
    if (cb != NULL) cb (t, userdata);
  }

}

static void secondWalk(tree_t *t, int vertically, double modsum_init, void *userdata, callback_t cb) {
  double modsum = modsum_init + t->mod;

  double d = t->prelim + modsum;
  if (vertically != 0) {
    t->x = d + (t->w / 2.0);
    t->y += t->h / 2.0;
  }
  else {
    t->y = d + (t->h / 2.0);
    t->x += t->w / 2.0;
  }

  addChildSpacing(t);
  
  if (cb != NULL) cb (t, userdata);
  
  for(int i = 0 ; i < t->cs ; i++) secondWalk(t->c[i], vertically, modsum, userdata, cb);
}

static void setupWalk (tree_t *t, int vertically) {
  double b = bottom (t, vertically);
  for(int i = 0; i < t->cs; i++) {
    tree_t *child = t->c[i];
    if (vertically != 0) child->y += b; else child->x += b;
    setupWalk (child, vertically);
  }
}

void layout(tree_t *t, int vertically, void *userdata, callback_t firstcb, callback_t secondcb){
  setupWalk (t, vertically);
  firstWalk(t, vertically, userdata, firstcb);
  secondWalk(t, vertically, 0.0, userdata, secondcb);
}
