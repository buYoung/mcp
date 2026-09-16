interface Box { bucket: Box.Bucket | undefined }
declare namespace Box {
  interface Bucket { items: Array<number>; next: Bucket | undefined }
}
interface TextBox { bucket: TextSpace.Bucket }
declare namespace TextSpace { interface Bucket { items: string } }
interface MixedBox { bucket: Box.Bucket | TextSpace.Bucket }
class Registry {
  values = new Map<string, Box>();
  texts = new Map<string, TextBox>();
  mixed = new Map<string, MixedBox>();
  add(value: Box) { this.values.set("item", value); } // @STORE
  addText(value: TextBox) { this.texts.set("item", value); } // @TEXT_STORE
  addMixed(value: MixedBox) { this.mixed.set("item", value); } // @MIXED_STORE
  read() { return this.values.get("item")!.bucket!.items; } // @READ
  next() { return this.values.get("item")!.bucket!.next!.items; } // @NEXT_READ
  other() { return this.values.get("other")!.bucket!.items; } // @OTHER_READ
  text() { return this.texts.get("item")!.bucket.items; } // @TEXT_READ
  ambiguous() { return this.mixed.get("item")!.bucket.items; } // @MIXED_READ
}
