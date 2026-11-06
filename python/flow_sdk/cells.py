"""Screens of cells: letters in their colors, with pieces of pictures among
them, drawn into each frame and sent as the difference from the screen shown
before, only the cells that changed (the SDK's ``flow_sdk::screen``). A
:class:`Screen` holds both, the one being drawn and the one shown last, so
a program draws into it and asks for what to send::

    import flow_sdk as flow

    with flow.screen():
        size = flow.size()
        cells = flow.Screen(size.cols, size.rows)
        while playing:
            cells.clear()
            cells.text(2, 1, "hp 12", 0xF04040, bold=True)
            flow.write(cells.show())

Cells count from 0 at the top left, and what falls off the screen isn't
drawn. Colors are 0xRRGGBB; a letter with no background keeps the
terminal's own, which in Flow is the glass and the liquid, until
:meth:`Screen.ground` lays one of the program's own under it.
"""

from typing import Optional, Union

from . import _native
from ._native import Handle, i32, lib, u16, u32

#: The gray of a blank cell's letters, and of text given no color.
GRAY = 0xC8C8C8


def _background(bg: Optional[int]) -> int:
    """A background for C: 0xRRGGBB, or -1 for the terminal's own."""
    if bg is None:
        return -1
    bg = int(bg)
    if not 0 <= bg <= 0xFFFFFF:
        raise ValueError("a background is 0xRRGGBB or None, not %#x" % bg)
    return bg


class Screen(Handle):
    """A screen ``cols`` × ``rows`` cells (:func:`flow_sdk.size`'s), blank,
    with nothing shown yet. Make a new one after a resize."""

    FREE = "flow_screen_free"
    WHAT = "screen"

    def __init__(self, cols: int, rows: int):
        handle = lib().flow_screen_new(u16(cols, "cols"), u16(rows, "rows"))
        super().__init__(handle, "a screen %r × %r: it has cells" % (cols, rows))
        self.cols, self.rows = int(cols), int(rows)

    def text(self, col: int, row: int, text: Union[str, bytes], fg: int = GRAY, bg: Optional[int] = None, bold: bool = False) -> int:
        """Writes ``text`` from cell (``col``, ``row``), a cell a character,
        in ``fg`` on ``bg`` (None for the terminal's own), bold or not;
        returns the column after it."""
        if isinstance(text, bytes):
            text = text.decode("utf-8")
        status = lib().flow_screen_text(self._live(), i32(col, "col"), i32(row, "row"), _native.text(text), u32(fg, "fg") & 0xFFFFFF, _background(bg), bool(bold))
        if status != 0:
            raise ValueError("Flow's library refused the text")
        return int(col) + len(text)

    def image(self, col: int, row: int, cols: int, rows: int, id: int) -> "Screen":
        """Picture ``id`` over ``cols`` × ``rows`` cells from (``col``,
        ``row``), each on the background of the cell it covers: send it
        first (:func:`flow_sdk.picture.upload`, at the placement it takes
        unless given, over as many cells)."""
        if lib().flow_screen_image(self._live(), i32(col, "col"), i32(row, "row"), u16(cols, "cols"), u16(rows, "rows"), u32(id, "id")) != 0:
            raise ValueError("Flow's library refused the picture")
        return self

    def clear(self, col: int = 0, row: int = 0, cols: Optional[int] = None, rows: Optional[int] = None, bg: Optional[int] = None) -> "Screen":
        """Blanks ``cols`` × ``rows`` cells from (``col``, ``row``) on ``bg``:
        all of them unless told, to draw the next frame afresh."""
        cols = self.cols if cols is None else cols
        rows = self.rows if rows is None else rows
        if lib().flow_screen_clear(self._live(), i32(col, "col"), i32(row, "row"), i32(cols, "cols"), i32(rows, "rows"), _background(bg)) != 0:
            raise ValueError("Flow's library refused the clear")
        return self

    def ground(self, bg: int, col: int = 0, row: int = 0, cols: Optional[int] = None, rows: Optional[int] = None) -> "Screen":
        """Lays ``bg`` under every cell of ``cols`` × ``rows`` from (``col``,
        ``row``), all of them unless told, that has no background of its
        own: after drawing, it leaves nothing to the terminal's background,
        so a light one can't wash the screen out."""
        cols = self.cols if cols is None else cols
        rows = self.rows if rows is None else rows
        if bg is None or not 0 <= int(bg) <= 0xFFFFFF:
            raise ValueError("a ground is 0xRRGGBB, not %r" % (bg,))
        if lib().flow_screen_ground(self._live(), i32(col, "col"), i32(row, "row"), i32(cols, "cols"), i32(rows, "rows"), int(bg)) != 0:
            raise ValueError("Flow's library refused the ground")
        return self

    def row(self, row: int) -> str:
        """Row ``row``'s letters, a picture's cells as spaces."""
        text = _native.take(lib().flow_screen_row(self._live(), u16(row, "row")))
        if text is None:
            raise ValueError("there's no row %d on a screen %d high" % (row, self.rows))
        return text

    def show(self, bare: bool = False) -> str:
        """The escapes that bring the terminal from the screen shown last (a
        cleared one the first time) to this one, picture cells bare if
        ``bare`` (:attr:`flow_sdk.Caps.bare`, where each row of a taller
        picture has to be a picture of its own). It's shown from then on,
        and stays to draw over. Write it after the pictures it shows."""
        return _native.made(lib().flow_screen_show(self._live(), bool(bare)), "the screen")

    def forget(self) -> None:
        """Has the next :meth:`show` draw everything, as after the terminal
        was cleared or another program drew on it."""
        if lib().flow_screen_forget(self._live()) != 0:
            raise ValueError("Flow's library refused the screen")
