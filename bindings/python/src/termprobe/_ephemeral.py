from __future__ import annotations

import os
import re
import secrets
import threading
from typing import Optional

_session_counter = 0
_session_counter_lock = threading.Lock()


def unique_session(prefix: Optional[str] = None) -> str:
    global _session_counter
