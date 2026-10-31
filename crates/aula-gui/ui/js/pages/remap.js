import { $, $$, clamp, segmented, setOptions } from "../controls.js";
import { info, run } from "../device.js";
import { isModifier, modifierBit, modifierLabels, usageLabel, usageOf } from "../hid.js";
import { t } from "../i18n.js";
import { remember, save, store } from "../store.js";
import { macroTitle } from "./macros.js";

// Legends on the keycap, in English like the printed ones. The editor names
// them through the `media.*` and `mouse.*` strings.
const MEDIA = { play: "Play", stop: "Stop", prev: "Prev", next: "Next", volup: "Vol +", voldown: "Vol −", mute: "Mute" };

const MOUSE = {
  left: "LMB",
  right: "RMB",
  middle: "MMB",
  button4: "MB4",
  button5: "MB5",
  doubleClick: "2×LMB",
  scrollUp: "Wheel ↑",
  scrollDown: "Wheel ↓",
};

const PLAY = ["once", "times", "untilPressedAgain", "whileHeld", "click"];

const MULTI_KEYS = 3;
const MAX_TIMES = 255;

function actionLabel(action) {
  switch (action.kind) {
    case "disabled":
      return "Off";
    case "key":
      return [...modifierLabels(action.value.modifiers), usageLabel(action.value.usage)].join("+");
    case "consumer": {
      const media = info.mediaKeys.find(([, usage]) => usage === action.value);
      return media ? MEDIA[media[0]] : `0x${action.value.toString(16)}`;
    }
    case "mouse":
      return MOUSE[action.value];
    case "multiKey":
      return action.value.filter(Boolean).map(usageLabel).join("+");
    case "macro":
      return `M${action.value.index + 1}`;
  }
}

// Calls `onDone` with the usages pressed, in order, once all are released.
function captureKeys(onDone) {
  const held = new Set();
  const pressed = [];
  const down = (event) => {
    event.preventDefault();
    const usage = usageOf(event.code);
    if (usage === undefined) return;
    held.add(usage);
    if (!pressed.includes(usage)) pressed.push(usage);
  };
  const up = (event) => {
    event.preventDefault();
    held.delete(usageOf(event.code));
    if (held.size > 0 || pressed.length === 0) return;
    window.removeEventListener("keydown", down, true);
    window.removeEventListener("keyup", up, true);
    onDone(pressed);
  };
  window.addEventListener("keydown", down, true);
  window.addEventListener("keyup", up, true);
}

// Modifiers become bits; a lone modifier is sent as the key itself.
function shortcut(pressed) {
  const usage = pressed.filter((u) => !isModifier(u)).at(-1) ?? pressed.at(-1);
  const modifiers = pressed
    .filter((u) => isModifier(u) && u !== usage)
    .reduce((bits, u) => bits | modifierBit(u), 0);
  return { kind: "key", value: { modifiers, usage } };
}

export function remap({ redraw }) {
  let selected = null;
  let capturing = false;
  const keymap = () => store.keymaps[store.layer];

  setOptions($("#remap-media"), info.mediaKeys.map(([name, usage]) => [usage, t(`media.${name}`)]));
  setOptions($("#remap-mouse"), Object.keys(MOUSE).map((name) => [name, t(`mouse.${name}`)]));
  setOptions($("#remap-play"), PLAY.map((kind) => [kind, t(`play.${kind}`)]));
  $("#remap-times").max = MAX_TIMES;

  function renderApply() {
    $("#remap-apply").textContent = t(store.layer === "base" ? "remap.applyBase" : "remap.applyFn");
  }

  function renderEditor() {
    const key = info.keys.find((k) => k.slot === selected);
    const action = key && keymap()[key.slot];
    const kind = action?.kind ?? "default";

    $("#remap-key").hidden = !key;
    $("#remap-no-key").hidden = Boolean(key);
    $("#remap-action").hidden = !key;
    if (!key) return;

    $("#remap-key").textContent = usageLabel(key.usage);
    $("#remap-kind").value = kind;
    $("#remap-kind").querySelector('[value="macro"]').disabled = store.macros.length === 0;
    for (const el of $$("#remap-action [data-for]")) {
      el.hidden = !el.dataset.for.split(" ").includes(kind);
    }
    if (!capturing) $("#remap-capture").textContent = action ? actionLabel(action) : "";

    setOptions($("#remap-macro"), store.macros.map((macro, i) => [i, macroTitle(macro, i)]));
    if (kind === "consumer") $("#remap-media").value = action.value;
    if (kind === "mouse") $("#remap-mouse").value = action.value;
    if (kind === "macro") {
      const { index, play } = action.value;
      $("#remap-macro").value = index;
      $("#remap-play").value = play.kind;
      $("#remap-times-prop").hidden = play.kind !== "times";
      $("#remap-times").value = play.value ?? 2;
    }
  }

  function setAction(action) {
    if (action) keymap()[selected] = action;
    else delete keymap()[selected];
    save();
    renderEditor();
    redraw();
  }

  // What an action kind starts as when picked.
  function initialAction(kind) {
    const { usage } = info.keys.find((k) => k.slot === selected);
    switch (kind) {
      case "default": return null;
      case "disabled": return { kind };
      case "key": return { kind, value: { modifiers: 0, usage } };
      case "consumer": return { kind, value: info.mediaKeys[0][1] };
      case "mouse": return { kind, value: "left" };
      case "multiKey": return { kind, value: [usage, 0, 0] };
      case "macro": return { kind, value: { index: 0, play: { kind: "once" } } };
    }
  }

  function setMacroAction() {
    const kind = $("#remap-play").value;
    const times = clamp(Number($("#remap-times").value) || 1, 1, MAX_TIMES);
    const play = kind === "times" ? { kind, value: times } : { kind };
    setAction({ kind: "macro", value: { index: Number($("#remap-macro").value), play } });
  }

  segmented($("#remap-layer"), store.layer, (layer) => {
    store.layer = layer;
    save();
    renderApply();
    renderEditor();
    redraw();
  });
  renderApply();

  $("#remap-kind").addEventListener("change", (e) => setAction(initialAction(e.target.value)));
  $("#remap-media").addEventListener("change", (e) => setAction({ kind: "consumer", value: Number(e.target.value) }));
  $("#remap-mouse").addEventListener("change", (e) => setAction({ kind: "mouse", value: e.target.value }));
  for (const id of ["#remap-macro", "#remap-play", "#remap-times"]) {
    $(id).addEventListener("change", setMacroAction);
  }

  $("#remap-capture").addEventListener("click", (event) => {
    if (capturing) return;
    capturing = true;
    event.currentTarget.blur();
    const multi = $("#remap-kind").value === "multiKey";
    event.currentTarget.textContent = multi ? t("remap.captureMulti", { count: MULTI_KEYS }) : t("remap.capture");
    captureKeys((pressed) => {
      capturing = false;
      if (!multi) return setAction(shortcut(pressed));
      const usages = pressed.slice(0, MULTI_KEYS);
      setAction({ kind: "multiKey", value: [...usages, 0, 0, 0].slice(0, MULTI_KEYS) });
    });
  });

  $("#remap-reset").addEventListener("click", () => {
    store.keymaps[store.layer] = {};
    save();
    renderEditor();
    redraw();
  });

  $("#remap-apply").addEventListener("click", async () => {
    const { layer } = store;
    const keys = Object.entries(keymap()).map(([slot, action]) => [Number(slot), action]);
    if (await run("set_keymap", { layer, keys }, t("remap.done"))) remember(layer, keys);
  });

  return {
    view(key) {
      const action = keymap()[key.slot];
      return { sub: action ? actionLabel(action) : "", selected: key.slot === selected };
    },
    press(slot) {
      selected = slot;
      renderEditor();
      redraw();
    },
    show: renderEditor,
  };
}
