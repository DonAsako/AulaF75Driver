import { $ } from "../controls.js";
import { listen, localTime, refreshButtons, run } from "../device.js";
import { t } from "../i18n.js";

export function screen() {
  const apply = $("#screen-apply");
  const preview = $("#screen-preview");
  let gif = null;
  // The first progress report, once the GIF is decoded: the rate is measured from it.
  let first = null;

  apply.dataset.blocked = "true";

  $("#screen-file").addEventListener("change", async (event) => {
    const [file] = event.target.files;
    if (!file) return;
    gif = new Uint8Array(await file.arrayBuffer());
    $("#screen-file-name").textContent = file.name;
    URL.revokeObjectURL(preview.src);
    preview.src = URL.createObjectURL(file);
    apply.dataset.blocked = "false";
    refreshButtons();
  });

  listen("screen-progress", ({ payload: [sent, total] }) => {
    const now = performance.now();
    first ??= { now, sent };
    $("#screen-bar").max = total;
    $("#screen-bar").value = sent;
    if (sent === first.sent) {
      $("#screen-progress-text").textContent = t("screen.sent", { sent, total });
      return;
    }
    const seconds = Math.round(((now - first.now) / 1000 / (sent - first.sent)) * (total - sent));
    $("#screen-progress-text").textContent = t("screen.progress", { sent, total, seconds });
  });

  apply.addEventListener("click", async () => {
    first = null;
    $("#screen-bar").removeAttribute("value");
    $("#screen-progress-text").textContent = t("screen.preparing");
    $("#screen-progress").hidden = false;
    await run("upload_screen", gif, t("screen.done"));
    $("#screen-progress").hidden = true;
  });

  $("#clock-sync").addEventListener("click", () => {
    run("sync_clock", { time: localTime() }, t("screen.clockDone"));
  });

  return {};
}
