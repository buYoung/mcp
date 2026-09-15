package callbacks
import rfx "reflect"
type Router struct { cb func(); other func() }
func (r *Router) install(f, g func()) {
    r.cb = f // @S1
    r.other = g // @S2
}
func (r *Router) fire() {
    value := rfx.ValueOf(r.cb)
    value.Call(nil) // @I1
}
type Value struct {}
func (v Value) Call(args []any) {}
func (r *Router) custom() {
    rfx := struct { ValueOf func(func()) Value }{func(f func()) Value { return Value{} }}
    value := rfx.ValueOf(r.cb)
    value.Call(nil) // @I2
}
