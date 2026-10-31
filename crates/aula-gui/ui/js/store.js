// What the user configured, kept in the webview's storage: the keyboard
// cannot report its keymaps, macros or settings back.

const STORAGE_KEY = "aula-f75";

const DEFAULTS = {
  tab: "lighting",
  // null follows the system for both.
  language: null,
  theme: null,
  // `direction` is sent as 0: which values exist is unknown.
  lighting: { effect: "Static", color: "#ffffff", colorful: false, brightness: 5, speed: 3, direction: 0 },
  keyColors: {},
  keyBrightness: 5,
  layer: "base",
  keymaps: { base: {}, fn: {} },
  macros: [],
  settings: {
    disableAltTab: false,
    disableAltF4: false,
    disableWin: false,
    fnMode: "momentary",
    sleep: "never",
    debounce: 2,
  },
  // What the keyboard last accepted, sent again by a resync: the settings,
  // the macro steps, the keys of each layer, and the lighting as
  // { kind: "effect", lighting } or { kind: "keys", brightness, colors }.
  applied: { settings: null, macros: null, base: null, fn: null, light: null },
};

function load() {
  try {
    return { ...structuredClone(DEFAULTS), ...JSON.parse(localStorage.getItem(STORAGE_KEY)) };
  } catch {
    return structuredClone(DEFAULTS);
  }
}

export const store = load();

export function remember(field, value) {
  store.applied[field] = structuredClone(value);
  save();
}

export function save() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  } catch {
    // Without storage the configuration lasts for the session only.
  }
}
