"""Builds the package with Flow's C library inside it, where there is one.

pyproject.toml says everything else. A wheel built with nothing but the
Python (the library found at run time by FLOW_SDK_LIB or the system's
paths) is pure, for any platform, as before. Copy the library built for a
platform into flow_sdk/ first (libflow_sdk_c.dylib, libflow_sdk_c.so or
flow_sdk_c.dll, from `cargo build --release -p flow-sdk-c`) and the wheel
carries it and is marked for that platform alone: py3-none-<platform>, since
ctypes asks nothing of the Python it runs in.
"""

from pathlib import Path

from setuptools import setup

try:
    from setuptools.command.bdist_wheel import bdist_wheel
except ImportError:  # setuptools before 70.1 had it from the wheel package
    from wheel.bdist_wheel import bdist_wheel

#: The library's names on macOS, Linux and Windows.
LIBRARIES = ("libflow_sdk_c.dylib", "libflow_sdk_c.so", "flow_sdk_c.dll")
BUNDLED = [name for name in LIBRARIES if (Path(__file__).resolve().parent / "flow_sdk" / name).is_file()]


class Wheel(bdist_wheel):
    """A platform's wheel when the library goes in it, else a pure one."""

    def finalize_options(self):
        super().finalize_options()
        if BUNDLED:
            self.root_is_pure = False

    def get_tag(self):
        python, abi, platform = super().get_tag()
        if BUNDLED:
            return "py3", "none", platform
        return python, abi, platform


setup(cmdclass={"bdist_wheel": Wheel})
