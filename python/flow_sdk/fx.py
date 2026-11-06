"""Flow's liquid and its sounds: an :class:`Effect` at a cell (OSC 7731),
one of Flow's sounds by name, DECPS notes, and the glass's tint; and a
:class:`Budget` that keeps a program under Flow's allowance of them.

Flow takes effects from the tab in front only, about six a second after a
burst of eight, at cells on the screen shown. Another terminal ignores
them, but send them only in Flow to be tidy (``terminal.caps.flow``), with
``terminal.send_effects`` (down Flow's pipe on Windows).
"""

import ctypes
from typing import Iterable, Optional, Tuple, Union

from ._native import lib, made, u16

KINDS = ("bolt", "burst", "ripple", "stir")
#: Flow's sounds by name, in FLOW_SOUND_* order.
SOUNDS = ("drop", "splash", "thunder", "whoosh", "chime", "low", "ding", "thud", "click", "boing", "bell")

Cell = Tuple[int, int]


def _sound(name: str) -> int:
    try:
        return SOUNDS.index(name.lower())
    except (ValueError, AttributeError):
        raise ValueError("Flow has no sound %r; it has %s" % (name, ", ".join(SOUNDS))) from None


class Effect:
    """An effect at a cell ``at`` (column, row, from the screen's top-left
    from 0). Each setter returns the effect, so they chain:
    ``Effect.burst((12, 3)).color(0xff8000).size(2.5).sound("chime")``."""

    def __init__(self, kind: str, at: Cell):
        if kind not in KINDS:
            raise ValueError("an effect is one of %s, not %r" % (", ".join(KINDS), kind))
        self._handle = lib().flow_effect_new(KINDS.index(kind), u16(at[0], "col"), u16(at[1], "row"))
        if not self._handle:
            raise MemoryError("Flow's library made no effect")

    @classmethod
    def _adopt(cls, handle) -> "Effect":
        """The effect behind a handle the library made elsewhere (a
        stage's), freed with the effect."""
        effect = cls.__new__(cls)
        effect._handle = handle
        return effect

    @classmethod
    def bolt(cls, at: Cell, to: Cell) -> "Effect":
        """Lightning from ``at`` to ``to``."""
        return cls("bolt", at).to(to)

    @classmethod
    def burst(cls, at: Cell) -> "Effect":
        """A splash, in its color where the liquid takes color."""
        return cls("burst", at)

    @classmethod
    def ripple(cls, at: Cell) -> "Effect":
        """A ring spreading."""
        return cls("ripple", at)

    @classmethod
    def stir(cls, at: Cell, dir: Tuple[float, float]) -> "Effect":
        """The liquid pushed along ``dir`` (y down)."""
        return cls("stir", at).dir(dir)

    def _set(self, result: int, what: str) -> "Effect":
        if result != 0:
            raise ValueError("Flow's library refused the effect's %s" % what)
        return self

    def _live(self):
        if not self._handle:
            raise ValueError("the effect was closed")
        return self._handle

    def to(self, at: Cell) -> "Effect":
        """Where a bolt ends."""
        return self._set(lib().flow_effect_to(self._live(), u16(at[0], "col"), u16(at[1], "row")), "end")

    def color(self, rgb: int) -> "Effect":
        """0xRRGGBB."""
        return self._set(lib().flow_effect_color(self._live(), int(rgb) & 0xFFFFFF), "color")

    def dir(self, dir: Tuple[float, float]) -> "Effect":
        """Which way a stir pushes, y down."""
        return self._set(lib().flow_effect_dir(self._live(), float(dir[0]), float(dir[1])), "direction")

    def size(self, columns: float) -> "Effect":
        """How far it reaches, 0.5 to 40 columns."""
        return self._set(lib().flow_effect_size(self._live(), float(columns)), "size")

    def strength(self, strength: float) -> "Effect":
        """How hard, 0 to 1."""
        return self._set(lib().flow_effect_strength(self._live(), float(strength)), "strength")

    def volume(self, volume: float) -> "Effect":
        """Its sound at ``volume`` (0 to 1) of what Flow would make it."""
        return self._set(lib().flow_effect_volume(self._live(), float(volume)), "volume")

    def sound(self, name: str) -> "Effect":
        """One of Flow's sounds (:data:`SOUNDS`) in place of its kind's."""
        return self._set(lib().flow_effect_sound(self._live(), _sound(name)), "sound")

    def quiet(self) -> "Effect":
        """No sound at all."""
        return self._set(lib().flow_effect_quiet(self._live()), "sound")

    def escape(self) -> str:
        return made(lib().flow_effect_escape(self._live()), "the effect")

    __str__ = escape

    def close(self) -> None:
        if self._handle:
            lib().flow_effect_free(self._handle)
            self._handle = None

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass


class Budget:
    """What a program may set off and stay under Flow's allowance, so
    nothing it sends is dropped unseen: a full ``burst`` of effects and
    named sounds coming back at ``per_second`` a second (7, then 5 a
    second, a little under Flow's 8 and 6, unless given). Time is the
    program's own clock in seconds; notes and the tint don't count."""

    def __init__(self, per_second: Optional[float] = None, burst: Optional[float] = None):
        nan = float("nan")
        self._handle = lib().flow_budget_new(nan if per_second is None else float(per_second), nan if burst is None else float(burst))
        if not self._handle:
            raise MemoryError("Flow's library made no budget")

    def _live(self):
        if not self._handle:
            raise ValueError("the budget was closed")
        return self._handle

    def take(self, now: float, keep: float = 0.0) -> bool:
        """Spends one at ``now`` if ``keep`` would still be left after it,
        for what matters more; False, and nothing spent, if not."""
        return bool(lib().flow_budget_take(self._live(), float(now), float(keep)))

    def left(self, now: float) -> float:
        """How many are left at ``now``."""
        return lib().flow_budget_left(self._live(), float(now))

    def close(self) -> None:
        if self._handle:
            lib().flow_budget_free(self._handle)
            self._handle = None

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass


def sound(name: str, at: Optional[Cell] = None, strength: Optional[float] = None, volume: Optional[float] = None) -> str:
    """One of Flow's sounds alone, placed left or right by ``at``'s column,
    as strong as ``strength`` (0.5 unless given), at ``volume`` (full
    unless given)."""
    col, row = (-1, -1) if at is None else (u16(at[0], "col"), u16(at[1], "row"))
    nan = float("nan")
    pointer = lib().flow_sound(_sound(name), col, row, nan if strength is None else float(strength), nan if volume is None else float(volume))
    return made(pointer, "the sound")


def note(name: str) -> Optional[int]:
    """DECPS's number for a note by name ("C5" 1 to "C7" 25, "C#5" or
    "Db5", "r" a rest 0), or None past its two octaves."""
    n = lib().flow_note(name.encode("utf-8"))
    return None if n < 0 else n


def notes(volume: int, length: int, tune: Iterable[Union[int, str]]) -> str:
    """Notes as DECPS plays them: ``volume`` 0 (silent) to 7, each note
    ``length`` 32nds of a second, the notes as numbers (1 C5 to 25 C7, 0 a
    rest) or names. Empty for no notes."""
    numbers = []
    for n in tune:
        if isinstance(n, str):
            number = note(n)
            if number is None:
                raise ValueError("DECPS plays C5 to C7, not %r" % n)
            n = number
        numbers.append(max(0, min(int(n), 255)))
    if not numbers:
        return ""
    array = (ctypes.c_uint8 * len(numbers))(*numbers)
    return made(lib().flow_notes(max(0, min(int(volume), 255)), max(0, min(int(length), 255)), array, len(numbers)), "the notes")


def tint(color: int, strength: float) -> str:
    """Washes Flow's glass in ``color`` (0xRRGGBB) as far as ``strength``
    (0 to 1) says while the program holds the alternate screen, as a game's
    night. Send it when it changes, not each frame."""
    return made(lib().flow_tint(int(color) & 0xFFFFFFFF, float(strength)), "the tint")


def tint_off() -> str:
    """Washes the tint off again."""
    return tint(0, 0.0)
