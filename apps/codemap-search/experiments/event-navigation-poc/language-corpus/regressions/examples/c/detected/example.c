typedef void (*Callback)(void);
typedef struct Router { Callback cb; } Router;
void install(Router *r, Callback cb) { r->cb = cb; } /* S */
void fire(Router *r) { r->cb(); } /* I */
