
#include <stdlib.h>
#include "non-layered-tidy-trees.h"

static IYL_t * updateIYL(double minY, int i, IYL_t *init) {
  // ^{\normalfont Remove siblings that are hidden by the new subtree_t *

  IYL_t * ih = init;
  while(ih != NULL && minY >= ih->lowY) ih = ih->nxt;
  // ^{\normalfont Prepend the new subtree_t *

  IYL_t *out = (IYL_t *) malloc (sizeof (IYL_t));
  out->lowY = minY;
  out->index = i;
  out->nxt = ih;

  return out;
}

// ^{\normalfont Process change and shift to add intermediate spacing to mod.}^
static void addChildSpacing(tree_t *t){
  double d = 0.0, modsumdelta = 0.0;
  for(int i = 0 ; i < t->cs; i++){
    d+=t->c[i]->shift;
    modsumdelta+=d + t->c[i]->change;
    t->c[i]->mod+=modsumdelta;
  }
}

static double bottom(tree_t *t) { return t->y + t->h;  }

static  void setExtremes(tree_t *t) {
  if(t->cs == 0){
    t->el = t; t->er = t;
    t->msel = t->mser =0.0;
  } else {
    t->el = t->c[0]->el; t->msel = t->c[0]->msel;
    t->er = t->c[t->cs-1]->er; t->mser = t->c[t->cs-1]->mser;
  }
}

static void distributeExtra(tree_t *t, int i, int si, double dist) {
  // ^{\normalfont Are there intermediate children?}^
  if(si != i-1){
    double nr = i - si;
    t->c[si +1]->shift+=dist/nr;
    t->c[i]->shift-=dist/nr;
    t->c[i]->change-=dist - dist/nr;
  }
}

static void moveSubtree (tree_t *t, int i, int si, double dist) {
  // ^{\normalfont Move subtree by changing mod.}^
  t->c[i]->mod+=dist; t->c[i]->msel+=dist; t->c[i]->mser+=dist;
  distributeExtra(t, i, si, dist);
}

static tree_t * nextLeftContour (tree_t *t) {return t->cs==0 ? t->tl : t->c[0];}
static tree_t * nextRightContour(tree_t *t) {return t->cs==0 ? t->tr : t->c[t->cs-1];}

static void setLeftThread(tree_t *   t, int i, tree_t *     cl, double modsumcl) {
  tree_t *         li = t->c[0]->el;
  li->tl = cl;
  // ^{\normalfont Change mod so that the sum of modifier after following thread is correct.}^
  double diff = (modsumcl - cl->mod) - t->c[0]->msel ;
  li->mod += diff;
  // ^{\normalfont Change preliminary x coordinate so that the node does not move.}^
  li->prelim-=diff;
  // ^{\normalfont Update extreme node and its sum of modifiers.}^
  t->c[0]->el = t->c[i]->el; t->c[0]->msel = t->c[i]->msel;
}

// ^{\normalfont Symmetrical to setLeftThread.}^
static void setRightThread(tree_t *     t, int i, tree_t *     sr, double modsumsr) {
  tree_t *         ri = t->c[i]->er;
  ri->tr = sr;
  double diff = (modsumsr - sr->mod) - t->c[i]->mser ;
  ri->mod += diff;
  ri->prelim-=diff;
  t->c[i]->er = t->c[i-1]->er; t->c[i]->mser = t->c[i-1]->mser;
}

static void seperate(tree_t *t,int i,  IYL_t *ih ){
  // ^{\normalfont Right contour node of left siblings and its sum of modfiers.}^
  tree_t *sr = t->c[i-1];
  double mssr = sr->mod;

  // ^{\normalfont Left contour node of current subtree_t * and its sum of modfiers.}^
  tree_t *cl = t->c[i];
  double mscl = cl->mod;

  while(sr != NULL && cl != NULL){
    if(bottom(sr) > ih->lowY) ih = ih->nxt;

    // ^{\normalfont How far to the left of the right side of sr is the left side of cl?}^
    double dist = (mssr + sr->prelim + sr->w) - (mscl + cl->prelim);
    if(dist > 0){
      mscl+=dist;
      moveSubtree (t,i,ih->index,dist);
    }
    double sy = bottom(sr), cy = bottom(cl);
    // ^{\normalfont Advance highest node(s) and sum(s) of modifiers}^
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

static void positionRoot(tree_t *     t) {
  // ^{\normalfont Position root between children, taking into account their mod.}^
  t->prelim = (t->c[0]->prelim + t->c[0]->mod + t->c[t->cs-1]->mod +
           t->c[t->cs-1]->prelim +  t->c[t->cs-1]->w)/2 - t->w/2;
}

static void firstWalk(tree_t *t, void *userdata, callback_t cb) {

  if(t->cs == 0){ setExtremes(t); return; }

  firstWalk(t->c[0], userdata, cb);

  // ^{\normalfont Create siblings in contour minimal vertical coordinate and index list.}^
  IYL_t *ih =  updateIYL(bottom(t->c[0]->el),0,NULL);

  for(int i = 1; i < t->cs; i++){
    firstWalk(t->c[i], userdata, cb);
    //^{\normalfont Store lowest vertical coordinate while extreme nodes still point in current subtree_t *

    double minY = bottom(t->c[i]->er);
    seperate(t,i,ih);
    ih = updateIYL(minY,i,ih);
  }
  positionRoot(t);
  setExtremes(t);
  if (cb != NULL) cb (t, userdata);
}

static void secondWalk(tree_t *    t, double modsum, void *userdata, callback_t cb) {
  modsum+=t->mod;
  // ^{\normalfont Set absolute (non-relative) horizontal coordinate.}^
  t->x = t->prelim + modsum;
  if (cb != NULL) cb (t, userdata);
  addChildSpacing(t);
  for(int i = 0 ; i < t->cs ; i++) secondWalk(t->c[i],modsum, userdata, cb);
}

void layout(tree_t *t, void *userdata, callback_t firstcb, callback_t secondcb){ 
  firstWalk(t, userdata, firstcb);
  secondWalk(t,0.0, userdata, secondcb);
}
