

typedef struct tree_s tree_t;

// ^{\normalfont A linked list of the indexes of left siblings and their lowest vertical coordinate.}^  
typedef struct chain_s  chain_t;

typedef void (*callback_t) (tree_t *, void *);

struct tree_s {
  long id;
  double w, h;          // ^{\normalfont Width and height.}^
  double x, y, prelim, mod, shift, change;
  tree_t *tl;
  tree_t *tr;          // ^{\normalfont Left and right thread.}^                        
  tree_t *el;
  tree_t *er;          // ^{\normalfont Extreme left and right nodes.}^ 
  double msel, mser;    // ^{\normalfont Sum of modifiers at the extreme nodes.}^ 
  tree_t **c; int cs;     // ^{\normalfont Array of children and number of children.}^
  tree_t *p;  // my parent.
};

struct chain_s { double low; int index; chain_t *nxt;};

void layout(tree_t *, int, void *, callback_t, callback_t);