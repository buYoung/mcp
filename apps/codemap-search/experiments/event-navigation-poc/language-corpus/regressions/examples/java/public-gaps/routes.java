import java.lang.reflect.Method;
class Descriptor {
    final Method method;
    final String text;
    Descriptor(Method method, String text) { this.method = method; this.text = text; }
}
class Router {
    private Descriptor descriptor;
    private String label;
    void install(Descriptor descriptor, String label) {
        this.descriptor = descriptor; // @S1
        this.label = label; // @S2
    }
    void fire(Object target) throws Exception {
        descriptor.method.invoke(target); // @I1
    }
}
