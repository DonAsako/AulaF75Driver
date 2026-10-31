// Interface strings. The language follows the system unless one is chosen
// in the settings.

import { de } from "./locales/de.js";
import { en } from "./locales/en.js";
import { es } from "./locales/es.js";
import { fr } from "./locales/fr.js";
import { it } from "./locales/it.js";
import { ja } from "./locales/ja.js";
import { ko } from "./locales/ko.js";
import { zhTW } from "./locales/zh-TW.js";
import { zh } from "./locales/zh.js";
import { store } from "./store.js";

const STRINGS = { en, fr, de, es, it, ja, ko, zh, "zh-TW": zhTW };

// Each language by its own name, for the language picker.
export const LANGUAGES = {
  en: "English",
  fr: "Français",
  de: "Deutsch",
  es: "Español",
  it: "Italiano",
  ja: "日本語",
  ko: "한국어",
  zh: "简体中文",
  "zh-TW": "繁體中文",
};

// The chosen language, then the system's, each tag before its base (zh-TW, zh).
const preferred = [store.language, ...navigator.languages.flatMap((tag) => [tag, tag.split("-")[0]])];
export const language = preferred.find((code) => code in STRINGS) ?? "en";

export const t = (key, params = {}) =>
  (STRINGS[language][key] ?? en[key] ?? key).replace(/\{(\w+)\}/g, (_, name) => params[name]);

// Which element property each attribute's string goes to.
const TARGETS = {
  "data-i18n": "textContent",
  "data-i18n-title": "title",
  "data-i18n-alt": "alt",
  "data-i18n-label": "ariaLabel",
};

export function translatePage() {
  document.documentElement.lang = language;
  for (const [attribute, property] of Object.entries(TARGETS)) {
    for (const el of document.querySelectorAll(`[${attribute}]`)) el[property] = t(el.getAttribute(attribute));
  }
}
