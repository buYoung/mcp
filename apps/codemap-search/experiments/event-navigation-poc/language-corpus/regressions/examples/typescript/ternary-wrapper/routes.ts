function wrap(body: (...args: any[]) => void, ...transforms: any[]) {
  const fn = transforms.length === 0
    ? function(this: any, ...values: any[]) { return body.apply(this, arguments); }
    : function(this: any, ...values: any[]) { return 0; };
  return Object.defineProperty(fn, "length", { value: body.length });
}
function route(callback: () => void, decoy: () => void) {
  const left = new Map<string, () => void>();
  const right = new Map<string, () => void>();
  left.set("ready", callback); // @LEFT
  right.set("ready", decoy); // @RIGHT
  const action = wrap((key) => { left.get(key)(); }); // @READ
  action("ready");
}
