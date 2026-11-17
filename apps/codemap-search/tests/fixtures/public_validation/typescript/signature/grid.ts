import type { Layout } from './layout';
import type { Panel } from './panel';

export class Grid implements Layout {
  add(panel: Panel): void {}

  resize(width: number, sizes: Record<string, number>): void {}
}
