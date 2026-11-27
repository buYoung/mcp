export abstract class Codec<T> {
  abstract decode(raw: string): T;

  width(): number {
    return 1;
  }
}

export class TextCodec extends Codec<string> {
  decode(raw: string): string {
    return raw.trim();
  }
}

// The receiver's declared type carries a type argument.
export function measure(codec: Codec<string>): number {
  return codec.width();
}

export function parse(codec: Codec<string>): string {
  return codec.decode('a');
}
