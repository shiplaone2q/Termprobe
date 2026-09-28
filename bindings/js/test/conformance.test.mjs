import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { test } from "node:test";

import * as sdk from "../dist/index.js";
import { Termprobe } from "../dist/index.js";

const BIN = process.env.TERMPROBE__BIN || "termprobe";

function agentContext() {
  const out = execFileSync(BIN, ["agent-context"], { encoding: "utf8" });
  return JSON.parse(out);
}

let schema;
try {
  schema = agentContext();
} catch {
  schema = null;
}

const MAPPING = {
  open: [["client", "open"]],
  run: [["client", "run"]],
  restart: [["client", "restart"]],
  close: [["client", "close"], ["module", "closeAll"]],
  sessions: [["module", "sessions"]],
  state: [["client", "state"]],
  text: [["client", "text"]],
  screenshot: [["client", "screenshot"]],
  record: [["client", "startRecording"], ["client", "stopRecording"]],
  cells: [["client", "cells"]],
  get: [
    ["client", "getCommand"],
    ["client", "getOutput"],
    ["client", "getExitCode"],
    ["client", "getCwd"],
    ["client", "getCursor"],
    ["client", "getSize"],
    ["client", "getViewport"],
    ["client", "getTitle"],
    ["client", "getClipboard"],
    ["client", "getBellCount"],
    ["client", "getBellEvents"],
  ],
  type: [["client", "type"]],
  submit: [["client", "submit"]],
  key: [["client", "keyboard"]],
  press: [["client", "press"]],
  mouse: [["client", "mouse"]],
  resize: [["client", "resize"]],
  scroll: [["client", "scroll"], ["client", "scrollToTop"], ["client", "scrollToBottom"]],
  write: [["client", "write"]],
  signal: [["client", "signal"]],
  kill: [["client", "kill"]],
  wait: [["client", "waitTitle"], ["client", "waitClipboard"], ["client", "waitIdle"], ["client", "waitCommand"], ["client", "waitExit"], ["client", "waitBell"]],
  expect: [["client", "getByText"], ["client", "expectTitle"], ["client", "expectExitCode"], ["client", "expectOutput"], ["client", "expectBellCount"], ["client", "expectSnapshot"]],
  find: [["client", "getByText"]],
  click: [["client", "getByText"]],
  highlight: [["client", "getByText"]],
  "get-recording": [["module", "getRecording"]],
};

const EXCLUDED = new Set(["monitor", "status", "daemon", "usage", "agent-context", "skill"]);

test("every cli command is mapped or excluded", { skip: !schema }, () => {
  const instance = new Termprobe("conformance");
  for (const command of Object.keys(schema.commands)) {
    if (EXCLUDED.has(command)) {
