import { $, $$, clamp, element } from "../controls.js";
import { info, run } from "../device.js";
import { usageLabel, usageOf } from "../hid.js";
import { t } from "../i18n.js";
import { remember, save, store } from "../store.js";

const MAX_DELAY_MS = 65535;

const mouseButton = (button) => t(`mouse.${button}`);

// [action string, how the event names its key or button]
const EVENTS = {
  keyDown: ["macros.press", usageLabel],
  keyUp: ["macros.release", usageLabel],
  mouseDown: ["macros.press", mouseButton],
  mouseUp: ["macros.release", mouseButton],
};

export const macroTitle = (macro, index) => macro.name || t("macros.default", { number: index + 1 });

const delay = (ms) => clamp(Math.round(ms) || info.minDelayMs, info.minDelayMs, MAX_DELAY_MS);

const step = (kind, value) => ({ event: { kind, value }, delayMs: info.minDelayMs });

export function macros() {
  let current = store.macros.length ? 0 : null;
  let recording = null;
  const macro = () => store.macros[current];

  function stepRow({ event }, index, steps) {
    const [action, describe] = EVENTS[event.kind];
    const pause = element("input");
    pause.type = "number";
    pause.min = info.minDelayMs;
    pause.max = MAX_DELAY_MS;
    pause.value = steps[index].delayMs;
    pause.setAttribute("aria-label", t("macros.pause"));
    pause.addEventListener("change", () => {
      steps[index].delayMs = delay(Number(pause.value));
      pause.value = steps[index].delayMs;
      save();
    });

    const remove = element("button", { className: "icon", text: "✕" });
    remove.setAttribute("aria-label", t("macros.removeStep"));
    remove.addEventListener("click", () => {
      steps.splice(index, 1);
      save();
      render();
    });

    const row = element("tr");
    for (const content of [t(action), describe(event.value), pause, remove]) {
      const cell = element("td");
      cell.append(content);
      row.append(cell);
    }
    return row;
  }

  function render() {
    $("#macro-list").replaceChildren(...store.macros.map((m, i) => {
      const button = element("button");
      button.append(element("span", { className: "tag", text: `M${i + 1}` }), macroTitle(m, i));
      button.setAttribute("aria-current", i === current);
      button.addEventListener("click", () => {
        stopRecording();
        current = i;
        render();
      });
      const item = element("li");
      item.append(button);
      return item;
    }));

    $("#macro-add").disabled = store.macros.length >= info.maxMacros;
    $("#macro-none").hidden = store.macros.length > 0;
    $("#macro-editor").hidden = current === null;
    $("#macro-delete").hidden = current === null;
    if (current === null) return;

    const { name, steps } = macro();
    if ($("#macro-name").value !== name) $("#macro-name").value = name;
    $("#macro-name").placeholder = macroTitle({}, current);
    $("#macro-record").textContent = t(recording ? "macros.stop" : "macros.record");
    $("#macro-record").classList.toggle("recording", Boolean(recording));
    $("#macro-steps").hidden = steps.length === 0;
    $("#macro-no-steps").hidden = steps.length > 0;
    $("#macro-steps tbody").replaceChildren(...steps.map(stepRow));
  }

  // Each recorded event pauses until the next one.
  function recorder(kind) {
    return (event) => {
      const usage = usageOf(event.code);
      if (event.repeat || usage === undefined) return;
      event.preventDefault();
      const { steps } = macro();
      const now = performance.now();
      if (recording.last !== null) steps.at(-1).delayMs = delay(now - recording.last);
      recording.last = now;
      steps.push(step(kind, usage));
      save();
      render();
    };
  }

  function stopRecording() {
    if (!recording) return;
    window.removeEventListener("keydown", recording.down, true);
    window.removeEventListener("keyup", recording.up, true);
    recording = null;
  }

  $("#macro-record").addEventListener("click", (event) => {
    event.currentTarget.blur();
    if (recording) {
      stopRecording();
    } else {
      recording = { last: null, down: recorder("keyDown"), up: recorder("keyUp") };
      window.addEventListener("keydown", recording.down, true);
      window.addEventListener("keyup", recording.up, true);
    }
    render();
  });

  for (const button of $$("[data-click]")) {
    button.addEventListener("click", () => {
      const { click } = button.dataset;
      macro().steps.push(step("mouseDown", click), step("mouseUp", click));
      save();
      render();
    });
  }

  $("#macro-add").addEventListener("click", () => {
    stopRecording();
    store.macros.push({ name: "", steps: [] });
    current = store.macros.length - 1;
    save();
    render();
  });

  $("#macro-name").addEventListener("input", (event) => {
    macro().name = event.target.value;
    save();
    render();
  });

  // Keys bound to the deleted macro lose their binding; later macros move up.
  $("#macro-delete").addEventListener("click", () => {
    stopRecording();
    const removed = current;
    store.macros.splice(removed, 1);
    for (const keymap of Object.values(store.keymaps)) {
      for (const [slot, action] of Object.entries(keymap)) {
        if (action.kind !== "macro") continue;
        if (action.value.index === removed) delete keymap[slot];
        else if (action.value.index > removed) action.value.index -= 1;
      }
    }
    current = store.macros.length ? Math.min(removed, store.macros.length - 1) : null;
    save();
    render();
  });

  $("#macros-apply").addEventListener("click", async () => {
    stopRecording();
    render();
    const steps = store.macros.map((m) => m.steps);
    if (await run("set_macros", { macros: steps }, t("macros.done"))) remember("macros", steps);
  });

  return { show: render, hide: stopRecording };
}
