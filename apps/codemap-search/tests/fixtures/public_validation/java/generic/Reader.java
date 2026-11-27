package generic;

public class Reader {
  // The receiver's declared type carries a type argument.
  int measure(Codec<String> codec) {
    return codec.width();
  }

  String parse(Codec<String> codec) {
    return codec.decode("a");
  }
}
