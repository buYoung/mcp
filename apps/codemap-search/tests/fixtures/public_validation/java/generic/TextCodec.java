package generic;

public class TextCodec extends Codec<String> {
  @Override
  public String decode(String raw) {
    return raw.trim();
  }
}
