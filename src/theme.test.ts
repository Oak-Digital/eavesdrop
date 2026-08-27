// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { applyTheme, initializeTheme, isThemePreference } from "./theme";

const storedValues = new Map<string, string>();

beforeEach(() => {
  storedValues.clear();
  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: {
      getItem: (key: string) => storedValues.get(key) ?? null,
      setItem: (key: string, value: string) => storedValues.set(key, value),
      clear: () => storedValues.clear(),
    },
  });
});

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
});

describe("theme preference", () => {
  it("applies and stores a chosen theme", () => {
    applyTheme("light");

    expect(document.documentElement.dataset.theme).toBe("light");
    expect(window.localStorage.getItem("eavesdrop-theme")).toBe("light");
  });

  it("restores a stored theme during startup", () => {
    window.localStorage.setItem("eavesdrop-theme", "system");

    const stopSynchronizing = initializeTheme();

    expect(document.documentElement.dataset.theme).toBe("system");
    stopSynchronizing();
  });

  it("rejects unknown preferences", () => {
    expect(isThemePreference("sepia")).toBe(false);
  });
});
