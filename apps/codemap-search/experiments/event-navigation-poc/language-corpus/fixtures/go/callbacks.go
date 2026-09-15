package callbacks

type Router struct { primary func(); secondary func() }
func (r *Router) setPrimary(cb func()) { r.primary = cb } // store_primary
func (r *Router) setSecondary(cb func()) { r.secondary = cb } // store_secondary
func (r *Router) firePrimary() { r.primary() } // call_primary
func (r *Router) fireSecondary() { r.secondary() } // call_secondary
type Other struct { primary func() }
func (r *Other) firePrimary() { r.primary() } // call_other
