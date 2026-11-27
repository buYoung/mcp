package shapes.impl;

import shapes.Shape;
import shapes.Unit;

public abstract class Circle implements Shape {
  @Override
  public String describe(String prefix) {
    return prefix + "circle";
  }

  @Override
  public double scaled(Unit unit) {
    return 1.0;
  }

  public void render(Canvas canvas) {}
}
