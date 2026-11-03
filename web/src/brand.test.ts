import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BRAND } from "./brand";

const srcDir = __dirname;
const ALLOWED = new Set(["brand.json", "brand.ts", "brand.test.ts"]);
const walk = (dir: string): string[] =>
  readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });

describe("brand", () => {
  it("has a non-empty name and title", () => {
    expect(BRAND.name.length).toBeGreaterThan(0);
    expect(BRAND.adminTitle.length).toBeGreaterThan(0);
  });

  it("does not hardcode the product name outside the brand files (tests may use fixtures)", () => {
    const name = new RegExp(`(?<![A-Za-z@/.-])${BRAND.name}(?![A-Za-z])`);
    const offenders = walk(srcDir)
      .filter((f) => /\.(ts|tsx|html|css)$/.test(f) && !/\.test\.tsx?$/.test(f))
      .filter((f) => !ALLOWED.has(f.slice(srcDir.length + 1)))
      .filter((f) => name.test(readFileSync(f, "utf8").replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, "")))
      .map((f) => f.slice(srcDir.length + 1));
    expect(offenders).toEqual([]);
  });
});
