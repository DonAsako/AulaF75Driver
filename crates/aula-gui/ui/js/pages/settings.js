import { $, bind, setOptions } from "../controls.js";
import { info, run } from "../device.js";
import { LANGUAGES, t } from "../i18n.js";
import { remember, save, store } from "../store.js";
import { applyTheme } from "../theme.js";

export function settings() {
  const { settings } = store;
  $("#set-debounce").max = info.maxDebounce;
  bind($("#set-win"), settings, "disableWin");
  bind($("#set-alt-tab"), settings, "disableAltTab");
  bind($("#set-alt-f4"), settings, "disableAltF4");
  bind($("#set-fn"), settings, "fnMode");
  bind($("#set-sleep"), settings, "sleep");
  bind($("#set-debounce"), settings, "debounce");

  $("#settings-apply").addEventListener("click", async () => {
    if (await run("set_settings", { settings }, t("settings.done"))) remember("settings", settings);
  });

  // Every string is placed at startup, so a new language takes a reload.
  const picker = $("#app-language");
  setOptions(picker, [["", t("settings.languageSystem")], ...Object.entries(LANGUAGES)]);
  picker.value = store.language ?? "";
  picker.addEventListener("change", () => {
    store.language = picker.value || null;
    save();
    location.reload();
  });

  const theme = $("#app-theme");
  setOptions(theme, [["", t("settings.themeSystem")], ["light", t("settings.themeLight")], ["dark", t("settings.themeDark")]]);
  theme.value = store.theme ?? "";
  theme.addEventListener("change", () => {
    store.theme = theme.value || null;
    save();
    applyTheme();
  });

  return {};
}
