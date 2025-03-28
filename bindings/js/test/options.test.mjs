import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ExpectationError,
  NoSessionError,
  Termprobe,
  UsageError,
  uniqueSession,
} from "../dist/index.js";
import {
  artifactPayload,
  backendPayload,
  envPairs,
  profilePayload,
  recordingPayload,
  tracePayload,
  resolveTimeout,
  timeoutsPayload,
} from "../dist/config.js";
import { NativeRuntime, mapNativeError } from "../dist/native.js";
import { resolve } from "node:path";

const shell = process.platform === "win32" ? "pwsh" : undefined;
const evalArgs =
  typeof globalThis.Deno === "undefined"
    ? ["-e", "console.log('ready'); setInterval(() => {}, 1000)"]
    : ["eval", "console.log('ready'); setInterval(() => {}, 1000)"];

const ALL_TIMEOUT_ENV_VARS = [
  "TERMPROBE__TIMEOUT_MS",
  "TERMPROBE__EXPECT_TIMEOUT_MS",
  "TERMPROBE__TIMEOUT_TEXT_MS",
  "TERMPROBE__TIMEOUT_IDLE_MS",
  "TERMPROBE__TIMEOUT_COMMAND_MS",
  "TERMPROBE__TIMEOUT_EXIT_MS",
  "TERMPROBE__TIMEOUT_READY_MS",
];

const CLASSES = ["text", "idle", "command", "exit", "ready"];

test("invalid capture backgrounds reject with UsageError before session lookup", async () => {
  const terminal = new Termprobe(uniqueSession("invalid-capture-background"));
  for (const background of [
    "", "#12", "#ff00zz", "#12345678", "#12é34", "256,0,0", "rgb(-1,0,0)",
  ]) {
    for (const capture of [
      () => terminal.screenshot("screen.svg", { background }),
      () => terminal.startRecording("recording.gif", { background }),
    ]) {
      await assert.rejects(
        capture,
        (error) => error instanceof UsageError && error.message.includes("color"),
        JSON.stringify(background),
      );
    }
  }
});

function withEnv(vars, fn) {
  const saved = {};
  for (const key of Object.keys(vars)) {
    saved[key] = process.env[key];
    if (vars[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = vars[key];
    }
  }
  try {
    return fn();
  } finally {
    for (const key of Object.keys(vars)) {
      if (saved[key] === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = saved[key];
      }
    }
  }
}

test("resolveTimeout returns undefined when nothing is configured", () => {
  for (const cls of CLASSES) {
    assert.equal(resolveTimeout(cls), undefined);
    assert.equal(resolveTimeout(cls, undefined, { timeouts: {} }), undefined);
  }
});

test("resolveTimeout precedence is per-call then client class", () => {
  assert.equal(resolveTimeout("text", 111, { timeouts: { text: 333 } }), 111);
  assert.equal(resolveTimeout("text", undefined, { timeouts: { text: 333 } }), 333);
  assert.equal(resolveTimeout("text", undefined, { timeouts: { idle: 333 } }), undefined);
  assert.equal(resolveTimeout("text", 0), 0);
});

test("resolveTimeout leaves environment fallback to the engine", () => {
  const vars = Object.fromEntries(ALL_TIMEOUT_ENV_VARS.map((name) => [name, "1234"]));
  withEnv(vars, () => {
    for (const cls of CLASSES) {
      assert.equal(resolveTimeout(cls), undefined);
    }
  });
});

test("timeoutsPayload omits empty values and keeps known classes", () => {
  assert.equal(timeoutsPayload(undefined), undefined);
  assert.equal(timeoutsPayload({}), undefined);
  assert.deepEqual(timeoutsPayload({ text: 1000, command: 2000 }), {
    text: 1000,
    command: 2000,
  });
  assert.deepEqual(timeoutsPayload({ ready: 45000 }), { ready: 45000 });
});

test("envPairs coerces records and preserves pair arrays", () => {
  assert.deepEqual(envPairs({ A: "1", B: 2, C: true, D: false }), [
    ["A", "1"],
    ["B", "2"],
    ["C", "true"],
    ["D", "false"],
  ]);
  assert.deepEqual(envPairs([["X", "Y"]]), [["X", "Y"]]);
  assert.deepEqual(envPairs(), []);
});

test("backendPayload validates backend names", () => {
  assert.equal(backendPayload(), undefined);
  assert.equal(backendPayload("alacritty"), "alacritty");
  assert.equal(backendPayload("ghostty"), "ghostty");
  assert.equal(backendPayload("rio"), "rio");
  assert.throws(() => backendPayload("xterm"), /unknown backend/);
  assert.throws(() => backendPayload("libghostty"), /unknown backend/);
});

test("profilePayload validates profile and color fields", () => {
  assert.deepEqual(profilePayload({ scrollback: 50, colors: { red: "#010203" } }), {
    scrollback: 50,
    colors: [["red", "#010203"]],
  });

  assert.throws(() => profilePayload({ scrollbacks: 50 }), /scrollbacks/);
  assert.throws(
    () => profilePayload({ colors: { chartreuse: "#010203" } }),
    /chartreuse/,
  );
  assert.throws(() => profilePayload({ colors: { red: 123 } }), /must be a string/);
});

test("mouse helpers encode named buttons and modifiers", async () => {
  const calls = [];
  const originals = {
    clickLocator: NativeRuntime.prototype.clickLocator,
    mouseClick: NativeRuntime.prototype.mouseClick,
    mouseDown: NativeRuntime.prototype.mouseDown,
    mouseUp: NativeRuntime.prototype.mouseUp,
    mouseDrag: NativeRuntime.prototype.mouseDrag,
  };
  NativeRuntime.prototype.clickLocator = async (stages, button, clicks, timeout) => {
    calls.push(["locator", stages, button, clicks, timeout]);
  };
  NativeRuntime.prototype.mouseClick = async (options) => {
    calls.push(["click", options]);
  };
  NativeRuntime.prototype.mouseDown = async (x, y, button) => {
    calls.push(["down", x, y, button]);
  };
  NativeRuntime.prototype.mouseUp = async (x, y, button) => {
    calls.push(["up", x, y, button]);
  };
  NativeRuntime.prototype.mouseDrag = async (x1, y1, x2, y2, button) => {
    calls.push(["drag", x1, y1, x2, y2, button]);
  };

  try {
    const su = new Termprobe("mouse-options");
    await su.mouse.click(null, null, {
      onText: "OK",
      button: "right",
      alt: true,
      ctrl: true,
      shift: true,
      clicks: 2,
    });
    await su.mouse.down(1, 2, { button: "middle", ctrl: true });
    await su.mouse.up(3, 4, { button: "right", alt: true });
    await su.mouse.drag(5, 6, 7, 8, { shift: true });

    assert.deepEqual(calls, [
      [
        "click",
        {
          x: undefined,
          y: undefined,
          onText: "OK",
          button: 30,
          clicks: 2,
        },
      ],
      ["down", 1, 2, 17],
      ["up", 3, 4, 10],
      ["drag", 5, 6, 7, 8, 4],
    ]);

    await su.getByText("Open").unique().click({
      button: "middle",
      alt: true,
      ctrl: true,
      shift: true,
      clicks: 2,
      timeout: 50,
    });
    const locatorCall = calls.at(-1);
    assert.equal(locatorCall[0], "locator");
