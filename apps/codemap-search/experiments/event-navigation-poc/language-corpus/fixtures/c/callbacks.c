typedef void (*Callback)(void);
typedef struct Router { Callback primary; Callback secondary; } Router;
void set_primary(Router *self, Callback cb) { self->primary = cb; } // store_primary
void set_secondary(Router *self, Callback cb) { self->secondary = cb; } // store_secondary
void fire_primary(Router *self) { self->primary(); } // call_primary
void fire_secondary(Router *self) { self->secondary(); } // call_secondary
typedef struct Other { Callback primary; } Other;
void fire_other(Other *self) { self->primary(); } // call_other
