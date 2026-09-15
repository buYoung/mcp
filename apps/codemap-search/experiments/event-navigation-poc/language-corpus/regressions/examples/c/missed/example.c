typedef void (*Callback)(void);
static Callback current;
void install(Callback cb) { current = cb; } /* S */
void fire(void) { current(); } /* I */
