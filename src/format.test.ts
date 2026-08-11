import { describe, expect, it } from "vitest";
import { formatDuration, formatSize } from "./format";

describe("formatDuration", () => {
  it("formats short recordings", () => expect(formatDuration(65_900)).toBe("01:05"));
  it("formats long recordings", () => expect(formatDuration(3_661_000)).toBe("01:01:01"));
  it("never displays negative time", () => expect(formatDuration(-1)).toBe("00:00"));
});

describe("formatSize", () => {
  it("formats megabytes", () => expect(formatSize(5 * 1024 * 1024)).toBe("5.0 MB"));
});
