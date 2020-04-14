"""Run with an installed wheel or TUI_TEST_NATIVE_LIBRARY."""

import asyncio
import sys

from tui_test import TuiTest


async def main():
    async with TuiTest.ephemeral("shared-library") as terminal:
