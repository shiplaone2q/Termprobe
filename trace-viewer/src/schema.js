/** @typedef {(value: unknown, path: string) => void} Validator */

/** @param {unknown} value @param {string} path @returns {asserts value is Record<string, unknown>} */
function object(value, path) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) invalid(path, "an object");
}
/** @param {string} path @param {string} expected @returns {never} */
function invalid(path, expected) {
  throw new Error(`${path}: expected ${expected}`);
}
/** @param {(value: unknown) => boolean} predicate @param {string} expected @returns {Validator} */
const scalar = (predicate, expected) => (value, path) => {
  if (!predicate(value)) invalid(path, expected);
};
const text = scalar((value) => typeof value === "string", "a string");
const bool = scalar((value) => typeof value === "boolean", "a boolean");
const number = scalar((value) => typeof value === "number" && Number.isFinite(value) && value >= 0, "a non-negative finite number");
const integer = scalar((value) => Number.isSafeInteger(value) && Number(value) >= 0, "a non-negative safe integer");
const positive = scalar((value) => typeof value === "number" && Number.isFinite(value) && value > 0, "a positive finite number");
const dimension = scalar((value) => Number.isSafeInteger(value) && Number(value) > 0, "a positive safe integer");
/** @param {Validator} validate @returns {Validator} */
const optional = (validate) => (value, path) => { if (value !== undefined) validate(value, path); };
/** @param {Validator} validate @returns {Validator} */
const nullable = (validate) => optional((value, path) => { if (value !== null) validate(value, path); });
/** @param {readonly unknown[]} values @returns {Validator} */
const choice = (values) => scalar((value) => values.includes(value), values.map(String).join(" or "));
/** @param {Validator} validate @returns {Validator} */
const list = (validate) => (value, path) => {
  if (!Array.isArray(value)) invalid(path, "an array");
  value.forEach((item, index) => validate(item, `${path}[${index}]`));
};
/** @param {Record<string, Validator>} fields @returns {Validator} */
const shape = (fields) => (value, path) => {
  object(value, path);
  for (const [key, validate] of Object.entries(fields)) validate(value[key], `${path}.${key}`);
};
/** @param {Record<string, Validator>} variants @returns {Validator} */
const tagged = (variants) => (value, path) => {
  object(value, path);
  const kind = value.kind;
  if (typeof kind !== "string" || !Object.hasOwn(variants, kind)) invalid(`${path}.kind`, Object.keys(variants).join(" or "));
  variants[kind](value, path);
};
const position = shape({ column: integer, row: integer });
const size = shape({ cols: dimension, rows: dimension });
const cursor = shape({ column: integer, row: integer, shape: text, visible: bool });
/** @type {Validator} */
const occurrence = (value, path) => typeof value === "string"
  ? choice(["any", "unique", "first", "last"])(value, path) : shape({ nth: integer })(value, path);
const style = shape({
  foreground: nullable(text), background: nullable(text), underline_color: nullable(text), underline_style: nullable(text),
  bold: nullable(bool), dim: nullable(bool), italic: nullable(bool), inverse: nullable(bool), hidden: nullable(bool),
  strikethrough: nullable(bool), blink: nullable(bool),
});
const anchor = shape({ text, regex: bool, occurrence });
const direction = choice(["within", "before", "after"]);
/** @type {Validator} */
function locator(value, path) {
  if (path.split(".").length > 128) invalid(path, "a locator nested at most 64 levels deep");
  shape({ selector, occurrence, direction, style, within: nullable(locator) })(value, path);
}
const selector = tagged({
  text: shape({ selector: shape({ text, regex: bool, full: bool, whitespace: choice(["exact", "normalize"]),
    scope: shape({ after: nullable(anchor), before: nullable(anchor) }) }) }),
  style: shape({ selector: shape({ style, full: bool }) }),
  link: shape({ selector: shape({ uri: text, full: bool }) }),
  and: shape({ selector: shape({ left: locator, right: locator }) }),
  or: shape({ selector: shape({ left: locator, right: locator }) }),
  filter: shape({ selector: shape({ input: locator, has: nullable(locator), has_not: nullable(locator) }) }),
});
const expectation = tagged({
  locator: shape({ query: locator, outcome: choice(["matches", "visible", "hidden", "unique", "actionable"]) }),
  value: shape({ subject: text, expected: text }),
  unavailable: shape({ reason: text }),
});
const mouseOptions = shape({ button: choice(["left", "middle", "right"]), alt: bool, ctrl: bool, shift: bool });
const mousePosition = shape({ x: integer, y: integer });
const mouseButton = shape({ x: integer, y: integer, options: mouseOptions });
const input = shape({
  arguments: tagged({
    write: shape({ data: text }), submit: shape({ data: text }),
    key: shape({ keys: list(text), action: choice(["press", "down", "repeat", "up"]) }),
