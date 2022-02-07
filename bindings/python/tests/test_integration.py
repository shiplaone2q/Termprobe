import asyncio
import base64
import gc
import json
import os
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import tui_test
from tui_test import (
    Colors,
    ExpectationError,
    FailureArtifactRef,
    FailureArtifactStatus,
    FailureDetails,
    InternalError,
    NoSessionError,
    Profile,
    TuiTest,
    TextStyle,
    Timeouts,
    UsageError,
    get_recording,
    testing,
    unique_session,
)
from tui_test import _native

SHELL = "pwsh" if sys.platform == "win32" else "bash"
TWO_BELLS_COMMAND = (
    "[Console]::Out.Write([char]7); [Console]::Out.Write([char]7)"
    if sys.platform == "win32"
    else "printf '\\a\\a'"
)
DELAYED_BELL_COMMAND = (
    "Start-Sleep -Seconds 1; [Console]::Out.Write([char]7)"
    if sys.platform == "win32"
    else "sleep 1; printf '\\a'"
)


def clipboard_command(base64):
    if sys.platform == "win32":
        return (
            "[Console]::Out.Write(([char]27).ToString() + "
            f"']52;c;{base64}' + ([char]7).ToString())"
        )
    return f"printf '\\033]52;c;{base64}\\a'"


def run(coro):
    return asyncio.run(coro)


class IntegrationTests(unittest.TestCase):
    def _client(self, **kwargs):
        return TuiTest.ephemeral("pytest", **kwargs)

    def test_echo_roundtrip(self):
        async def scenario():
            async with self._client() as su:
                await su.open(shell=SHELL)
                await su.submit("echo hello-sdk")
                await su.wait_command()
                await su.get_by_text("hello-sdk").first().expect()
                await su.expect_exit_code(0)
                st = await su.state()
                self.assertGreater(st.cols, 0)

        run(scenario())

    def test_invalid_spawn_options_with_retries_preserve_existing_session(self):
        async def scenario():
            script = "import time; print('retry-owner-ready', flush=True); time.sleep(30)"
            async with self._client() as su:
                opened = await su.run(sys.executable, "-c", script)
                await su.get_by_text("retry-owner-ready").wait(timeout=5000)
                for options in ({"cols": -1}, {"timeouts": Timeouts(ready=-1)}):
                    for method, args in (
                        (su.open, ()),
                        (su.run, (sys.executable, "-c", script)),
                    ):
                        with self.subTest(method=method.__name__, options=options):
                            with self.assertRaises(UsageError):
                                await method(*args, **options, retries=2, restart=True)
                            self.assertIn(su.session, await tui_test.sessions())
                            await su.get_by_text("retry-owner-ready").expect(timeout=0)
                            reused = await su.run(sys.executable, "-c", script)
                            self.assertEqual(reused["shell_pid"], opened["shell_pid"])
                for method, args, options in (
                    (su.open, (), {"shell": 123}),
                    (su.run, (123,), {}),
                ):
                    with self.assertRaises(TypeError):
                        await method(*args, **options, retries=2)
                    await su.get_by_text("retry-owner-ready").expect(timeout=0)

        run(scenario())

    def test_recording_api_writes_an_asciicast_file(self):
        async def scenario():
            with tempfile.TemporaryDirectory() as root:
                path = Path(root) / "demo.cast"
                async with self._client() as su:
                    await su.open(shell=SHELL)
                    await su.start_recording(
                        str(path),
                        format="cast",
                        fps=24,
                        speed=1.0,
                        idle_time_limit=2.0,
                    )
                    await su.submit("echo sdk-recording")
                    await su.wait_command()
                    self.assertEqual(await su.stop_recording(), str(path))
                cast = path.read_text(encoding="utf-8")
                self.assertIn('"version":2', cast)
                self.assertIn("sdk-recording", cast)

        run(scenario())

    def test_trace_retention_and_recording_directory(self):
        async def scenario():
            with tempfile.TemporaryDirectory() as root:
                disabled = self._client(
                    recording={"directory": root},
                    trace={"mode": "off", "directory": str(Path(root) / "traces")},
                )
                result = await disabled.open(shell=SHELL, wait_ready=False)
                self.assertEqual(result["recording"], "")
                await disabled.close()

                always = self._client(
                    recording={"directory": root},
                    trace={"mode": "on", "directory": str(Path(root) / "traces")},
                )
                result = await always.open(shell=SHELL, wait_ready=False)
                self.assertTrue(result["recording"].startswith(root))
                self.assertTrue(Path(result["recording"]).is_file())
                await always.close()

        run(scenario())

    def test_restart_preserves_failure_artifact_recording_without_trace_options(self):
        async def scenario():
            with tempfile.TemporaryDirectory() as root:
                su = self._client(artifacts={
                    "dir": root, "on_failure": "text", "include_recording": True,
                })
                try:
                    opened = await su.run(
                        sys.executable, "-c",
                        "import time; print('restart-ready', flush=True); time.sleep(30)",
                    )
                    self.assertTrue(opened["recording"])
                    restarted = await su.restart(graceful_timeout=0)
                    self.assertTrue(restarted["recording"])
                    await su.get_by_text("restart-ready").wait(timeout=5000)
                    with self.assertRaises(ExpectationError) as failure:
                        await su.get_by_text("missing restart marker").expect(timeout=0)
                    recording = failure.exception.artifact.recording
                    self.assertIsNotNone(recording)
                    self.assertIn("restart-ready", Path(recording).read_text(encoding="utf-8"))
                finally:
                    await su.close_quiet()

        run(scenario())

    def test_failed_open_recording_is_readable_before_close(self):
        async def scenario():
            with tempfile.TemporaryDirectory() as root:
                name = unique_session("recording-failed-open")
                su = TuiTest(
                    name,
                    recording={"directory": root},
                    trace={"mode": "on-failure", "directory": str(Path(root) / "traces")},
                )
                with self.assertRaises(ExpectationError):
                    await su.run(
                        sys.executable,
                        "-c",
                        "import time; time.sleep(2)",
                        wait_ready=True,
                        timeouts=Timeouts(ready=50),
                    )
                self.assertIn('"version":2', await get_recording(name))
                await su.close()

        run(scenario())

    def test_recording_api_exports_styled_unicode_to_apng_and_gif(self):
        async def scenario():
            command = (
                'Write-Host "`e[1;3mstyled-é`e[0m"'
                if sys.platform == "win32"
                else "printf '\\033[1;3mstyled-é\\033[0m\\n'"
            )
            with tempfile.TemporaryDirectory() as root:
                for format, extension in (("apng", "png"), ("gif", "gif")):
                    with self.subTest(format=format):
                        path = Path(root) / f"styled.{extension}"
                        async with self._client() as su:
                            await su.open(shell=SHELL, cols=20, rows=4)
                            if format == "apng":
                                screenshot = Path(root) / "zoomed.svg"
                                await su.screenshot(
                                    str(screenshot), zoom=0.5
                                )
                                self.assertIn(
                                    # Follows the default style; update
                                    # deliberately if a default gap moves.
                                    'width="139" height="94" '
                                    'viewBox="0 0 278 188"',
                                    screenshot.read_text(encoding="utf-8"),
                                )
                            await su.start_recording(
                                str(path), format=format, fps=30, zoom=0.5
                            )
                            await su.submit(command)
                            await su.wait_command()
                            self.assertEqual(
                                await su.stop_recording(), str(path)
                            )
                        data = path.read_bytes()
                        if format == "apng":
                            self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
                            self.assertIn(b"acTL", data)
                            self.assertEqual(
                                int.from_bytes(data[16:20], "big"), 278
                            )
                            self.assertEqual(
                                int.from_bytes(data[20:24], "big"), 188
                            )
                        else:
                            self.assertEqual(data[:6], b"GIF89a")
                            self.assertEqual(
                                int.from_bytes(data[6:8], "little"), 278
                            )
                            self.assertEqual(
                                int.from_bytes(data[8:10], "little"), 188
                            )

        run(scenario())

