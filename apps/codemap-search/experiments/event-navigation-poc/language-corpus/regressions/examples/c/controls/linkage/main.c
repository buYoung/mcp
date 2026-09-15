typedef void (*Callback)(void);
static Callback current;
void install(Callback f) { current = f; }
void fire(void) { current(); }
