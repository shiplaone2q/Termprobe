import asyncio
import ctypes as C
import json
import threading
import unittest
from unittest.mock import Mock, patch

from tui_test import _abi as abi
from tui_test import _native as native


class OwnershipTests(unittest.TestCase):
    def test_null_and_empty_strings_remain_distinct(self):
        missing, empty = native._string(None), native._string("")
        self.assertFalse(missing.data)
        self.assertTrue(empty.data)
        self.assertEqual(empty.len, 0)
        self.assertIsNone(native._text(missing))
        self.assertEqual(native._text(empty), "")

    def test_result_is_freed_after_success_error_and_conversion_failure(self):
        for code, text, exception in (
            (0, "copied", None),
            (2, "bad argument", native.NativeUsageError),
            (0, "copied", IndexError),
        ):
            with self.subTest(code=code, exception=exception):
                result = abi.TuiResult()
                result.error_kind = code
                result.error_message = native._string(text)
                result.text = native._string(text)
                result.snapshot = 99
                pointer = C.pointer(result)
                library = Mock()
                library.tui_text.return_value = pointer
                with patch.object(native, "_load", return_value=library):
                    kind = "snapshot" if exception is IndexError else "text"
                    if exception:
                        with self.assertRaises(exception):
                            native._invoke("text", (), kind)
                    else:
                        self.assertEqual(native._invoke("text", (), kind), "copied")
                library.tui_result_free.assert_called_once_with(pointer)

    def test_packed_screen_owns_immutable_copied_bytes(self):
        result = abi.TuiResult()
        result.text = native._string("UTF-8: café\n")
        result.size = abi.TuiSize(80, 24)
        library = Mock()
        library.tui_packed_screen.return_value = C.pointer(result)

        def overwrite(pointer):
            C.memset(pointer.contents.text.data, ord("x"), pointer.contents.text.len)

        library.tui_result_free.side_effect = overwrite
        with patch.object(native, "_load", return_value=library):
            screen, cols, rows = native._invoke("packed_screen", (), "packed")
        self.assertEqual((cols, rows), (80, 24))
        self.assertTrue(screen.readonly)
        self.assertEqual(screen.tobytes(), "UTF-8: café\n".encode())

    def test_error_details_and_artifact_survive_result_release(self):
        result = abi.TuiResult()
        result.error_kind = 1
        result.error_message = native._string("not found")
        result.error_details = native._string('{"reason":"timeout"}')
        result.error_artifact = native._string('{"directory":"artifacts"}')
        library = Mock()
        library.tui_expect_locator.return_value = C.pointer(result)
        with patch.object(native, "_load", return_value=library):
            with self.assertRaises(native.NativeAssertionError) as caught:
                native._invoke("expect_locator", ())
        envelope = json.loads(caught.exception._tui_test_error_json)
        self.assertEqual(envelope["details"], {"reason": "timeout"})
