import type { Notifier } from './notifier';

export class EmailNotifier implements Notifier {
  send(count: number): boolean {
    return count > 0;
  }
}

// A generic type is not linked to the declarations it implements.
export class BufferedNotifier<T> implements Notifier {
  private pending: T[] = [];

  send(count: number): boolean {
    return this.pending.length >= count;
  }
}
