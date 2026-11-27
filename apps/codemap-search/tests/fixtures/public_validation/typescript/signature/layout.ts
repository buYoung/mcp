import type { Panel } from './panel';

export interface Layout {
  add(panel: Panel): void;
  resize(width: number, sizes: Record<string, number>): void;
}
