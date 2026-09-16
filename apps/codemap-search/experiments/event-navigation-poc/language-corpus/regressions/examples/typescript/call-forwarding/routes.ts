function wrap(body: (...args: any[]) => void) {
  return function(this: any, ...values: any[]) { body.apply(this, arguments); };
}
function route(callback: () => void, decoy: () => void) {
  const left = new Map<string, () => void>();
  const right = new Map<string, () => void>();
  left.set("ready", callback); // @LEFT
  right.set("ready", decoy); // @RIGHT
  const action = wrap((key) => {
    const item = left.get(key);
    item(); // @READ
  });
  action("ready");
}
