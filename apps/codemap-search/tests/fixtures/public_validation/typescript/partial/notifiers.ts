import type { Notifier } from './notifier';

export class EmailNotifier implements Notifier {
  send(count: number): boolean {
    return count > 0;
  }
}

// A generic type implements the declaration like any other type.
export class BufferedNotifier<T> implements Notifier {
  private pending: T[] = [];

  send(count: number): boolean {
    return this.pending.length >= count;
  }
}
