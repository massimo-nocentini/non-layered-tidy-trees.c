
#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT __attribute__((visibility("default")))
#endif

#ifdef _WIN32
#if defined __LP64__ || defined _WIN64
#define CallingConvention __attribute__((ms_abi))
#else
#define CallingConvention __stdcall
#endif
#else
#define CallingConvention
#endif

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

EXPORT void CallingConvention layout(treeinput_t *);

EXPORT double CallingConvention bottom(tree_t *, int);

EXPORT void CallingConvention free_tree(tree_t *);
