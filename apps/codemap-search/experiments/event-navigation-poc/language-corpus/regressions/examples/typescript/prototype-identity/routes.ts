function make(field: string) {
  const proto = { run(this: any) { this[field](); } }; // @READ
  const Cell = function(this: any, callback: () => void) {
    this[field] = callback; // @STORE
  } as any;
  Cell.prototype = proto;
  return function(callback: () => void) { return new Cell(callback); };
}
function route(left: () => void, right: () => void) {
  const first = make("left");
  const second = make("right");
  const cell = first(left);
  const other = second(right);
  cell.run();
}
