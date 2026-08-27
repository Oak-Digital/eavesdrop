import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");

function token(name: string) {
  const value = styles.match(new RegExp(`--${name}:\\s*(#[0-9a-f]{6})`, "i"))?.[1];
  if (!value) throw new Error(`Missing CSS token --${name}`);
  return value;
}

function luminance(hex: string) {
  const channels = hex.slice(1).match(/.{2}/g)!.map((value) => parseInt(value, 16) / 255);
  const [red, green, blue] = channels.map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  return red * 0.2126 + green * 0.7152 + blue * 0.0722;
}

function contrast(foreground: string, background: string) {
  const lighter = Math.max(luminance(foreground), luminance(background));
  const darker = Math.min(luminance(foreground), luminance(background));
  return (lighter + 0.05) / (darker + 0.05);
}

describe("text color tokens", () => {
  it("keeps tertiary text readable on every dark surface", () => {
    const tertiary = token("text-tertiary");
    const backgrounds = ["bg-canvas", "bg-base", "bg-surface", "bg-elevated", "bg-muted"];

    backgrounds.forEach((background) => {
      expect(contrast(tertiary, token(background)), background).toBeGreaterThanOrEqual(4.5);
    });
  });
});
