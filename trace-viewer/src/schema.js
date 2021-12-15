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
