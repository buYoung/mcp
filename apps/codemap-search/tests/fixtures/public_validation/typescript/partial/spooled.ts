import type { Notifier } from '#partial/notifier';

// The base is named through a module alias that no indexed file resolves.
export class SpooledNotifier implements Notifier {
  send(count: number): boolean {
    return count > 1;
  }
}
