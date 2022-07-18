import asyncio
import unittest
from unittest import mock

from tui_test import InternalError, TuiTest, UsageError, testing
from tui_test.types import Colors, Profile, Timeouts


def run(coro):
    return asyncio.run(coro)


class _FakeTerminal:
    def __init__(self):
        self.closed = 0

    async def close_quiet(self):
        self.closed += 1


class TerminalSnapshotTests(unittest.TestCase):
    def test_trims_trailing_whitespace_per_line(self):
        self.assertEqual(testing.terminal_snapshot("a   \nb\t\n"), "a\nb")

    def test_drops_trailing_blank_lines(self):
        self.assertEqual(testing.terminal_snapshot("a\n\n\n\n"), "a")

    def test_keeps_interior_blank_lines(self):
        self.assertEqual(testing.terminal_snapshot("a\n\nb\n"), "a\n\nb")

    def test_all_blank_collapses_to_empty(self):
        self.assertEqual(testing.terminal_snapshot("\n  \n\n"), "")


class DefaultShellTests(unittest.TestCase):
    def test_matches_the_engine_default(self):
        import sys

        if sys.platform == "win32":
            expected = "powershell"
        elif sys.platform == "darwin":
            expected = "zsh"
        else:
            expected = "bash"
        self.assertEqual(testing.DEFAULT_SHELL, expected)


class RegistryTests(unittest.TestCase):
    def tearDown(self):
        run(testing.close_all_tracked())

    def test_tracking_and_closing(self):
        a, b = _FakeTerminal(), _FakeTerminal()
        testing.track_terminal(a)
        testing.track_terminal(b)
        self.assertEqual(testing.tracked_count(), 2)
        run(testing.close_all_tracked())
        self.assertEqual((a.closed, b.closed), (1, 1))
        self.assertEqual(testing.tracked_count(), 0)

