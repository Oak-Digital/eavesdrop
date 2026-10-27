import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");

function token(name: string, theme: "dark" | "light") {
  const palette = theme === "light" ? styles.split(':root[data-theme="light"]')[1].split("}")[0] : styles.split("}")[0];
  const value = palette.match(new RegExp(`--${name}:\\s*(#[0-9a-f]{6})`, "i"))?.[1];
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
  it.each(["dark", "light"] as const)("keeps supporting text readable on %s surfaces", (theme) => {
    const backgrounds = ["bg-canvas", "bg-base", "bg-surface", "bg-elevated", "bg-muted", "bg-hover", "bg-selected"];

    ["text-primary", "text-secondary", "text-tertiary"].forEach((foreground) => {
      backgrounds.forEach((background) => {
        expect(contrast(token(foreground, theme), token(background, theme)), `${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5);
      });
    });
  });

  it.each(["dark", "light"] as const)("keeps action labels readable in %s mode", (theme) => {
    [["accent-fg", "accent"], ["accent-fg", "accent-hover"], ["danger-fg", "danger"]].forEach(([foreground, background]) => {
      expect(contrast(token(foreground, theme), token(background, theme)), `${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5);
    });
  });
});
