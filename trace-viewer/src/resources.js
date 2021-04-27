/** @import { TraceModel, TraceFrame, ImageResult } from "./types.js" */

export const IMAGE_ERROR = "Captured SVG could not be displayed. Terminal text and cell metadata are still available.";

/** Browser-only evidence resources; captured SVG is never inserted into the page DOM.
 * @param {TraceModel} model
 */
export function createResources(model) {
  /** @type {string[]} */
  const urls = [];
  /** @param {Blob} blob */
  function url(blob) {
    const value = URL.createObjectURL(blob);
    urls.push(value);
    return value;
  }
  /** @param {TraceFrame} frame @returns {ImageResult} */
  function image(frame) {
    if (!frame.svg) return {};
    const document = new DOMParser().parseFromString(frame.svg, "image/svg+xml");
    const root = document.documentElement;
    if (document.querySelector("parsererror") || root.localName !== "svg" || root.namespaceURI !== "http://www.w3.org/2000/svg") {
      return { error: IMAGE_ERROR };
    }
