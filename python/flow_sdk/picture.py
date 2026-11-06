"""Pictures with the kitty graphics protocol, as Flow shows them: sent
once as a virtual placement and shown by placeholder cells (U+10EEEE in
the picture's id as a color), which scroll and clear as text does. The
functions here return the escapes as strings; :func:`show` does the whole
of it, bare cells on Windows included.
"""

import ctypes
import os
from dataclasses import dataclass
from typing import Optional, Tuple

from . import _native
from ._native import lib, made, u16, u32
from .term import _flush

#: The placement a picture gets unless the program picks another.
PLACEMENT = 1
#: The most rows or columns a picture's cells can number.
MAX_CELLS = 297

_FORMATS = {"rgb": (24, 3), "rgba": (32, 4), "png": (100, None)}


@dataclass(frozen=True)
class Image:
    """A picture to send: ``rgb`` or ``rgba`` pixels (rows top first,
    straight alpha) or a ``png`` file's bytes."""

    format: str
    width: int
    height: int
    data: bytes

    @classmethod
    def rgba(cls, width: int, height: int, data) -> "Image":
        return cls("rgba", width, height, bytes(data))

    @classmethod
    def rgb(cls, width: int, height: int, data) -> "Image":
        return cls("rgb", width, height, bytes(data))

    @classmethod
    def png(cls, data) -> "Image":
        """A PNG file's bytes; the terminal reads its size from them."""
        return cls("png", 0, 0, bytes(data))

    def _c(self):
        """The image as C takes it, and the bytes it points at, which have
        to outlive the call."""
        if self.format not in _FORMATS:
            raise ValueError("an image is rgb, rgba or png, not %r" % self.format)
        code, per_pixel = _FORMATS[self.format]
        if per_pixel and len(self.data) != self.width * self.height * per_pixel:
            raise ValueError("%d × %d %s pixels take %d bytes, not %d" % (self.width, self.height, self.format, self.width * self.height * per_pixel, len(self.data)))
        buffer = ctypes.c_char_p(self.data)
        image = _native.FlowImage(code, u32(self.width, "width"), u32(self.height, "height"), ctypes.cast(buffer, ctypes.POINTER(ctypes.c_uint8)), len(self.data))
        return image, buffer


def first_id(seed: Optional[int] = None) -> int:
    """An id to start the program's pictures from, picked by ``seed`` (the
    process's id unless given) in the 24 bits a cell's color holds. Count up
    from it for more, staying at or under 0xFFFFFF."""
    seed = os.getpid() if seed is None else seed
    return lib().flow_picture_id(seed & 0xFFFFFFFF)


def upload(id: int, image: Image, cols: int, rows: int, placement: int = PLACEMENT) -> str:
    """The commands that send ``image`` as picture ``id``, shown by
    ``placement`` over ``cols`` × ``rows`` cells. Sending an id again
    replaces it. RGBA that's opaque throughout goes as RGB."""
    c_image, _keep = image._c()
    return made(lib().flow_picture_upload(u32(id, "id"), ctypes.byref(c_image), u32(placement, "placement"), u16(cols, "cols"), u16(rows, "rows")), "the picture")


def transmit(id: int, image: Image) -> str:
    """The commands that send ``image`` and only keep it, for a program
    that places it with its own commands."""
    c_image, _keep = image._c()
    return made(lib().flow_picture_transmit(u32(id, "id"), ctypes.byref(c_image)), "the picture")


def delete(id: int) -> str:
    """Forgets picture ``id`` and its pixels."""
    return made(lib().flow_picture_delete(u32(id, "id")), "the id")


def delete_shown() -> str:
    """Takes away every picture the screen in use shows, keeping the
    scrollback's and the other screen's."""
    return made(lib().flow_picture_delete_shown(), "the delete")


def placeholder_row(id: int, row: int, cols: int, start: int = 0, placement: int = PLACEMENT) -> str:
    """Row ``row`` of a picture's cells, ``cols`` wide from its column
    ``start``: write it where the row goes."""
    return made(lib().flow_picture_placeholder_row(u32(id, "id"), u32(placement, "placement"), u16(row, "row"), u16(start, "start"), u16(cols, "cols")), "the row")


def placeholders(id: int, at: Tuple[int, int], cols: int, rows: int, placement: int = PLACEMENT) -> str:
    """All of a picture's cells, with its top-left at screen cell ``at``
    (column, row, from 0), each row placed by the cursor."""
    col, row = at
    return made(lib().flow_picture_placeholders(u32(id, "id"), u32(placement, "placement"), u16(col, "col"), u16(row, "row"), u16(cols, "cols"), u16(rows, "rows")), "the cells")


def bare_row(id: int, cols: int) -> str:
    """A row of bare cells (``caps.bare``, through ConPTY on Windows): the
    id's color and the character alone. Each row of a taller picture is
    then a picture of its own."""
    return made(lib().flow_picture_bare_row(u32(id, "id"), u16(cols, "cols")), "the row")


def show(terminal, id: int, image: Image, cols: int, rows: int, at: Optional[Tuple[int, int]] = None) -> int:
    """Sends ``image`` as picture ``id`` where ``terminal`` takes pictures
    and writes the cells that show it over ``cols`` × ``rows`` cells: with
    its top-left at ``at`` (column, row), or at the cursor, a line each. On
    bare cells each row is a picture of its own, ids ``id`` up. Returns how
    many ids it took. Check ``terminal.caps.pictures`` first."""
    c_image, _keep = image._c()
    col, row = (-1, -1) if at is None else (u16(at[0], "col"), u16(at[1], "row"))
    _flush()
    ids = lib().flow_picture_show(terminal._live(), u32(id, "id"), ctypes.byref(c_image), u16(cols, "cols"), u16(rows, "rows"), col, row)
    if ids < 0:
        raise ValueError("the picture couldn't be shown (a PNG on bare cells can't be)")
    return ids
