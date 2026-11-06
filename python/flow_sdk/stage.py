"""Stages: where a :class:`~flow_sdk.Scene` goes over the terminal's cells.
A stage takes as many cells as one placement may cover in Flow, centered
above the rows the program keeps for its words; each frame's scene goes as
the same object (``terminal.send(stage.show(scene))``), placed the first
time and not sent at all when it's the last frame again; and points of the
picture and the screen's cells map both ways, for clicks and for the
liquid's effects::

    stage = flow.Stage.fit(flow.size(), terminal.caps.cell, words=1)
    scene = flow.Scene(stage.width, stage.height)              # each frame
    camera = flow.Camera((0, 0, 0), yaw, 0.4, 6.0, 0.7, stage.height)
    camera.face(scene, 0x7CE7FF, square, (0, 0, 1))            # and so on
    terminal.send(stage.show(scene))
"""

import ctypes
from typing import Optional, Tuple

from . import _native
from ._native import Handle, floats, lib, u16, u32
from .fx import KINDS, Effect
from .scene import Scene
from .term import Size

Pixel = Tuple[float, float]


class Stage(Handle):
    """A scene's place over the cells: made with :meth:`fit`. Its picture
    is :attr:`width` × :attr:`height` of the screen's pixels, over
    :attr:`cols` × :attr:`rows` cells from (:attr:`left`, :attr:`top`),
    with the program's words from row :attr:`below`."""

    FREE = "flow_stage_free"
    WHAT = "stage"

    def __init__(self, handle):
        super().__init__(handle, "the stage")
        layout = _native.FlowStageLayout()
        lib().flow_stage_layout(handle, ctypes.byref(layout))
        self.left, self.top, self.cols, self.rows, self.below = layout.left, layout.top, layout.cols, layout.rows, layout.below
        self.width, self.height = layout.width, layout.height

    @classmethod
    def fit(cls, size: Optional[Size], cell: Optional[Tuple[int, int]] = None, words: int = 1) -> Optional["Stage"]:
        """The stage for a terminal of ``size`` (:func:`flow_sdk.size`, with
        a cell's pixels from :meth:`Terminal.cell_size` where it gave none)
        whose cells have ``cell`` whole pixels (``terminal.caps.cell``),
        keeping ``words`` rows below it. None if it would be under 16 × 6
        cells or no cell size is known at all: fit another after a resize."""
        if size is None:
            return None
        cell_width, cell_height = size.cell or (0.0, 0.0)
        c_size = _native.FlowSize(u16(size.cols, "cols"), u16(size.rows, "rows"), float(cell_width), float(cell_height))
        whole = cell or (0, 0)
        handle = lib().flow_stage_new(ctypes.byref(c_size), u16(whole[0], "a cell's width"), u16(whole[1], "a cell's height"), u16(words, "words"))
        return cls(handle) if handle else None

    def id(self, id: int) -> "Stage":
        """Its object's id, in place of 1."""
        if lib().flow_stage_id(self._live(), u32(id, "id")) != 0:
            raise ValueError("Flow's library refused the id")
        return self

    def made_by(self, generator: str) -> "Stage":
        """Names the program in each frame's GLB, in place of "flow-sdk"."""
        if lib().flow_stage_made_by(self._live(), generator.encode("utf-8")) != 0:
            raise ValueError("Flow's library refused the name")
        return self

    def packed(self, packed: bool = True) -> "Stage":
        """Sends each frame packed into whole numbers (KHR_mesh_quantization),
        half the size: for Flow, which reads them, where not every terminal
        does. ``False`` sends them as floats again."""
        if lib().flow_stage_packed(self._live(), bool(packed)) != 0:
            raise ValueError("Flow's library refused to pack the stage")
        return self

    def cell_at(self, at: Pixel) -> Tuple[int, int]:
        """The screen cell (column, row) a point of the picture is in, or
        the nearest on it."""
        col, row = ctypes.c_uint16(), ctypes.c_uint16()
        if lib().flow_stage_cell_at(self._live(), float(at[0]), float(at[1]), ctypes.byref(col), ctypes.byref(row)) != 0:
            raise ValueError("a point of the picture is two finite numbers, not %r" % (at,))
        return (col.value, row.value)

    def point_at(self, col: int, row: int) -> Optional[Pixel]:
        """The point of the picture at the middle of screen cell (``col``,
        ``row``), or None if the cell isn't on the picture: for clicks."""
        x, y = ctypes.c_float(), ctypes.c_float()
        if lib().flow_stage_point_at(self._live(), u16(col, "col"), u16(row, "row"), ctypes.byref(x), ctypes.byref(y)) != 0:
            return None
        return (x.value, y.value)

    def show(self, scene: Scene) -> str:
        """What shows ``scene``, to send with :meth:`Terminal.send`: the
        object registered again, and the first time where it goes. Empty
        when it's the last frame again."""
        return _native.made(lib().flow_stage_show(self._live(), scene._live()), "the scene")

    def clear(self) -> str:
        """Takes the picture away: before fitting another after a resize,
        and on the way out."""
        return _native.made(lib().flow_stage_clear(self._live()), "the stage")

    def effect(self, kind: str, at: Pixel, size: float, strength: float = 0.5, to: Optional[Pixel] = None, dir: Optional[Pixel] = None) -> Effect:
        """An effect for Flow's liquid at point ``at`` of the picture,
        reaching ``size`` pixels as hard as ``strength``, put in the
        screen's cells: ``to`` is where a bolt ends and ``dir`` which way a
        stir pushes (y up, as the picture has it). Its color and sound are
        set on the :class:`~flow_sdk.Effect` it gives, as any effect's."""
        if kind not in KINDS:
            raise ValueError("an effect is one of %s, not %r" % (", ".join(KINDS), kind))
        pair = lambda p: None if p is None else floats(p, 2)
        handle = lib().flow_stage_effect(self._live(), KINDS.index(kind), floats(at, 2), pair(to), pair(dir), float(size), float(strength))
        if not handle:
            raise ValueError("Flow's library refused the effect: its numbers are finite")
        return Effect._adopt(handle)
