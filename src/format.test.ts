import { describe, expect, it } from "vitest";
import { formatDuration, formatSize, transcriptionPercentage } from "./format";

describe("formatDuration", () => {
  it("formats short recordings", () => expect(formatDuration(65_900)).toBe("01:05"));
  it("formats long recordings", () => expect(formatDuration(3_661_000)).toBe("01:01:01"));
  it("never displays negative time", () => expect(formatDuration(-1)).toBe("00:00"));
});

describe("formatSize", () => {
  it("formats megabytes", () => expect(formatSize(5 * 1024 * 1024)).toBe("5.0 MB"));
  it("formats model-sized downloads in gigabytes", () => expect(formatSize(4_683_074_240)).toBe("4.4 GB"));
  it("switches units at the gigabyte boundary", () => {
    expect(formatSize(1024 ** 3 - 1)).toBe("1024.0 MB");
    expect(formatSize(1024 ** 3)).toBe("1.0 GB");
  });
});

describe("transcriptionPercentage", () => {
  it("converts a ratio to a whole percentage", () => expect(transcriptionPercentage(0.425)).toBe(43));
  it("clamps values outside 0..1", () => {
    expect(transcriptionPercentage(-0.5)).toBe(0);
    expect(transcriptionPercentage(1.5)).toBe(100);
  });
  it("treats a non-finite progress as no progress", () => {
    expect(transcriptionPercentage(Number.NaN)).toBe(0);
    expect(transcriptionPercentage(Number.POSITIVE_INFINITY)).toBe(100);
  });
});
