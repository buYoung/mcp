package callbacks
import "reflect"
type Router struct { cb func() }
func keep(value reflect.Value) reflect.Value { return value }
func (r *Router) install(f func()) { r.cb = f }
func (r *Router) fire() {
    value := keep(reflect.ValueOf(r.cb))
    value.Call(nil)
}
