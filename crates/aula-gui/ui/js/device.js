// The Rust commands, one at a time, and the keyboard's connection status.

import { $, $$, toast } from "./controls.js";
import { t } from "./i18n.js";

const { invoke } = window.__TAURI__.core;
export const { listen } = window.__TAURI__.event;

const STATUS_INTERVAL_MS = 15000;

let busy = false;
let connection = null;

export const info = await invoke("info");

// A [data-device] button works when the keyboard is connected and idle, over
// USB for [data-usb-only], and when its page has not set [data-blocked].
export function refreshButtons() {
  for (const button of $$("[data-device]")) {
    const wrongConnection = button.hasAttribute("data-usb-only") && connection !== "usb";
    button.disabled = busy || !connection || wrongConnection || button.dataset.blocked === "true";
  }
}

function setBusy(value) {
  busy = value;
  refreshButtons();
}

// Runs a command and reports the outcome. Resolves to { value } on success
// and to undefined on error.
export async function run(command, args, success) {
  setBusy(true);
  try {
    const value = await invoke(command, args);
    toast(success);
    return { value };
  } catch (error) {
    toast(String(error), { error: true });
    return undefined;
  } finally {
    setBusy(false);
  }
}

function showStatus(status, error) {
  const el = $("#status");
  el.dataset.state = status ? "connected" : "missing";
  el.title = error ? String(error) : "";
  $("#status-text").textContent = t(status ? `status.${status.connection}` : "status.missing");

  const battery = $("#battery");
  battery.hidden = status?.battery == null;
  if (!battery.hidden) {
    battery.style.setProperty("--level", status.battery / 100);
    $("#battery-text").textContent = t("status.battery", { percent: status.battery });
  }
}

// The clock command's time, from the local clock. Weekday 0 is Sunday.
export function localTime() {
  const now = new Date();
  return {
    year: now.getFullYear(),
    month: now.getMonth() + 1,
    day: now.getDate(),
    hour: now.getHours(),
    minute: now.getMinutes(),
    second: now.getSeconds(),
    weekday: now.getDay(),
  };
}

export function watchStatus() {
  const refresh = async () => {
    if (busy) return;
    try {
      const status = await invoke("status");
      connection = status.connection;
      showStatus(status);
    } catch (error) {
      connection = null;
      showStatus(null, error);
    }
    refreshButtons();
  };
  refresh();
  setInterval(refresh, STATUS_INTERVAL_MS);
}
