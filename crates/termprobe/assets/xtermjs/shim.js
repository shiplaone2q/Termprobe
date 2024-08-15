// Host shim + Rust-facing glue for the vendored `@xterm/headless` bundle.
//
// Two jobs:
//
// 1. Stand in for the handful of browser/Node globals the bundle reaches for.
//    `process.title` is the important one: xterm.js branches on
//    `typeof process !== 'undefined' && 'title' in process` to set `isNode`,
//    and every `navigator` access sits on the other side of that branch, so
//    defining it means no user-agent sniffing ever runs.
//
// 2. Make writing synchronous. `Terminal.write()` is async: it queues the
//    chunk and drains it from a `setTimeout` callback, so the grid is still
//    empty when it returns. `Emulator::process` has to have the grid ready
//    when it returns, so timers are collected into a queue that `feed()`
//    drains to empty before it hands control back to Rust. The drain is
//    ordinary FIFO, which is all xterm.js needs: it only ever schedules its
//    own continuation.
//
// `performance.now()` returning a constant is deliberate, not a stub: the
// write loop yields to a fresh timer once it has spent 12ms on a chunk, and a
// frozen clock means it never does, so one `feed()` always consumes the whole
// chunk instead of leaving a tail for the next drain.

globalThis.process = { title: 'termprobe' };
globalThis.performance = { now: function () { return 0; } };
globalThis.console = {
  log: function () {}, warn: function () {}, error: function () {},
  debug: function () {}, info: function () {}, trace: function () {},
};

var __timers = [];
globalThis.setTimeout = function (fn) { return __timers.push(fn); };
globalThis.clearTimeout = function () {};
globalThis.setInterval = function () { return 0; };
globalThis.clearInterval = function () {};
globalThis.queueMicrotask = function (fn) { __timers.push(fn); };

// The bundle is UMD and assigns to `exports`.
globalThis.exports = {};
globalThis.module = { exports: globalThis.exports };

globalThis.__boot = function (cols, rows, scrollback, base) {
  var term = new exports.Terminal({
    cols: cols,
    rows: rows,
    scrollback: scrollback,
    // `getUnderlineStyle`, `getUnderlineColor` and `getNullCell` are proposed
    // API; without this every one of them throws.
    allowProposedApi: true,

    // `CSI 22 t` pushes the window title and `CSI 23 t` pops it, which is how
    // a shell puts back the title it had before running a command. xterm.js
    // implements both but leaves them off by default, and the rest of the
    // window operations stay off: this terminal has no window to resize,
    // move, or report the position of.
    windowOptions: { pushTitle: true, popTitle: true },
  });

  // The headless bundle ships only the Unicode 6 width tables, which call
  // every astral emoji one column wide. alacritty measures them as two, so
  // without this a line containing an emoji puts every following cell in a
  // different column on the two backends, moving what `cells`, the locator,
  // and the SVG renderer report. The Unicode 11 provider restores the pair.
  if (typeof globalThis.__unicode11 === 'function') {
    term.loadAddon(new globalThis.__unicode11());
    term.unicode.activeVersion = '11';
  }

  // SGR 59 (reset underline color) does not clear xterm.js's stored color: it
  // leaves the extended-attribute record holding RGB #ffffff, which reads back
  // through the public getters as an explicit white underline and is
  // indistinguishable there from a real `58;2::255:255:255`. Correcting it at
  // the point the parser sets it keeps the two apart, which reading the cell
  // afterwards cannot: 0 is the same value the record holds for a cell that
  // never named an underline color.
  var __charAttributes = term._core._inputHandler.charAttributes.bind(
    term._core._inputHandler
  );
  term._core._inputHandler.charAttributes = function (params) {
    var result = __charAttributes(params);
    for (var i = 0; i < params.length; i++) {
      if (params.params[i] === 59 && !params.hasSubParams(i)) {
        term._core._inputHandler._curAttrData.extended.underlineColor = 0;
      }
    }
    return result;
  };

  function followLiveViewport() {
    var buffer = term.buffer.active;
    if (buffer.type === 'normal' && buffer.viewportY === buffer.baseY) {
      term.scrollToBottom();
    }
  }

  // History clearing moves the viewport but leaves xterm.js's scroll flag set.
  // Reset it before the parser processes more output in the same write.
  term._core._inputHandler.onScroll(followLiveViewport);
  term.buffer.onBufferChange(followLiveViewport);

  // xterm.js resolves a cell's `urlId` through this service; there is no
  // public getter for a link on `IBufferCell`.
  var oscLinks = term._core._oscLinkService;

  // Replies the terminal wants sent back up the PTY (DA, CPR, and friends).
  var replies = [];
  term.onData(function (d) { replies.push(d); });

  var title = null;
  term.onTitleChange(function (t) { title = t ? t : null; });

  var bellCount = 0;
  term.onBell(function () { bellCount++; });

  // Colors a program set at runtime, keyed by the same slot numbering the
  // Rust side uses: 0-255 palette, then foreground, background, cursor. Only
  // what a program actually set lives here, so clearing an entry restores
  // whatever the profile configured without having to remember it twice.
  var FG = 256, BG = 257, CURSOR = 258;
  var overrides = {};

  // `base[i]` is the color slot `i` resolves to when no program has changed
  // it: the profile for the sixteen ANSI slots and the three dynamic ones,
  // and the standard xterm table for the rest. It is computed on the Rust
  // side so both backends answer a query with the same value.
  function resolved(slot) {
    var v = overrides[slot];
    return v === undefined ? base[slot] : v;
  }

  // xterm.js hands an OSC handler its payload but not the terminator that
  // ended the sequence, and a reply has to echo the one the query used: a
  // program reading until the terminator it sent would otherwise wait for one
  // that never comes. The bytes are scanned on the way in and each terminator
  // queued for the handler that is about to run.
  //
  // Each entry is the code of an OSC sequence and whether it ended in `BEL`.
  // Keyed by code because the scan sees every sequence while only the color
  // ones are answered here: a title arriving between two queries would
  // otherwise shift the queue out of step and put the wrong terminator on a
  // reply.
  //
  // The scan carries its state between calls because a PTY read splits
  // wherever it likes, including between the two bytes of an `ST`.
  //
  // Only the codes answered here are recorded. The scan sees every sequence,
  // and one whose terminator is never claimed would sit in this list for the
  // life of the session: a shell that retitles the window on every prompt
  // would leak an entry per prompt.
  var ANSWERED = { 4: 1, 10: 1, 11: 1, 12: 1, 104: 1, 110: 1, 111: 1, 112: 1 };
  var terminators = [];
  var inOsc = false, sawEsc = false, code = -1, readingCode = false;
  function scanTerminators(bytes) {
    for (var i = 0; i < bytes.length; i++) {
