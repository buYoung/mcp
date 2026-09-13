export class KnownBus {
  on(key: string, handler: () => void) {}
  once(key: string, handler: () => void) {}
  off(key: string, handler: () => void) {}
  emit(key: string) {}
}
