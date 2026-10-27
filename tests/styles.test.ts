import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");

type Theme = "dark" | "light" | "system-light";
const themes: Theme[] = ["dark", "light", "system-light"];

function token(name: string, theme: Theme, seen: string[] = []): string {
  const base = styles.split("}")[0];
  const selector = theme === "system-light" ? ':root[data-theme="system"]' : ':root[data-theme="light"]';
  const palette = theme === "dark" ? base : styles.split(selector)[1].split("}")[0];
  const declaration = new RegExp(`--${name}:\\s*([^;]+);`, "i");
  const value = (palette.match(declaration) ?? base.match(declaration))?.[1].trim();
  if (!value) throw new Error(`Missing CSS token --${name}`);
  if (seen.includes(name)) throw new Error(`Circular CSS token --${name}`);
  const alias = value.match(/^var\(--([\w-]+)\)$/)?.[1];
  if (alias) return token(alias, theme, [...seen, name]);
  if (!/^#[0-9a-f]{6}$/i.test(value)) throw new Error(`Unsupported CSS color --${name}: ${value}`);
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
  it.each(themes)("keeps supporting text readable on %s surfaces", (theme) => {
    const backgrounds = ["bg-canvas", "bg-base", "bg-surface", "bg-elevated", "bg-muted", "bg-hover", "bg-active", "bg-selected"];

    ["text-primary", "text-secondary", "text-tertiary"].forEach((foreground) => {
      backgrounds.forEach((background) => {
        expect(contrast(token(foreground, theme), token(background, theme)), `${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5);
      });
    });
  });

  it.each(themes)("keeps action labels readable in %s mode", (theme) => {
    [["accent-fg", "accent"], ["accent-fg", "accent-hover"], ["danger-fg", "danger"]].forEach(([foreground, background]) => {
      expect(contrast(token(foreground, theme), token(background, theme)), `${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5);
    });
  });
});
