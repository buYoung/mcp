import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.HashSet;
import org.codehaus.groovy.ast.ClassNode;
import org.codehaus.groovy.ast.MethodNode;
import org.codehaus.groovy.control.SourceUnit;

// Parse and AST conversion only: never compile or run inspected source/transforms.
public class GroovyOracle {
    public static void main(String[] args) throws Exception {
        String source = Files.readString(Path.of(args[0]), StandardCharsets.UTF_8);
        SourceUnit unit = SourceUnit.create(args[0], source);
        unit.parse();
        unit.completePhase();
        unit.convert();
        System.out.println("Groovy SourceUnit\t3.0.25");
        var seen = new HashSet<ClassNode>();
        for (ClassNode type : unit.getAST().getClasses()) emit(type, seen);
    }
    private static void emit(ClassNode type, HashSet<ClassNode> seen) {
        if (!seen.add(type)) return;
        for (MethodNode method : type.getMethods()) {
            if (method.getDeclaringClass() == type) emitMethod(method, method.getName());
        }
        for (MethodNode constructor : type.getDeclaredConstructors()) emitMethod(constructor, type.getNameWithoutPackage());
        var inner = type.getInnerClasses();
        while (inner.hasNext()) emit(inner.next(), seen);
    }
    private static void emitMethod(MethodNode method, String name) {
        if (method.isSynthetic() || method.getLineNumber() < 1 || method.getLastLineNumber() < 1) return;
        String encoded = Base64.getEncoder().encodeToString(name.getBytes(StandardCharsets.UTF_8));
        int end = method.getLastLineNumber() - (method.getLastColumnNumber() == 1 ? 1 : 0);
        System.out.println(encoded + "\t" + method.getLineNumber() + "\t" + method.getLineNumber() + "\t" + end);
    }
}
