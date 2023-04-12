import asyncio
import ctypes
import sys
import unittest
from unittest.mock import Mock, patch

from tui_test import NoSessionError, TuiTest, UsageError, Viewport, _native, unique_session
from tui_test import _abi


class ScrollbackTests(unittest.TestCase):
    def test_viewport_counts_survive_result_release(self):
        maximum = ctypes.c_size_t(-1).value
        result = _abi.TuiResult()
        result.viewport = _abi.TuiViewport(maximum - 1, maximum)
        library = Mock()
        library.tui_get_viewport.return_value = ctypes.pointer(result)

        def release(pointer):
            pointer.contents.viewport = _abi.TuiViewport()

        library.tui_result_free.side_effect = release
        with patch.object(_native, "_load", return_value=library):
            viewport = _native._invoke("get_viewport", (), "viewport")
        self.assertEqual(viewport, {"offset": maximum - 1, "history": maximum})
        library.tui_result_free.assert_called_once()
