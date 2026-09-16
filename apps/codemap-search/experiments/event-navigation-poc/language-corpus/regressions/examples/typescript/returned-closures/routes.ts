function makeReader(entries: Map<string, () => void>) {
  return () => {
    const callback = entries.get("ready");
    callback(); // READ
  };
}
function applyBody(action: (key: string) => void, key: string) { action(key); }
function ignoreBody(action: () => void) { return 0; }
function route(first: () => void, second: () => void) {
  const left = new Map<string, () => void>();
  const right = new Map<string, () => void>();
  left.set("ready", first); // LEFT
  right.set("ready", second); // RIGHT
  const read = makeReader(left);
  read();
  applyBody((key) => {
    const callback = left.get(key);
    callback(); // APPLIED
  }, "ready");
  ignoreBody(() => {
    const callback = right.get("ready");
    callback(); // IGNORED
  });
}
