import asyncio
import json
import os
import re
import unittest
from unittest import mock

import tui_test
from tui_test import (
    FailureArtifactRef,
    FailureArtifactStatus,
    FailureDetails,
    FailureReason,
    _config as cfg,
)
from tui_test import _ephemeral as ephemeral
from tui_test import client
from tui_test.errors import (
    ExpectationError,
    InternalError,
    NoSessionError,
    UsageError,
)
from tui_test.types import AutomaticRecording, Colors, Profile, TextStyle, Timeouts


def run(coro):
    return asyncio.run(coro)


class _FakeNative:
    def __init__(self):
        self.calls = []
        self.reply = {}
        self.error = None

    def __getattr__(self, name):
        def invoke(*args):
            self.calls.append((name, args))

            async def complete():
                if self.error is not None:
                    raise self.error
                return self.reply

            return complete()

        return invoke


class _CapturingClient(client.TuiTest):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fake = _FakeNative()
        self._native = self.fake


class ClipboardPatternTests(unittest.TestCase):
    def test_compiled_patterns_keep_python_matching_semantics(self):
        cases = (
            (re.compile(r"(?<=prefix-)ready(?=-42)"), "prefix-ready-42", True),
            (re.compile(r"^(\w+)-\1$"), "echo-echo", True),
            (re.compile(r"^ready\Z"), "ready", True),
            (re.compile(r"ready$"), "ready\n", True),
            (re.compile(r"^i$", re.IGNORECASE), "\u0130", True),
            (re.compile(r"^\w$"), "\u00b2", True),
            (re.compile(r"^\w+$"), "e\u0301", False),
            (re.compile(r"^\w+$", re.ASCII), "\u00e9", False),
            (re.compile(r"(?a:^\w+$)"), "ready_42", True),
            (re.compile(r"^ ready - [0-9]+ $", re.VERBOSE), "ready-42", True),
            (re.compile(r"(?x: ^ ready - [0-9]+ $ )"), "ready-42", True),
            (re.compile(r"[(?x:]"), "?", True),
            (
                re.compile(r"^ready.*end$", re.IGNORECASE | re.MULTILINE | re.DOTALL),
                "before\nREADY\nend\nafter",
                True,
            ),
        )
        for pattern, value, matches in cases:
            with self.subTest(pattern=pattern, value=value):
                terminal = _CapturingClient("s")
                terminal.fake.get_clipboard = mock.AsyncMock(return_value=value)
                terminal.fake.wait_clipboard = mock.AsyncMock(
                    side_effect=ExpectationError("clipboard did not change")
                )
                if matches:
                    run(terminal.wait_clipboard(pattern, timeout=0))
                    terminal.fake.wait_clipboard.assert_not_called()
                else:
                    with self.assertRaisesRegex(ExpectationError, "^wait_clipboard:"):
                        run(terminal.wait_clipboard(pattern, timeout=0))
                    terminal.fake.wait_clipboard.assert_awaited_once_with(None, False, 0)
                terminal.fake.get_clipboard.assert_awaited_once_with()

    def test_plain_strings_and_change_waits_still_use_native_matching(self):
        terminal = _CapturingClient("s")
        run(terminal.wait_clipboard(timeout=123))
        run(terminal.wait_clipboard("ready", timeout=456))
        self.assertEqual(terminal.fake.calls, [
            ("wait_clipboard", (None, False, 123)),
            ("wait_clipboard", ("ready", False, 456)),
        ])

    def test_invalid_patterns_are_rejected_before_native_dispatch(self):
        for pattern in (re.compile(b"ready"), b"ready", 42):
            with self.subTest(pattern=pattern):
                terminal = _CapturingClient("s")
                with self.assertRaisesRegex(TypeError, "compiled text regex"):
                    run(terminal.wait_clipboard(pattern))
                self.assertEqual(terminal.fake.calls, [])

    def test_regex_waits_share_one_deadline_across_clipboard_changes(self):
        terminal = _CapturingClient("s")
        terminal.fake.state = mock.AsyncMock(return_value={"timeouts": {"text": 100}})
        terminal.fake.get_clipboard = mock.AsyncMock(
            side_effect=["pending", "still pending", "ready"]
        )
        terminal.fake.wait_clipboard = mock.AsyncMock()
        with mock.patch.object(
            client.time, "monotonic_ns",
            side_effect=[1_000_000_000, 1_020_000_000, 1_075_000_000],
        ):
            run(terminal.wait_clipboard(re.compile("ready")))
        terminal.fake.state.assert_awaited_once_with()
        self.assertEqual(terminal.fake.wait_clipboard.await_args_list, [
            mock.call(None, False, 80),
            mock.call(None, False, 25),
        ])

    def test_regex_waits_honor_client_and_per_call_timeouts(self):
        for timeout, expected in ((None, 123), (0, 0), (456, 456), (2**63, 2**63)):
            with self.subTest(timeout=timeout):
                terminal = _CapturingClient("s", timeouts=Timeouts(text=123))
                terminal.fake.get_clipboard = mock.AsyncMock(
                    side_effect=["pending", "ready"]
                )
                terminal.fake.wait_clipboard = mock.AsyncMock()
                with mock.patch.object(client.time, "monotonic_ns", return_value=0):
                    run(terminal.wait_clipboard(re.compile("ready"), timeout=timeout))
                terminal.fake.wait_clipboard.assert_awaited_once_with(None, False, expected)
                self.assertEqual(terminal.fake.calls, [])

    def test_regex_waits_validate_timeouts_even_when_the_clipboard_would_match(self):
        for timeout in (-1, True, 1.5, "123", 2**64):
            with self.subTest(timeout=timeout):
                terminal = _CapturingClient("s")
                terminal.fake.get_clipboard = mock.AsyncMock(return_value="ready")
                with self.assertRaisesRegex(UsageError, "timeout_ms"):
                    run(terminal.wait_clipboard(re.compile("ready"), timeout=timeout))
                terminal.fake.get_clipboard.assert_not_called()
                self.assertEqual(terminal.fake.calls, [])

    def test_regex_wait_preserves_native_errors(self):
        for error in (
            NoSessionError("missing session"),
            InternalError("clipboard unavailable"),
        ):
            with self.subTest(error=error):
                terminal = _CapturingClient("s")
                terminal.fake.error = error
                with self.assertRaises(type(error)) as raised:
                    run(terminal.wait_clipboard(re.compile("ready"), timeout=0))
                self.assertIs(raised.exception, error)


class TimeoutResolutionTests(unittest.TestCase):
    def test_returns_none_when_nothing_configured(self):
        for class_name in ("text", "idle", "command", "exit", "ready"):
            self.assertIsNone(cfg.resolve_timeout(class_name))

    def test_per_call_beats_timeouts_field(self):
        self.assertEqual(
            cfg.resolve_timeout("command", call=111, timeouts={"command": 222}),
            111,
        )

    def test_timeouts_field_used_when_no_call(self):
        self.assertEqual(
            cfg.resolve_timeout("command", timeouts={"command": 222}), 222
        )

    def test_none_entry_in_timeouts_falls_through(self):
        self.assertIsNone(cfg.resolve_timeout("text", timeouts={"text": None}))

    def test_normalize_timeouts_accepts_dataclass_and_mapping(self):
        self.assertIsNone(cfg.normalize_timeouts(None))
        self.assertEqual(cfg.normalize_timeouts({"command": 42})["command"], 42)
        normalized = cfg.normalize_timeouts(Timeouts(command=42))
        self.assertEqual(normalized["command"], 42)
        self.assertIsNone(normalized["idle"])

    def test_session_timeouts_payload_omits_when_empty(self):
        self.assertIsNone(cfg.session_timeouts_payload(None))
        self.assertIsNone(cfg.session_timeouts_payload({}))
        self.assertIsNone(cfg.session_timeouts_payload(Timeouts()))

    def test_session_timeouts_payload_keeps_only_set_classes(self):
        self.assertEqual(
            cfg.session_timeouts_payload(
                Timeouts(command=2000, ready=3000)
            ),
            {"command": 2000, "ready": 3000},
        )


class ProfileResolutionTests(unittest.TestCase):
    def test_normalize_accepts_dataclass_and_mapping(self):
        self.assertIsNone(cfg.normalize_profile(None))
        self.assertEqual(
            cfg.normalize_profile(
                Profile(scrollback=50, colors=Colors(red="#010203"))
            ),
            {"scrollback": 50, "colors": {"red": "#010203"}},
        )
        self.assertEqual(
            cfg.normalize_profile({"colors": {"bright_blue": "#abc"}}),
            {"colors": {"bright_blue": "#abc"}},
        )

    def test_unknown_profile_fields_are_rejected(self):
        with self.assertRaises(ValueError):
            cfg.normalize_profile({"scrollbacks": 10})
        with self.assertRaises(ValueError):
            cfg.normalize_profile({"colors": {"chartreuse": "#123456"}})


class RecordingResolutionTests(unittest.TestCase):
    def test_accepts_only_directory(self):
        self.assertEqual(
            cfg.normalize_recording(
                AutomaticRecording(directory="casts")
            ),
            {"directory": "casts"},
        )
        with self.assertRaises(ValueError):
            cfg.normalize_recording({"mode": "sometimes"})
        with self.assertRaises(TypeError):
            cfg.normalize_recording({"directory": ""})
        with self.assertRaises(ValueError):
            cfg.normalize_recording({"other": 1})

    def test_trace_modes_are_separate_from_recording(self):
        for mode in ("on", "off", "on-failure"):
            self.assertEqual(cfg.normalize_trace({"mode": mode, "directory": "traces"}), {"mode": mode, "directory": "traces"})
        with self.assertRaises(ValueError):
            cfg.normalize_trace({"mode": "always"})
        with self.assertRaises(TypeError):
            cfg.normalize_trace({"directory": ""})


class FailureDiagnosticsTests(unittest.TestCase):
    def test_native_envelope_is_decoded_and_preserves_cause(self):
        native_error = client.native.NativeAssertionError("native message")
        native_error._tui_test_error_json = json.dumps(
            {
                "kind": "assertion",
                "message": "structured message",
                "details": {
                    "schema_version": 1,
                    "operation": "locator.location",
                    "reason": "locator_no_match",
                    "summary": "missing",
                    "truncated": False,
                    "unknown_additive_field": True,
                },
                "artifact": {
                    "status": "partial",
                    "directory": "artifacts/failure",
                    "report": "artifacts/failure/failure.md",
                    "report_html": "artifacts/failure/failure.html",
                    "timeline": "artifacts/failure/timeline.json",
                    "screen_svg": "artifacts/failure/current.svg",
                    "errors": ["recording omitted"],
                    "unknown_additive_field": True,
                },
            }
