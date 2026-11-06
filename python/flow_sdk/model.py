"""3D models with the Ratty graphics protocol: an object registered by an
id (GLB or OBJ bytes, or a path), placed centered on a cell and fitted to a
block of cells, its :class:`Look` changed, and deleted. Placing an id again
moves it; registering it again draws the new model in its place.
"""

import ctypes
import math
from dataclasses import dataclass
from typing import Optional, Sequence

from . import _native
from ._native import lib, made, u16, u32

#: The scale that undoes the tenth's margin a model is fitted with, so its
#: box is its cells.
FILL = 1.0 / 0.9

_FORMATS = {"obj": 0, "glb": 1}


@dataclass
class Look:
    """How a placed object looks. What's left as None the terminal keeps
    as it was."""

    #: Spins, tumbles and bobs.
    animate: Optional[bool] = None
    scale: Optional[float] = None
    #: Thickness for flat artwork.
    depth: Optional[float] = None
    #: 0xRRGGBB, which tints an OBJ (a glTF has its own colors).
    color: Optional[int] = None
    brightness: Optional[float] = None
    #: Pixels, x right, y up (z nearer or further); any may be None.
    offset: Optional[Sequence[Optional[float]]] = None
    #: Degrees about x, y and z, turned in that order.
    rotation: Optional[Sequence[Optional[float]]] = None
    scale3: Optional[Sequence[Optional[float]]] = None

    def _c(self):
        look = _native.FlowLook()
        lib().flow_look_init(ctypes.byref(look))
        if self.animate is not None:
            look.animate = 1 if self.animate else 0
        if self.color is not None:
            look.color = int(self.color) & 0xFFFFFF
        for name in ("scale", "depth", "brightness"):
            value = getattr(self, name)
            if value is not None:
                setattr(look, name, float(value))
        for name in ("offset", "rotation", "scale3"):
            values = getattr(self, name)
            if values is None:
                continue
            values = list(values)
            if len(values) != 3:
                raise ValueError("%s takes three values (x, y, z), not %d" % (name, len(values)))
            target = getattr(look, name)
            for i, value in enumerate(values):
                target[i] = math.nan if value is None else float(value)
        return look


def _look(look: Optional[Look]):
    return None if look is None else ctypes.byref(look._c())


def _format(format: str) -> int:
    try:
        return _FORMATS[format.lower()]
    except (KeyError, AttributeError):
        raise ValueError("a model is obj or glb, not %r" % (format,)) from None


def register(id: int, data, format: str = "glb", name: Optional[str] = None, chunk: Optional[int] = None) -> str:
    """Registers a model's bytes as object ``id``: in one command, or in
    commands of ``chunk`` bytes at most. ``name`` is the file's name, for
    the terminal's messages."""
    data = bytes(data)
    c_name = None if name is None else name.encode("utf-8")
    pointer = lib().flow_model_register_chunks(u32(id, "id"), _format(format), data, len(data), c_name, int(chunk or 0))
    return made(pointer, "the model (a name holding ';' or a control character?)")


def register_path(id: int, path: str, format: Optional[str] = None) -> str:
    """Registers the file at ``path``, which Flow reads once the user allows
    it; its format from its extension unless given."""
    path = str(path)
    format = format or path.rsplit(".", 1)[-1]
    return made(lib().flow_model_register_path(u32(id, "id"), _format(format), path.encode("utf-8")), "the path")


def place(id: int, row: int, col: int, cols: int, rows: int, look: Optional[Look] = None) -> str:
    """Places object ``id`` centered on screen cell (``row``, ``col``),
    counting from 0, over ``cols`` × ``rows`` cells."""
    return made(lib().flow_model_place(u32(id, "id"), u16(row, "row"), u16(col, "col"), u16(cols, "cols"), u16(rows, "rows"), _look(look)), "the placement")


def place_over(id: int, top: int, left: int, cols: int, rows: int, look: Optional[Look] = None) -> str:
    """Places object ``id`` over the ``cols`` × ``rows`` cells whose
    top-left is (``top``, ``left``)."""
    return made(lib().flow_model_place_over(u32(id, "id"), u16(top, "top"), u16(left, "left"), u16(cols, "cols"), u16(rows, "rows"), _look(look)), "the placement")


def update(id: int, look: Look) -> str:
    """Changes how placed object ``id`` looks, without placing it again."""
    return made(lib().flow_model_update(u32(id, "id"), _look(look)), "the look")


def delete(id: int) -> str:
    """Deletes object ``id`` and takes its placement away."""
    return made(lib().flow_model_delete(u32(id, "id")), "the id")


def delete_all() -> str:
    """Deletes every object the program's tab holds."""
    return made(lib().flow_model_delete_all(), "the delete")
