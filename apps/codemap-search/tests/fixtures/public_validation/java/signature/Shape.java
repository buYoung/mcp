package shapes;

public interface Shape {
  String describe(String prefix);

  double scaled(Unit unit);

  void render(Canvas canvas);
}
