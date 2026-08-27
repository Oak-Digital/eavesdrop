import type { ThemePreference } from "./types";

const THEME_STORAGE_KEY = "eavesdrop-theme";

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === "light" || value === "dark" || value === "system";
}

export function applyTheme(theme: ThemePreference) {
  document.documentElement.dataset.theme = theme;
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // The database remains the source of truth when web storage is unavailable.
  }
}

export function initializeTheme() {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (isThemePreference(stored)) applyTheme(stored);
  } catch {
    // The dark CSS default is used until the persisted app settings load.
  }

  const synchronizeWindow = (event: StorageEvent) => {
    if (event.key === THEME_STORAGE_KEY && isThemePreference(event.newValue)) {
      document.documentElement.dataset.theme = event.newValue;
    }
  };
  window.addEventListener("storage", synchronizeWindow);
  return () => window.removeEventListener("storage", synchronizeWindow);
}
