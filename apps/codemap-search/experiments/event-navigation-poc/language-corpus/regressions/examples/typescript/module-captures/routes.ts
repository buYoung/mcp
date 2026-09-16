import { primary, secondary, registry, makeStore } from './barrel';
function install(callback: () => void) {
  primary.set('entry', callback); // @S1
}
function fire() { registry.left.get('entry')!(); } // @I1
function other() { secondary.get('entry')!(); } // @I2
function instances(callback: () => void) {
  const first = makeStore();
  const second = makeStore();
  first.put('entry', callback);
  first.get('entry')!(); // @I3
  second.get('entry')!(); // @I4
}
