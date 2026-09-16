import { Registry } from './registry.service';
declare function resolve(type: any): any;
class Router {
  first: Registry;
  second: Registry;
  constructor() {
    this.first = resolve(Registry);
    this.second = resolve(Registry);
  }
  install(callback: () => void) {
    this.first.values.set('entry', callback); // @S2
  }
  fire() { this.first.get('entry')(); } // @I1
  other() { this.second.get('entry')(); } // @I2
}
