import asyncio
import inspect
import json
import unittest
from pathlib import Path
from typing import get_type_hints

from tui_test import Cursor, Locator, NoSessionError, State, TuiTest, UsageError, _native, unique_session


INVALID_CAPTURE_BACKGROUNDS = (
    "", "#12", "#ff00zz", "#12345678", "#12é34", "256,0,0", "rgb(-1,0,0)",
)


class CaptureBackgroundTests(unittest.TestCase):
    def test_invalid_background_errors_come_from_native_awaitables(self):
        async def scenario():
            session = _native.NativeSession(unique_session("invalid-native-background"))
            for background in INVALID_CAPTURE_BACKGROUNDS:
                for capture in (
                    lambda: session.screenshot("screen.svg", False, background=background),
                    lambda: session.start_recording(
                        "recording.gif", None, None, None, None, background=background
                    ),
                ):
                    with self.subTest(background=background):
                        awaitable = capture()
                        self.assertTrue(inspect.isawaitable(awaitable))
                        with self.assertRaisesRegex(_native.NativeUsageError, "color"):
                            await awaitable

        asyncio.run(scenario())

    def test_invalid_backgrounds_raise_public_usage_errors(self):
        async def scenario():
            terminal = TuiTest(unique_session("invalid-background"))
            for background in INVALID_CAPTURE_BACKGROUNDS:
                for capture in (
                    lambda: terminal.screenshot("screen.svg", background=background),
                    lambda: terminal.start_recording("recording.gif", background=background),
                ):
                    with self.subTest(background=background):
                        with self.assertRaisesRegex(UsageError, "color"):
                            await capture()

        asyncio.run(scenario())


class _IndexValue:
    def __init__(self, value):
        self.value = value

    def __index__(self):
        return self.value


class NativeSurfaceTests(unittest.TestCase):
    def test_public_client_omits_one_shot_text_methods(self):
        for name in ("find_text", "wait_text", "expect_text"):
            self.assertFalse(hasattr(TuiTest, name), name)
        for method in (
            TuiTest.get_by_text,
            TuiTest.get_by_style,
            Locator.get_by_text,
            Locator.get_by_style,
        ):
            self.assertNotIn("occurrence", inspect.signature(method).parameters)
        self.assertNotIn(
            "style", inspect.signature(Locator.expect).parameters
        )

    def test_native_session_has_only_typed_terminal_methods(self):
        session = _native.NativeSession(unique_session("surface"))
        self.assertFalse(hasattr(session, "request"))
        for name in (
            "open",
            "run",
            "restart",
            "close",
            "state",
            "text",
            "find_locator",
            "wait_locator",
            "click_locator",
            "highlight_locator",
            "expect_locator",
            "packed_screen",
            "cells",
            "get_command",
            "get_output",
            "get_exit_code",
            "get_cwd",
            "get_cursor",
            "get_size",
            "get_clipboard",
            "get_bell_count",
            "get_bell_events",
            "write",
            "type",
            "submit",
            "press",
            "key_down",
            "repeat",
            "key_up",
            "mouse_click",
            "mouse_move",
            "mouse_down",
            "mouse_up",
            "mouse_drag",
            "mouse_scroll",
            "resize",
            "signal",
            "kill",
            "wait_clipboard",
            "wait_idle",
