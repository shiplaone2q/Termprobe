/** @import { FailureLocatorQuery, FailureMatchOccurrence, FailureTextAnchor, FailureTextStyle, OperationExpectation } from "./report.js" */
/** @import { Operation } from "./types.js" */
/** @type {readonly (keyof FailureTextStyle)[]} */
const styleKeys = ["foreground", "background", "bold", "dim", "italic", "underline_style", "underline_color", "inverse", "hidden", "strikethrough", "blink"];
const ansiNames = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "bright black", "bright red", "bright green", "bright yellow", "bright blue", "bright magenta", "bright cyan", "bright white"];
/** @param {FailureTextStyle} style */
function styleDescription(style) {
  return styleKeys.flatMap((key) => {
    const value = style[key];
    if (value == null) return [];
    if (typeof value === "string" && ["foreground", "background", "underline_color"].includes(key) && /^\d+$/.test(value)) {
      const index = Number(value);
      if (index <= 255) return [`${key.replaceAll("_", " ")}=ANSI ${index}${ansiNames[index] ? ` (${ansiNames[index]} slot)` : ""}`];
    }
    return [`${key.replaceAll("_", " ")}=${JSON.stringify(value)}`];
  }).join(", ");
}
/** @param {FailureMatchOccurrence} value */
function occurrence(value) {
  return typeof value === "object" ? `nth(${value.nth})` : value;
}
/** @param {FailureTextAnchor} value */
function anchor(value) {
  return `${value.regex ? "regex " : ""}${JSON.stringify(value.text)} (${occurrence(value.occurrence)})`;
}
/** @param {FailureLocatorQuery} query @returns {string} */
export function describeLocator(query) {
  const { selector } = query;
  let description;
  if (selector.kind === "text") {
    const text = selector.selector;
    description = `${text.regex ? "regex" : "text"} ${JSON.stringify(text.text)}`;
    if (text.whitespace === "normalize") description += " (normalized whitespace)";
    if (text.scope.after) description += ` after ${anchor(text.scope.after)}`;
    if (text.scope.before) description += ` before ${anchor(text.scope.before)}`;
  } else if (selector.kind === "style") {
    description = `cells matching ${styleDescription(selector.selector.style) || "any style"}`;
  } else if (selector.kind === "link") {
    description = selector.selector.uri === "" ? "cells with no hyperlink" : `cells linking exactly to ${JSON.stringify(selector.selector.uri)}`;
  } else if (selector.kind === "and" || selector.kind === "or") {
    const operator = selector.kind === "and" ? "cell intersection" : "cell union";
    description = `${operator} [${describeLocator(selector.selector.left)}] [${describeLocator(selector.selector.right)}]`;
  } else if (selector.kind === "filter") {
    description = `${describeLocator(selector.selector.input)} filtered by`;
    if (selector.selector.has) description += ` has [${describeLocator(selector.selector.has)}]`;
    if (selector.selector.has_not) description += ` has not [${describeLocator(selector.selector.has_not)}]`;
  } else {
    throw new Error("Unknown locator expression");
  }
  if ("full" in selector.selector && selector.selector.full) description += " in full scrollback";
  const style = styleDescription(query.style);
  if (style) description += ` with ${style}`;
  if (query.occurrence !== "any") description += ` (${occurrence(query.occurrence)})`;
  if (!query.within) return description;
  if (query.direction === "within" && (selector.kind === "style" || selector.kind === "link")) {
    return `[${describeLocator(query.within)}] whose whole match satisfies ${description}`;
  }
  return `${description} ${query.direction} [${describeLocator(query.within)}]`;
}
/** @param {OperationExpectation | undefined} value */
export function describeExpectation(value) {
  if (!value) return undefined;
  if (value.kind === "unavailable") return `Expectation not retained: ${value.reason}`;
  if (value.kind === "value") return `${value.subject}: ${value.expected}`;
  if (value.outcome === "matches") return `Resolve ${describeLocator(value.query)} (zero or more matches)`;
