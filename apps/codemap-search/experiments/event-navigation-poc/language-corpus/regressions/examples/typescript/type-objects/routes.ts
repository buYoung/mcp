type Factory = {
  (callback: () => void): () => void
  <T>(callback: () => void, tag: T): () => void
}
const identity: Factory = (callback: () => void) => callback;
const callbacks = new Map<string, () => void>();
const description = `interface Fake { callback: Unknown }`;
const pattern = /interface Fake {/;
function install(callback: () => void) {
  callbacks.set('ready', identity(callback)); // @S1
}
function fire() { callbacks.get('ready')!(); } // @I1
function other() { callbacks.get('absent')!(); } // @I2
function literal(callback: () => void) {
  const value = { callback }; // @S2
  value.callback(); // @I3
}
