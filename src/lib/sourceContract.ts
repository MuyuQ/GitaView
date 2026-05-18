import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const projectRoot = resolve(import.meta.dirname, "../..");

export function readProjectFile(path: string) {
  return readFileSync(resolve(projectRoot, path), "utf8");
}

export function readProjectFileCompact(path: string) {
  return readProjectFile(path).replace(/\s+/g, " ");
}

export function readProjectJson<T>(path: string): T {
  return JSON.parse(readProjectFile(path)) as T;
}
