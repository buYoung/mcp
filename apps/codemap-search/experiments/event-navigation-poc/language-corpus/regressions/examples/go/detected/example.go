package callbacks
type Router struct { cb func() }
func (r *Router) install(f func()) { r.cb = f } // S
func (r *Router) fire() { r.cb() } // I
