

/*
 * A type for hierarchies that act as trees.
 */
typedef struct tree_s
{
  int idx;
  int level;
  int childno;
  int centeredxy; // relates `x` and `y` to `w` and `h`, respectively.
  int isdummy;
  double w, h, margin; // Width and height.
  double x, y, prelim, mod, shift, change;
  struct tree_s *tl;
  struct tree_s *tr; // Left and right thread.
  struct tree_s *el;
  struct tree_s *er; // Extreme left and right nodes.
  double msel, mser; // Sum of modifiers at the extreme nodes.
  struct tree_s **c;
  int cs;           // Array of children and number of children.
  struct tree_s *p; // my parent.
} tree_t;

typedef void (*callback_t)(tree_t *, void *);

typedef void (*contourpairs_t)(tree_t *, tree_t *, double, void *);

typedef struct treeinput_s
{
  tree_t *t;

  int vertically;
  int centeredxy;

  void *walkud;
  callback_t walkcb;

  void *cpairsud;
  contourpairs_t cpairscb;

  double x, y;

} treeinput_t;

void layout(treeinput_t *);
double bottom(tree_t *, int);
void free_tree(tree_t *);
tree_t *init_tree(int, double, double, double, int, int);
tree_t **reifyflatchunks(int, double *, double *, int *, int);
void flat_xywh_into(tree_t *, double *);
void flat_xy_into(int, tree_t **, double *);

typedef struct fringemaxbottom_s
{
  double bottom;
  int vertically;
} fringemaxbottom_t;

double maxbottom(tree_t *, tree_t *, int, int *);
void maxbottombetween(tree_t *, tree_t *, fringemaxbottom_t *);