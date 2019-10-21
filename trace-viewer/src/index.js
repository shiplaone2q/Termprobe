import "../style.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { parseReport } from "./model.js";
import { createResources } from "./resources.js";

/** @param {string} message */
function showError(message) {
  const error = document.getElementById("report-error");
  if (!error) throw new Error(message);
