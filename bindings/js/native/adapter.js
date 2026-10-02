import koffi from "koffi";
import { layouts, types } from "./abi.js";
import { enums, methods, models } from "./models.js";

const PREFIX = "__termprobe_native_error__:";
const aliases = { TextStyle: "LocatorStyle", LocatorQuery: "LocatorExpression", LocatorDirection: "LocatorStageDirection" };
const snake = (name) => name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
const optional = (type) => type.startsWith("Option<") ? type.slice(7, -1) : undefined;
const absent = (value) => value === undefined || value === null;
const error = (kind, message, extra = {}) => new Error(PREFIX + JSON.stringify({ kind, message, ...extra }));

function integer(value, label, max = Number.MAX_SAFE_INTEGER, min = 0) {
  if (!Number.isSafeInteger(value) || value < min || value > max) {
    throw error("usage", `${label} must be an integer between ${min} and ${max}`);
  }
  return value;
}

function validate(type, value, label) {
  const inner = optional(type);
  if (inner) return absent(value) ? undefined : validate(inner, value, label);
  if (type.startsWith("Vec<")) {
    if (!Array.isArray(value)) throw new TypeError(`${label} must be an array`);
    return value.map((item, i) => validate(type.slice(4, -1), item, `${label}[${i}]`));
  }
  if (type === "(String,String)") {
    if (!Array.isArray(value) || value.length !== 2) throw new TypeError(`${label} must be a string pair`);
    return value.map((item, i) => validate("String", item, `${label}[${i}]`));
  }
  if (/^(u8|u16|u32|u64|usize|i32)$/.test(type)) {
    const max = { u8: 255, u16: 65535, u32: 4294967295, i32: 2147483647 }[type];
    return integer(value, label, max, type === "i32" ? -2147483648 : 0);
  }
  if (type === "MouseOptions") {
    const code = integer(value, "button", 255);
    if ((code & ~31) !== 0 || (code & 3) === 3) throw error("usage", `invalid mouse button code ${code}`);
    return { button: code & 3, shift: !!(code & 4), alt: !!(code & 8), ctrl: !!(code & 16) };
  }
  const primitive = { String: "string", PathBuf: "string", WhitespaceMode: "string", TraceMode: "string", FailureArtifactMode: "string", bool: "boolean", f64: "number" }[type];
  if (primitive) {
    if (typeof value !== primitive) throw new TypeError(`${label} must be a ${primitive}`);
    return value;
  }
  type = aliases[type] ?? type;
  if (enums[type]) {
    if (!Object.values(enums[type]).includes(value)) throw new TypeError(`invalid ${label}: ${String(value)}`);
    return value;
  }
  const fields = models[type];
  if (!fields) throw error("internal", `unknown native model ${type}`);
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError(`${label} must be an object`);
  if (type.startsWith("Locator")) {
    for (const key of Object.keys(value)) {
      if (!Object.hasOwn(fields, key)) throw new TypeError(`unknown locator field ${JSON.stringify(key)}`);
    }
  }
  return Object.fromEntries(Object.entries(fields).map(([key, [field, fieldType]]) => [
    field, validate(fieldType, value[key], label ? `${label}.${key}` : key),
  ]));
}

// Encoding stores raw pointers in native structs. Keep every backing allocation
// reachable until the start call returns and Rust finishes its input copy.
class Inputs {
  buffers = [];
  release() {
    this.buffers.length = 0;
  }
  string(value) {
    if (absent(value)) return { data: null, len: 0 };
    const bytes = Buffer.from(value, "utf8");
    const data = bytes.length ? bytes : Buffer.alloc(1);
    this.buffers.push(data);
    return { data, len: bytes.length };
  }
  array(type, values = []) {
    if (!values.length) return null;
    const buffer = Buffer.alloc(koffi.sizeof(types[type]) * values.length);
    this.buffers.push(buffer);
    koffi.encode(buffer, types[type], values.map((v) => this.struct(type, v)), values.length);
    return buffer;
  }
  struct(type, value) {
    if (type === "TuiString") return this.string(value);
    if (type.startsWith("TuiOptional")) {
      return { present: !absent(value), value: value ?? (type === "TuiOptionalBool" ? false : 0) };
    }
    const result = {};
    value ??= {};
    for (const [field, fieldType] of Object.entries(layouts[type])) {
      if (field.endsWith("_len")) {
        result[field] = value[field.slice(0, -4)]?.length ?? 0;
      } else if (fieldType.startsWith("*")) {
        const items = value[field] ?? [];
        result[field] = this.array(fieldType.slice(1), items);
      } else if (fieldType.startsWith("Tui")) {
        result[field] = this.struct(fieldType, value[field]);
      } else {
        result[field] = value[field] ?? (fieldType === "bool" ? false : 0);
      }
    }
    return result;
  }
}

function bytes(value) {
  if (value.data === null) return null;
  return Buffer.from(koffi.decode(value.data, "uint8_t", Number(value.len)));
}
function string(value) { return bytes(value)?.toString("utf8") ?? null; }
function array(type, pointer, length) {
  return length ? koffi.decode(pointer, types[type], Number(length)).map((value) => decode(type, value)) : [];
}
function decode(type, value) {
  if (type === "TuiString") return string(value);
  if (type.startsWith("TuiOptional")) return value.present ? Number(value.value) : null;
  if (type === "TuiColor") {
    if (value.kind === 0) return "default";
    if (value.kind === 1) return value.index;
    if (value.kind === 2) return "#" + [value.red, value.green, value.blue].map((v) => v.toString(16).padStart(2, "0")).join("");
    throw error("internal", `unknown native color kind ${value.kind}`);
  }
  const result = {};
  for (const [field, fieldType] of Object.entries(layouts[type])) {
    if (field.endsWith("_len")) continue;
    let decoded;
    if (fieldType.startsWith("*")) {
      decoded = array(fieldType.slice(1), value[field], value[`${field}_len`]);
      if (field === "modes") decoded = Object.fromEntries(decoded.map((m) => [m.name, m.enabled]));
      if (field === "palette") decoded = Object.fromEntries(decoded.map((p) => [p.key, p.value]));
    } else if (fieldType.startsWith("Tui")) decoded = decode(fieldType, value[field]);
    else decoded = typeof value[field] === "bigint" ? Number(value[field]) : value[field];
    result[field === "character" ? "char" : field] = decoded;
  }
  return result;
}

function resultValue(result, selector) {
  switch (selector) {
