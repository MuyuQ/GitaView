import { describe, expect, it } from "vitest";
import { resolveRefreshCompletion } from "./refreshGeneration";

describe("resolveRefreshCompletion", () => {
  it("accepts only the latest request generation", () => {
    expect(resolveRefreshCompletion(3, 3)).toBe(true);
    expect(resolveRefreshCompletion(2, 3)).toBe(false);
  });
});
