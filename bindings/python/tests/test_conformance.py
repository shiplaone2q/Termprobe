import json
import os
import subprocess
import unittest

import tui_test
from tui_test import TuiTest

BIN = os.environ.get("TUI_TEST_BIN") or "tui-test"

MAPPING = {
    "open": [("client", "open")],
    "run": [("client", "run")],
    "restart": [("client", "restart")],
    "close": [("client", "close"), ("module", "close_all")],
    "sessions": [("module", "sessions")],
    "state": [("client", "state")],
    "text": [("client", "text")],
    "screenshot": [("client", "screenshot")],
    "record": [("client", "start_recording"), ("client", "stop_recording")],
    "cells": [("client", "cells")],
    "get": [
        ("client", "get_command"),
        ("client", "get_output"),
        ("client", "get_exit_code"),
        ("client", "get_cwd"),
        ("client", "get_cursor"),
        ("client", "get_size"),
        ("client", "get_viewport"),
        ("client", "get_title"),
        ("client", "get_clipboard"),
        ("client", "get_bell_count"),
        ("client", "get_bell_events"),
    ],
    "type": [("client", "type")],
    "submit": [("client", "submit")],
    "key": [("client", "keyboard")],
    "press": [("client", "press")],
    "mouse": [("client", "mouse")],
    "resize": [("client", "resize")],
    "scroll": [("client", "scroll"), ("client", "scroll_to_top"), ("client", "scroll_to_bottom")],
    "write": [("client", "write")],
    "signal": [("client", "signal")],
    "kill": [("client", "kill")],
    "wait": [("client", "wait_title"), ("client", "wait_clipboard"), ("client", "wait_idle"), ("client", "wait_command"), ("client", "wait_exit"), ("client", "wait_bell")],
    "expect": [("client", "get_by_text"), ("client", "expect_title"), ("client", "expect_exit_code"), ("client", "expect_output"), ("client", "expect_bell_count"), ("client", "expect_snapshot")],
    "find": [("client", "get_by_text")],
    "click": [("client", "get_by_text")],
    "highlight": [("client", "get_by_text")],
    "get-recording": [("module", "get_recording")],
}

EXCLUDED = {"monitor", "usage", "agent-context", "skill", "daemon"}


def _have_binary():
    try:
        subprocess.run([BIN, "agent-context"], capture_output=True, check=True)
        return True
