import { $, bind, hexToRgb, rgbToHex, segmented } from "../controls.js";
import { info, run } from "../device.js";
import { t } from "../i18n.js";
import { remember, save, store } from "../store.js";

export function colors({ redraw }) {
  let tool = "paint";
  segmented($("#paint-tool"), tool, (value) => (tool = value));

  $("#colors-brightness").max = info.maxLevel;
  bind($("#colors-brightness"), store, "keyBrightness", redraw);

  const update = (keyColors) => {
    store.keyColors = keyColors;
    save();
    redraw();
  };

  const paint = (slot) => {
    const keyColors = { ...store.keyColors };
    if (tool === "paint") keyColors[slot] = $("#paint-color").value;
    else delete keyColors[slot];
    update(keyColors);
  };

  $("#colors-fill").addEventListener("click", () => {
    const color = $("#paint-color").value;
    update(Object.fromEntries(info.keys.map((key) => [key.slot, color])));
  });
  $("#colors-clear").addEventListener("click", () => update({}));

  $("#colors-apply").addEventListener("click", async () => {
    const brightness = store.keyBrightness;
    const colors = Object.entries(store.keyColors).map(([slot, hex]) => [Number(slot), hexToRgb(hex)]);
    if (await run("set_key_colors", { brightness, colors }, t("colors.done"))) {
      remember("light", { kind: "keys", brightness, colors });
    }
  });

  $("#colors-read").addEventListener("click", async () => {
    const reply = await run("read_key_colors", {}, t("colors.readDone"));
    if (!reply) return;
    const onBoard = new Set(info.keys.map((key) => key.slot));
    update(Object.fromEntries(
      reply.value.filter(([slot]) => onBoard.has(slot)).map(([slot, rgb]) => [slot, rgbToHex(rgb)]),
    ));
  });

  return {
    view(key) {
      const glow = store.keyColors[key.slot];
      return glow ? { glow, intensity: Math.max(store.keyBrightness, 1) / info.maxLevel } : {};
    },
    press: paint,
    drag: paint,
  };
}
