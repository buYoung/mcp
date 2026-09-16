package sample;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
class Base {
  private Object value;
  private Object other;
  private static final VarHandle HANDLE;
  static {
    try { HANDLE = MethodHandles.lookup().findVarHandle(Base.class, "value", Object.class); }
    catch (ReflectiveOperationException ex) { throw new ExceptionInInitializerError(ex); }
  }
  Base(Object initial) { this.value = initial; }
  Object read() { return HANDLE.getVolatile(this); }
  Object other() { return this.other; }
  void write(Object update) { HANDLE.compareAndSet(this, null, update); }
}
class Cell extends Base { Cell(Object initial) { super(initial); } }
class GuardCell {
  private final Object fixed = null;
  private Object mutable;
  private static final VarHandle FINAL, WRONG;
  static {
    try {
      FINAL = MethodHandles.lookup().findVarHandle(GuardCell.class, "fixed", Object.class);
      WRONG = MethodHandles.lookup().findVarHandle(GuardCell.class, "mutable", String.class);
    } catch (ReflectiveOperationException ex) { throw new ExceptionInInitializerError(ex); }
  }
  Object readFinal() { return FINAL.get(this); }
  void writeFinal(Object update) { FINAL.set(this, update); } // @FINAL_STORE
  Object readWrong() { return WRONG.get(this); }
  void writeWrong(Object update) { WRONG.set(this, update); } // @WRONG_STORE
}
