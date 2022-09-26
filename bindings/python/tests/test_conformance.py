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
