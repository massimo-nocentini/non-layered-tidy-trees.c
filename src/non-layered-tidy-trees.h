

typedef struct tree_s tree_t;

// ^{\normalfont A linked list of the indexes of left siblings and their lowest vertical coordinate.}^  
typedef struct IYL_s  IYL_t;

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
};

struct IYL_s { double lowY; int index; IYL_t *nxt;};

void update_width_height (tree_t *, double, double);
void layout(tree_t *, void *, callback_t, callback_t);