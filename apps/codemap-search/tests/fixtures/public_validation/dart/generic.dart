abstract class Codec<T> {
  T decode(String raw);

  int width() => 1;
}

class TextCodec extends Codec<String> {
  @override
  String decode(String raw) => raw.trim();
}

// The receiver's declared type carries a type argument.
int measure(Codec<String> codec) {
  return codec.width();
}

String parse(Codec<String> codec) {
  return codec.decode('a');
}
