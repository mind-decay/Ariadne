import { join } from "node:path";

export interface Point {
  x: number;
  y: number;
}

export type Vec2 = Readonly<Point>;

export enum Side {
  Left,
  Right,
}

export class Origin {
  constructor(readonly p: Point) {}

  translate(dx: number, dy: number): Point {
    return { x: this.p.x + dx, y: this.p.y + dy };
  }
}

export function distance(a: Point, b: Point): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}

function _hidden(): string {
  return join("a", "b");
}

function log(_t: unknown, _k: string, _d: PropertyDescriptor): void {}

export class Service {
  @log
  ping(): number {
    return 1;
  }
}
