
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
typedef struct tree_s {
  int idx;
  int level;
  int childno;
  double w, h;                  // Width and height.
  double x, y, prelim, mod, shift, change;
  struct tree_s *tl;
  struct tree_s *tr;            // Left and right thread.
  struct tree_s *el;
  struct tree_s *er;            // Extreme left and right nodes.
  double msel, mser;            // Sum of modifiers at the extreme nodes.
  struct tree_s **c; int cs;    // Array of children and number of children.
  struct tree_s *p;             // my parent.
} tree_t;

/*
 * A linked list of the indexes of left siblings and their lowest vertical coordinate.
 */
typedef struct chain_s { 
  double low; 
  int index; 
  struct chain_s *nxt;
} chain_t;

typedef void (*callback_t) (tree_t *, double, double, double, double, void *);

typedef void (*contourpairs_t) (tree_t *, tree_t *, double, void *);

EXPORT void CallingConvention layout(tree_t *, int, int, void *, callback_t, callback_t, contourpairs_t);
