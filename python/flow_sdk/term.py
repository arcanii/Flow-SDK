"""The terminal: whether there is one and whether it's Flow, raw keys and
the alternate screen, its size, what it can do (asked once, with
:func:`detect`), sending graphics and effects where they go, and the next
key, click or resize.

Escapes written with :func:`write` or :meth:`Terminal.send` keep their
order with ``print``: Python's own buffer is flushed before the library
writes. Outside a terminal these do nothing (``None``, ``False``), so a
program run with its input or output redirected falls back quietly.
"""

import ctypes
import sys
from contextlib import contextmanager
from dataclasses import dataclass
from typing import NamedTuple, Optional, Tuple

from . import _native
from ._native import lib, u32

# What detect() asks: FLOW_ASK_* in flow_sdk.h.
_ASK_PICTURES, _ASK_MODELS, _ASK_CELL, _ASK_AREA, _ASK_VERSION, _ASK_NO_PIPE, _ASK_BACKGROUND = 1, 2, 4, 8, 16, 32, 64

_KINDS = {1: "key", 2: "click", 3: "resize", 4: "other"}
_KEYS = {
    9: "tab",
    13: "enter",
    27: "escape",
    127: "backspace",
    0x110000: "other",
    0x110001: "left",
    0x110002: "right",
    0x110003: "up",
    0x110004: "down",
    0x110005: "home",
    0x110006: "end",
    0x110007: "page_up",
    0x110008: "page_down",
    0x110009: "delete",
    0x11000A: "insert",
    0x11000B: "backtab",
}
_F1 = 0x110101
_MODS = ((1, "shift"), (2, "ctrl"), (4, "alt"))
_BUTTONS = ("left", "right", "middle")


def in_flow() -> bool:
    """Whether the program runs in Flow, by the name Flow gives its shells."""
    return bool(lib().flow_in_flow())


def is_terminal() -> bool:
    """Whether standard input and output are both a terminal."""
    return bool(lib().flow_is_terminal())


def _flush():
    try:
        sys.stdout.flush()
    except (AttributeError, ValueError, OSError):
        pass


@contextmanager
def raw_mode():
    """Raw keys while inside, the screen kept as it is: for asking the
    terminal below the prompt. Gives whether raw mode went on (not outside a
    terminal)."""
    on = lib().flow_raw_mode(True) == 0
    try:
        yield on
    finally:
        if on:
            lib().flow_raw_mode(False)


@contextmanager
def screen(mouse: bool = False):
    """Raw keys on the alternate screen while inside, with the mouse's
    clicks, drags and moves if ``mouse``; the terminal handed back as it was
    on the way out, an exception's too. Gives whether it went on."""
    _flush()
    entered = lib().flow_term_enter(bool(mouse)) == 0
    try:
        yield entered
    finally:
        if entered:
            _flush()
            lib().flow_term_leave()


class Size(NamedTuple):
    """The terminal's size in cells, and a cell's in pixels where it tells."""

    cols: int
    rows: int
    cell: Optional[Tuple[float, float]]


def size() -> Optional[Size]:
    """The terminal's size, or None outside one."""
    out = _native.FlowSize()
    if lib().flow_term_size(ctypes.byref(out)) != 0:
        return None
    cell = (out.cell_width, out.cell_height) if out.cell_width > 0 and out.cell_height > 0 else None
    return Size(out.cols, out.rows, cell)


@dataclass(frozen=True)
class Caps:
    """What the terminal said it can do."""

    pictures: bool = False
    models: bool = False
    flow: bool = False
    #: Picture cells go bare (Windows, through ConPTY): see picture.bare_row.
    bare: bool = False
    #: Pictures go down Flow's pipe (Windows).
    piped: bool = False
    #: A cell's whole pixels (CSI 16 t), width and height.
    cell: Optional[Tuple[int, int]] = None
    #: The text area's pixels (CSI 14 t).
    area: Optional[Tuple[int, int]] = None
    #: XTVERSION's answer: "Flow 0.1.24", say.
    version: Optional[str] = None
    #: The background OSC 11 gave, 0xRRGGBB, through Flow's pipe on
    #: Windows since Flow 0.1.28; None from a terminal that doesn't say.
    background: Optional[int] = None

    @property
    def light_background(self) -> Optional[bool]:
        """Whether the background is light (:func:`is_light`); None if
        the terminal didn't say."""
        return None if self.background is None else is_light(self.background)


def is_light(rgb: int) -> bool:
    """Whether 0xRRGGBB is light: past the middle of the eye's brightness,
    so dark letters suit it better than light ones."""
    return bool(lib().flow_is_light(u32(rgb, "rgb") & 0xFFFFFF))


class Terminal:
    """The terminal as asked (:func:`detect`): what it can do, in
    :attr:`caps`, and where pictures and effects go."""

    def __init__(self, handle):
        if not handle:
            raise MemoryError("Flow's library made no terminal")
        self._handle = handle
        out = _native.FlowCaps()
        lib().flow_terminal_caps(handle, ctypes.byref(out))
        pair = lambda w, h: (w, h) if w and h else None
        background = ctypes.c_uint32()
        said = lib().flow_terminal_background(handle, ctypes.byref(background)) == 0
        self.caps = Caps(
            pictures=out.pictures,
            models=out.models,
            flow=out.flow,
            bare=out.bare,
            piped=out.piped,
            cell=pair(out.cell_width, out.cell_height),
            area=pair(out.area_width, out.area_height),
            version=_native.take(lib().flow_terminal_version(handle)),
            background=background.value if said else None,
        )

    @classmethod
    def plain(cls) -> "Terminal":
        """A terminal that can do nothing, as one that answers nothing."""
        return cls(lib().flow_terminal_plain())

    def _live(self):
        if not self._handle:
            raise ValueError("the terminal was closed")
        return self._handle

    def send(self, commands) -> None:
        """Sends graphics commands (pictures, models): down Flow's pipe on
        Windows, waiting until Flow has them; else to standard output."""
        data = _native.text(commands)
        _flush()
        if lib().flow_terminal_send(self._live(), data, len(data)) != 0:
            raise OSError("the graphics commands couldn't be sent")

    def send_effects(self, escapes) -> None:
        """Sends effects and sounds: down Flow's pipe on Windows, else to
        standard output."""
        data = _native.text(escapes)
        _flush()
        if lib().flow_terminal_send_effects(self._live(), data, len(data)) != 0:
            raise OSError("the effects couldn't be sent")

    def cell_size(self) -> Optional[Tuple[float, float]]:
        """A cell's size in pixels now, or None where the terminal doesn't say."""
        w, h = ctypes.c_float(), ctypes.c_float()
        if lib().flow_terminal_cell_size(self._live(), ctypes.byref(w), ctypes.byref(h)) != 0:
            return None
        return (w.value, h.value)

    def close(self) -> None:
        if self._handle:
            lib().flow_terminal_free(self._handle)
            self._handle = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass


def detect(
    pictures: bool = False,
    models: bool = False,
    cell: bool = False,
    area: bool = False,
    version: bool = False,
    pipe: bool = True,
    wait: float = 1.0,
    background: bool = False,
) -> Terminal:
    """Asks the terminal what it can do, once, waiting ``wait`` seconds at
    most for one that answers nothing, and with ``background`` its
    background color (:attr:`Caps.background`). Ask inside :func:`raw_mode` or
    :func:`screen`, before anything reads the keys. Outside a terminal on
    macOS and Linux it asks nothing and the answer is a plain terminal (Flow
    still known by name)."""
    ask = (
        (_ASK_PICTURES if pictures else 0)
        | (_ASK_MODELS if models else 0)
        | (_ASK_CELL if cell else 0)
        | (_ASK_AREA if area else 0)
        | (_ASK_VERSION if version else 0)
        | (_ASK_BACKGROUND if background else 0)
        | (0 if pipe else _ASK_NO_PIPE)
    )
    wait_ms = max(1, min(int(wait * 1000), 0xFFFFFFFF))
    _flush()
    return Terminal(lib().flow_detect(ask, wait_ms))


def write(text) -> None:
    """Writes an escape (or any text) to standard output now."""
    if isinstance(text, (bytes, bytearray)):
        _flush()
        sys.stdout.buffer.write(bytes(text))
    else:
        sys.stdout.write(text)
    sys.stdout.flush()


@dataclass(frozen=True)
class Event:
    """A key, a click or a resize, as :func:`next_event` reads it."""

    #: "key", "click", "resize" or "other".
    kind: str
    #: A key's character ("q"), or its name: "enter", "tab", "backspace",
    #: "escape", "left", "right", "up", "down", "home", "end", "page_up",
    #: "page_down", "delete", "insert", "backtab", "f1" to "f24", "other".
    key: Optional[str] = None
    #: Held with the key: some of "shift", "ctrl" and "alt".
    mods: frozenset = frozenset()
    #: Where a click went down, from 0; or a resize's new size in cells.
    col: int = 0
    row: int = 0
    #: A click's: "left", "right" or "middle".
    button: Optional[str] = None

    @property
    def is_quit(self) -> bool:
        """q, Escape or Ctrl+C: what quits a program that takes the screen."""
        if self.kind != "key":
            return False
        return self.key in ("q", "escape") or (self.key == "c" and "ctrl" in self.mods)


def _event(raw) -> Event:
    kind = _KINDS.get(raw.kind, "other")
    if kind == "key":
        code = raw.key
        if code in _KEYS:
            key = _KEYS[code]
        elif _F1 <= code < _F1 + 24:
            key = "f%d" % (code - _F1 + 1)
        elif code < 0x110000:
            key = chr(code)
        else:
            key = "other"
        mods = frozenset(name for bit, name in _MODS if raw.mods & bit)
        return Event("key", key=key, mods=mods)
    if kind == "click":
        button = _BUTTONS[raw.button] if raw.button < len(_BUTTONS) else None
        return Event("click", col=raw.col, row=raw.row, button=button)
    return Event(kind, col=raw.col, row=raw.row)


def next_event(timeout: Optional[float] = None) -> Optional[Event]:
    """The next key, click or resize, waiting ``timeout`` seconds at most
    (None for as long as it takes); None when the time ran out or outside
    a terminal."""
    ms = 0xFFFFFFFF if timeout is None else max(0, min(int(timeout * 1000), 0xFFFFFFFF))
    raw = _native.FlowEvent()
    if lib().flow_next_event(ms, ctypes.byref(raw)) != 1:
        return None
    return _event(raw)
