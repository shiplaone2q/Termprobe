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

