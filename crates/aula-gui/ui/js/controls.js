import { save } from "./store.js";

export const $ = (selector) => document.querySelector(selector);
export const $$ = (selector) => [...document.querySelectorAll(selector)];

export const hexToRgb = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  return { r: n >> 16, g: (n >> 8) & 0xff, b: n & 0xff };
};

export const rgbToHex = ({ r, g, b }) =>
  `#${[r, g, b].map((c) => c.toString(16).padStart(2, "0")).join("")}`;

export const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

export function element(tag, { className, text } = {}) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (text !== undefined) el.textContent = text;
  return el;
}

// Keeps `object[field]` in sync with an input and saves on every change. A
// range shows its value in the <output> that follows it.
export function bind(input, object, field, onChange = () => {}) {
  const property = input.type === "checkbox" ? "checked" : "value";
  const numeric = input.type === "range" || input.type === "number";
  const output = input.type === "range" ? input.nextElementSibling : null;
  const show = () => output && (output.value = input.value);

  input[property] = object[field];
  show();
  input.addEventListener("input", () => {
    object[field] = numeric ? Number(input.value) : input[property];
    save();
    show();
    onChange();
  });
}

// A row of buttons where one, identified by its `value`, is pressed.
export function segmented(container, current, onChange) {
  const buttons = [...container.querySelectorAll("button")];
  const select = (value) => buttons.forEach((b) => b.setAttribute("aria-pressed", b.value === value));
  select(current);
  for (const button of buttons) {
    button.addEventListener("click", () => {
      select(button.value);
      onChange(button.value);
    });
  }
}

export function setOptions(select, entries) {
  select.replaceChildren(...entries.map(([value, label]) => new Option(label, value)));
}

let toastTimer;
export function toast(message, { error = false } = {}) {
  const el = $("#toast");
  el.textContent = message;
  el.classList.toggle("error", error);
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), error ? 8000 : 3000);
}
