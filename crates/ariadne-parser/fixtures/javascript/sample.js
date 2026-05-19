import { readFile } from "node:fs/promises";

export class Counter {
  constructor(start = 0) {
    this.value = start;
  }

  increment() {
    this.value += 1;
    return this.value;
  }
}

export function makeCounter(start) {
  const counter = new Counter(start);
  return () => counter.increment();
}

export async function loadConfig(path) {
  const text = await readFile(path, "utf8");
  return JSON.parse(text);
}
