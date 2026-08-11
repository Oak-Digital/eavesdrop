import { describe, expect, it } from "vitest";
import { downloadPercentage } from "./useUpdater";

describe("downloadPercentage", () => {
  it("reports bounded update progress", () => {
    expect(downloadPercentage(25, 100)).toBe(25);
    expect(downloadPercentage(150, 100)).toBe(100);
    expect(downloadPercentage(25)).toBeNull();
  });
});
