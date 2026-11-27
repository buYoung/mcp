package generic;

public abstract class Codec<T> {
  public abstract T decode(String raw);

  public int width() {
    return 1;
  }
}
