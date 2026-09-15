package callbacks
import "reflect"
type Router struct { cb func() }
func (r *Router) install(f func()) { r.cb = f } // S
func (r *Router) fire() {
    v := reflect.ValueOf(r.cb)
    v.Call(nil) // I
}
