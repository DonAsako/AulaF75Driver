// Light or dark, following the system unless one is chosen in the settings.

import { store } from "./store.js";

const systemDark = matchMedia("(prefers-color-scheme: dark)");

export function applyTheme() {
  document.documentElement.dataset.theme = store.theme ?? (systemDark.matches ? "dark" : "light");
}

// Applied on import, before the rest of the interface loads.
applyTheme();
systemDark.addEventListener("change", applyTheme);
