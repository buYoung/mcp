typedef void (*Callback)(void);
static Callback current;
void other_fire(void) { current(); }
