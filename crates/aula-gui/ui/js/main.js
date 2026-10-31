// First, so the theme is set before anything renders.
import "./theme.js";
import { createBoard } from "./board.js";
import { $, $$ } from "./controls.js";
import { info, localTime, refreshButtons, run, watchStatus } from "./device.js";
import { t, translatePage } from "./i18n.js";
import { colors } from "./pages/colors.js";
import { lighting } from "./pages/lighting.js";
import { macros } from "./pages/macros.js";
import { remap } from "./pages/remap.js";
import { screen } from "./pages/screen.js";
import { settings } from "./pages/settings.js";
import { save, store } from "./store.js";

translatePage();

// A page may offer `view`, `press` and `drag` for the keyboard (see board.js),
// and `show` / `hide` to run when it is opened or left.
const board = createBoard($("#board"), info.keys);
let active = {};
const redraw = () => board.show(active);

const pages = {
  lighting: lighting({ redraw }),
  colors: colors({ redraw }),
  remap: remap({ redraw }),
  macros: macros(),
  screen: screen(),
  settings: settings(),
};

function open(name) {
  active.hide?.();
  active = pages[name];
  store.tab = name;
  save();

  for (const tab of $$("#tabs button")) tab.setAttribute("aria-selected", tab.value === name);
  for (const page of $$("[data-page]")) page.hidden = page.dataset.page !== name;
  document.body.dataset.tab = name;
  $("#stage").hidden = !active.view;

  active.show?.();
  if (active.view) redraw();
}

$("#tabs").addEventListener("click", (event) => {
  const tab = event.target.closest("button");
  if (tab) open(tab.value);
});

// The keyboard forgets nothing on its own, but another app or a reset may
// have changed it.
$("#resync").addEventListener("click", () => {
  const { settings, macros, base, fn, light } = store.applied;
  const keymaps = Object.entries({ base, fn }).filter(([, keys]) => keys);
  run("resync", { snapshot: { settings, macros, keymaps, light, time: localTime() } }, t("resync.done"));
});

open(pages[store.tab] ? store.tab : "lighting");
refreshButtons();
watchStatus();
