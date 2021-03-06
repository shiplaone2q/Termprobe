"""Build the ordinary shared library with setuptools-rust, without Python linkage."""

import os
from pathlib import Path
import re
import shutil

from setuptools import setup
from setuptools.command.sdist import sdist
from setuptools_rust import Binding, RustExtension
from setuptools_rust.build import build_rust
from wheel.bdist_wheel import bdist_wheel


ROOT = Path(__file__).resolve().parent
RUST_ROOT = ROOT / "_rust" if (ROOT / "_rust").is_dir() else ROOT.parent.parent


class SharedLibraryBuild(build_rust):
    def get_dylib_ext_path(self, ext, target_fname):
        extension_path = Path(super().get_dylib_ext_path(ext, target_fname))
        platform = self.plat_name.lower()
        filename = (
            "tui_test_ffi.dll" if platform.startswith("win")
            else "libtui_test_ffi.dylib" if platform.startswith("macosx")
