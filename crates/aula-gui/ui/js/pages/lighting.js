import { extent } from "../board.js";
import { $, bind, element, hexToRgb } from "../controls.js";
import { info, run } from "../device.js";
import { t } from "../i18n.js";
import { remember, save, store } from "../store.js";

export function lighting({ redraw }) {
  const light = store.lighting;
  const effects = $("#effects");

  const renderEffects = () => {
    for (const button of effects.children) {
      button.setAttribute("aria-checked", (button.value || null) === light.effect);
    }
  };

  for (const effect of ["", ...info.effects]) {
    const button = element("button", { text: t(effect ? `effect.${effect}` : "lighting.off") });
    button.value = effect;
    button.setAttribute("role", "radio");
    button.addEventListener("click", () => {
      light.effect = effect || null;
      save();
      renderEffects();
      redraw();
    });
    effects.append(button);
  }
  renderEffects();

  $("#light-brightness").max = info.maxLevel;
  $("#light-speed").max = info.maxLevel;
  bind($("#light-color"), light, "color", redraw);
  bind($("#light-colorful"), light, "colorful", redraw);
  bind($("#light-brightness"), light, "brightness", redraw);
  bind($("#light-speed"), light, "speed");

  $("#light-apply").addEventListener("click", async () => {
    const lighting = { ...light, color: hexToRgb(light.color) };
    if (await run("set_lighting", { lighting }, t("lighting.done"))) {
      remember("light", { kind: "effect", lighting });
    }
  });

  const { width } = extent(info.keys);
  return {
    // A still preview: the colour and brightness, not the animation.
    view(key) {
      if (!light.effect || light.brightness === 0) return {};
      const hue = Math.round((key.rect[0] / width) * 300);
      return {
        glow: light.colorful ? `hsl(${hue} 90% 55%)` : light.color,
        intensity: light.brightness / info.maxLevel,
      };
    },
  };
}
