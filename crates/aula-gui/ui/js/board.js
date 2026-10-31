// The drawn keyboard. A page decides what each key shows through `view(key)`
// and receives clicks through `press(slot)` and `drag(slot)`.

import { element } from "./controls.js";
import { usageLabel } from "./hid.js";

export const extent = (keys) => ({
  width: Math.max(...keys.map(({ rect: [x, , w] }) => x + w)),
  height: Math.max(...keys.map(({ rect: [, y, , h] }) => y + h)),
});

const percent = (value, total) => `${(value / total) * 100}%`;

export function createBoard(el, keys) {
  const { width, height } = extent(keys);
  el.style.aspectRatio = `${width} / ${height}`;

  const caps = keys.map((key) => {
    const [x, y, w, h] = key.rect;
    const cap = element("button", { className: "key" });
    cap.dataset.slot = key.slot;
    cap.setAttribute("aria-label", usageLabel(key.usage));
    Object.assign(cap.style, {
      left: percent(x, width),
      top: percent(y, height),
      width: percent(w, width),
      height: percent(h, height),
    });
    const label = element("span", { className: "sub" });
    cap.append(element("span", { className: "legend", text: usageLabel(key.usage) }), label);
    el.append(cap);
    return { key, cap, label };
  });

  let page = {};
  let dragging = false;
  const slotAt = (event) => Number(event.target.closest(".key")?.dataset.slot);

  el.addEventListener("pointerdown", (event) => {
    const slot = slotAt(event);
    if (!slot || !page.press) return;
    dragging = true;
    page.press(slot);
  });
  el.addEventListener("pointerover", (event) => {
    const slot = slotAt(event);
    if (dragging && slot) page.drag?.(slot);
  });
  window.addEventListener("pointerup", () => (dragging = false));

  return {
    show(next) {
      page = next;
      el.dataset.interactive = Boolean(page.press);
      for (const { key, cap, label } of caps) {
        const { glow, intensity = 1, sub = "", selected = false } = page.view(key);
        cap.classList.toggle("lit", Boolean(glow));
        if (glow) {
          cap.style.setProperty("--glow", glow);
          cap.style.setProperty("--intensity", intensity);
        }
        label.textContent = sub;
        cap.classList.toggle("remapped", Boolean(sub));
        cap.classList.toggle("selected", selected);
      }
    },
  };
}
