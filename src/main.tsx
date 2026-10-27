import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import App from "./App";
import "./styles.css";
import { initializeTheme } from "./theme";

initializeTheme();

// Only the native Mac library has a visual-effect view behind the webview.
// Browser previews, Windows, and the quick recorder keep their opaque canvas.
if ("__TAURI_INTERNALS__" in window && /Macintosh|Mac OS X/.test(navigator.userAgent)
  && new URLSearchParams(window.location.search).get("surface") !== "quick") {
  document.documentElement.dataset.nativeSidebar = "true";
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
