"""Run with an installed wheel or TUI_TEST_NATIVE_LIBRARY."""

import asyncio
import sys

from tui_test import TuiTest


async def main():
    async with TuiTest.ephemeral("shared-library") as terminal:
        await terminal.run(
            sys.executable, "-u", "-c",
            "print('Shared typed ABI: Python'); input()",
        )
        await terminal.get_by_text("Shared typed ABI: Python").expect()
