// HID keyboard usages (page 7) by KeyboardEvent.code, which names the
// physical key whatever the OS layout, with US keycap legends.

const series = (count, code, usage, label) =>
  Array.from({ length: count }, (_, i) => [code(i), usage + i, label(i)]);

const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS = "1234567890";

const KEYS = [
  ...series(26, (i) => `Key${LETTERS[i]}`, 0x04, (i) => LETTERS[i]),
  ...series(10, (i) => `Digit${DIGITS[i]}`, 0x1e, (i) => DIGITS[i]),
  ["Enter", 0x28, "Enter"], ["Escape", 0x29, "Esc"], ["Backspace", 0x2a, "Backspace"],
  ["Tab", 0x2b, "Tab"], ["Space", 0x2c, "Space"], ["Minus", 0x2d, "-"], ["Equal", 0x2e, "="],
  ["BracketLeft", 0x2f, "["], ["BracketRight", 0x30, "]"], ["Backslash", 0x31, "\\"],
  ["Semicolon", 0x33, ";"], ["Quote", 0x34, "'"], ["Backquote", 0x35, "`"], ["Comma", 0x36, ","],
  ["Period", 0x37, "."], ["Slash", 0x38, "/"], ["CapsLock", 0x39, "Caps Lock"],
  ...series(12, (i) => `F${i + 1}`, 0x3a, (i) => `F${i + 1}`),
  ["PrintScreen", 0x46, "PrtSc"], ["ScrollLock", 0x47, "ScrLk"], ["Pause", 0x48, "Pause"],
  ["Insert", 0x49, "Ins"], ["Home", 0x4a, "Home"], ["PageUp", 0x4b, "PgUp"],
  ["Delete", 0x4c, "Del"], ["End", 0x4d, "End"], ["PageDown", 0x4e, "PgDn"],
  ["ArrowRight", 0x4f, "→"], ["ArrowLeft", 0x50, "←"], ["ArrowDown", 0x51, "↓"], ["ArrowUp", 0x52, "↑"],
  ["NumLock", 0x53, "Num Lock"], ["NumpadDivide", 0x54, "Num /"], ["NumpadMultiply", 0x55, "Num *"],
  ["NumpadSubtract", 0x56, "Num -"], ["NumpadAdd", 0x57, "Num +"], ["NumpadEnter", 0x58, "Num Enter"],
  ...series(10, (i) => `Numpad${DIGITS[i]}`, 0x59, (i) => `Num ${DIGITS[i]}`),
  ["NumpadDecimal", 0x63, "Num ."], ["IntlBackslash", 0x64, "ISO \\"], ["ContextMenu", 0x65, "Menu"],
  ...series(12, (i) => `F${i + 13}`, 0x68, (i) => `F${i + 13}`),
  ["ControlLeft", 0xe0, "Ctrl"], ["ShiftLeft", 0xe1, "Shift"], ["AltLeft", 0xe2, "Alt"],
  ["MetaLeft", 0xe3, "Win"], ["ControlRight", 0xe4, "Right Ctrl"], ["ShiftRight", 0xe5, "Right Shift"],
  ["AltRight", 0xe6, "Right Alt"], ["MetaRight", 0xe7, "Right Win"],
];

const byCode = new Map(KEYS.map(([code, usage]) => [code, usage]));
const byUsage = new Map(KEYS.map(([, usage, label]) => [usage, label]));
// The layout gives Fn the vendor-specific usage 0xAF.
byUsage.set(0xaf, "Fn");

export const usageOf = (code) => byCode.get(code);

export const usageLabel = (usage) => byUsage.get(usage) ?? `0x${usage.toString(16).padStart(2, "0")}`;

export const isModifier = (usage) => usage >= 0xe0 && usage <= 0xe7;

// Modifier bit i is usage 0xE0 + i.
export const modifierBit = (usage) => 1 << (usage - 0xe0);

export const modifierLabels = (bits) =>
  Array.from({ length: 8 }, (_, i) => 0xe0 + i)
    .filter((usage) => bits & modifierBit(usage))
    .map(usageLabel);
