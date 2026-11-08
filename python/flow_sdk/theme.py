"""Theme packages for Flow (from 0.1.34): a folder holding ``themes.json``
and the font files and logo pictures it names, packed as a ``.tar.gz``
that ``flowapp install mine.tar.gz`` installs in a Flow tab. Its themes
then join Settings' Theme row and the new-tab picker, and one with a
built-in machine's id (``green``) takes that one's place::

    from flow_sdk import theme

    vt52 = (theme.Theme("vt52")
            .name("VT52").name_in("ja", "VT52 端末")
            .fonts(["VT323", "Glass TTY VT220"])
            .text(0xE8F0F0).cursor(0xE8F0F0).background(0x0A0F0A)
            .phosphor(0xE8F0F0).crt("classic")
            .start(["", "^VT52 READY", ""]).baud(9600).prompt("$ ")
            .sounds("terminal").screen(80, 24))
    theme.Package("vt-themes", "1.0", [vt52]).pack("vt-themes-1.0.tar.gz")

Flow refuses a whole package for one theme that's wrong, so everything is
checked first as Flow checks it, and nothing is written that fails: a name
Flow hasn't (a cursor shape, a CRT preset, sounds) raises ``ValueError`` at
once, and what only the whole can tell (an id, a screen's size, a font
file that isn't a font, two files for one place) raises :class:`Refused`
with Flow's reason from :meth:`Theme.check`, :meth:`Package.check` and what
writes. A file that can't be read or written raises ``OSError``. Nothing
here needs a terminal. Packing runs the system's ``tar``.
"""

import json as _json
import os
from ctypes import c_char_p, c_uint32
from pathlib import Path
from typing import Any, Iterable, Optional, Sequence, Union

from ._native import Handle, Reason, lib, made, text as _utf8, u16, u32

#: A cursor's shapes, as ``themes.json`` names them.
CURSOR_SHAPES = ("block", "underline", "bar")
#: The CRT presets built into Flow, which a theme's tube takes.
CRT_PRESETS = ("subtle", "classic", "arcade", "phosphor", "television", "monitor")
#: What a machine sounds like, as Flow's ``[theme]`` names them.
SOUNDS = ("quiet", "tube", "teletype", "disk", "fan", "printer", "terminal", "chime", "tones", "floppy")
#: What a function returns where a file couldn't be read or written:
#: ``FLOW_THEME_FAILED``.
FAILED = -2

Path_ = Union[str, "os.PathLike[str]"]


class Refused(ValueError):
    """Flow would refuse the theme or the package, for the reason given."""


def _done(result: int, reason: Reason) -> None:
    """Raises what a function with an ``error`` said, if it failed."""
    if result == 0:
        return
    why = reason.take() or "Flow's library refused it"
    if result == FAILED:
        raise OSError(why)
    raise Refused(why)


def _named(name: str, names: Sequence[str], what: str) -> bytes:
    if name not in names:
        raise ValueError("Flow has no %s %r; it has %s" % (what, name, ", ".join(names)))
    return _utf8(name)


def _texts(items: Iterable[str]):
    """Strings as a ``const char *const *`` and how many there are."""
    encoded = [_utf8(i) for i in items]
    return (c_char_p * len(encoded))(*encoded), len(encoded)


def _path(path: Path_) -> bytes:
    return _utf8(os.fspath(path))


class Theme(Handle):
    """A theme with id ``id`` (lowercase letters, digits and dashes; never
    ``flow`` or ``own``), wearing Flow's own colors, a block cursor, no
    tube, no start screen and no sounds until given its own. Each setter
    returns the theme, so they chain. Colors are 0xRRGGBB."""

    FREE = "flow_theme_free"
    WHAT = "theme"

    def __init__(self, id: str):
        super().__init__(lib().flow_theme_new(_utf8(id)), "the theme %r" % (id,))
        self.id = id

    def _set(self, result: int, what: str) -> "Theme":
        if result != 0:
            raise ValueError("Flow's library refused the theme's %s" % what)
        return self

    def name(self, name: str) -> "Theme":
        """Its name in English, as Settings shows it."""
        return self._set(lib().flow_theme_name(self._live(), _utf8(name)), "name")

    def name_in(self, code: str, name: str) -> "Theme":
        """Its name in the language with code ``code`` ("ja")."""
        return self._set(lib().flow_theme_name_in(self._live(), _utf8(code), _utf8(name)), "name in %s" % code)

    def fonts(self, families: Iterable[str]) -> "Theme":
        """The font families to try first, in place of any before."""
        array, count = _texts(families)
        return self._set(lib().flow_theme_fonts(self._live(), array, count), "fonts")

    def font_file(self, path: Path_) -> "Theme":
        """Brings the font file at ``path``, copied into the package's
        ``fonts/`` under its own name, so :meth:`fonts` can name it."""
        return self._set(lib().flow_theme_font_file(self._live(), _path(path)), "font file")

    def text(self, rgb: int) -> "Theme":
        """Text that programs leave uncolored."""
        return self._set(lib().flow_theme_text(self._live(), u32(rgb, "rgb") & 0xFFFFFF), "text")

    def cursor(self, rgb: int) -> "Theme":
        """The cursor's color."""
        return self._set(lib().flow_theme_cursor(self._live(), u32(rgb, "rgb") & 0xFFFFFF), "cursor")

    def background(self, rgb: int) -> "Theme":
        """The background, the glass's tint."""
        return self._set(lib().flow_theme_background(self._live(), u32(rgb, "rgb") & 0xFFFFFF), "background")

    def border(self, rgb: int) -> "Theme":
        """A border round the text, as a home computer's."""
        return self._set(lib().flow_theme_border(self._live(), u32(rgb, "rgb") & 0xFFFFFF), "border")

    def ansi(self, colors: Sequence[int]) -> "Theme":
        """Programs' sixteen colors: black, red, green, yellow, blue,
        magenta, cyan and white, then the bright ones."""
        colors = [u32(c, "color") & 0xFFFFFF for c in colors]
        if len(colors) != 16:
            raise ValueError("programs' colors are sixteen, not %d" % len(colors))
        return self._set(lib().flow_theme_ansi(self._live(), (c_uint32 * 16)(*colors), 16), "colors")

    def phosphor(self, rgb: int) -> "Theme":
        """Everything shown in shades of ``rgb``, as on a monochrome tube."""
        return self._set(lib().flow_theme_phosphor(self._live(), u32(rgb, "rgb") & 0xFFFFFF), "phosphor")

    def cursor_shape(self, shape: str) -> "Theme":
        """One of :data:`CURSOR_SHAPES`."""
        return self._set(lib().flow_theme_cursor_shape(self._live(), _named(shape, CURSOR_SHAPES, "cursor shape")), "cursor shape")

    def blink(self, blink: bool = True) -> "Theme":
        """Whether the cursor blinks."""
        return self._set(lib().flow_theme_blink(self._live(), bool(blink)), "blink")

    def cursor_speed(self, speed: float) -> "Theme":
        """How fast the cursor glides to its cell: 1 as Flow's, 0.5 to 4."""
        speed = float(speed)
        if not 0.5 <= speed <= 4.0:
            raise ValueError("a cursor's speed is from 0.5 to 4, not %r" % speed)
        return self._set(lib().flow_theme_cursor_speed(self._live(), speed), "cursor speed")

    def crt(self, preset: str) -> "Theme":
        """A tube, as one of :data:`CRT_PRESETS` has it."""
        return self._set(lib().flow_theme_crt(self._live(), _named(preset, CRT_PRESETS, "CRT preset")), "CRT")

    def start(self, lines: Iterable[str]) -> "Theme":
        """The start screen, a line each, typed into each new tab, in place
        of any before: a line starting with ``^`` is centered, and Flow's
        ``{…}`` fields stand for what they say (``{version}``, ``{logo}``,
        ``{wait}``, ``{tall}``, ``{red}``, ``{clear}`` and the rest)."""
        array, count = _texts(lines)
        return self._set(lib().flow_theme_start(self._live(), array, count), "start screen")

    def baud(self, baud: int) -> "Theme":
        """How fast the start screen is typed, in bits a second; 0 at once."""
        return self._set(lib().flow_theme_baud(self._live(), u32(baud, "baud")), "baud")

    def prompt(self, prompt: str) -> "Theme":
        """The prompt it gives the shells: ``\\n`` a new line, ``{cwd}`` the
        folder, ``{n}`` the command's number."""
        return self._set(lib().flow_theme_prompt(self._live(), _utf8(prompt)), "prompt")

    def sounds(self, sounds: str) -> "Theme":
        """What the machine sounds like: one of :data:`SOUNDS`."""
        return self._set(lib().flow_theme_sounds(self._live(), _named(sounds, SOUNDS, "sounds")), "sounds")

    def screen(self, cols: int, rows: int) -> "Theme":
        """Its screen in characters (10 to 500 by 4 to 300), which Fit
        window to offers first."""
        return self._set(lib().flow_theme_screen(self._live(), u16(cols, "cols"), u16(rows, "rows")), "screen")

    def logo(self, path: Path_) -> "Theme":
        """The PNG at ``path`` as the startup logo, copied into the
        package's ``logos/`` under its own name."""
        return self._set(lib().flow_theme_logo(self._live(), _path(path)), "logo")

    def key(self, section: str, key: str, value: Any) -> "Theme":
        """More of the look's settings: ``key`` of ``section`` as Flow's
        config holds it, as ``.key("fx", "style", "water")``. Raises
        :class:`Refused` where it isn't one a theme may bring, or isn't of
        its kind."""
        reason = Reason()
        result = lib().flow_theme_key(self._live(), _utf8(section), _utf8(key), _utf8(_json.dumps(value)), reason.ref())
        _done(result, reason)
        return self

    def check(self) -> None:
        """Raises :class:`Refused` where Flow wouldn't take the theme, or
        ``OSError`` where a file it brings can't be read."""
        reason = Reason()
        _done(lib().flow_theme_check(self._live(), reason.ref()), reason)


class Package(Handle):
    """A package of themes called ``name`` (lowercase letters, digits and
    dashes) at ``version`` ("1.0"), holding ``themes``."""

    FREE = "flow_theme_package_free"
    WHAT = "package"

    def __init__(self, name: str, version: str, themes: Iterable[Theme] = ()):
        super().__init__(lib().flow_theme_package_new(_utf8(name), _utf8(version)), "the package %r" % (name,))
        self.name, self.version = name, version
        for theme in themes:
            self.add(theme)

    def add(self, theme: Theme) -> "Package":
        """Adds a copy of ``theme``, in place of one with its id already
        there; the package, so adds chain."""
        if lib().flow_theme_package_add(self._live(), theme._live()) != 0:
            raise ValueError("Flow's library refused the theme")
        return self

    def check(self) -> None:
        """Raises :class:`Refused` where Flow wouldn't take the package, or
        ``OSError`` where a file a theme brings can't be read."""
        reason = Reason()
        _done(lib().flow_theme_package_check(self._live(), reason.ref()), reason)

    def json(self) -> str:
        """Its ``themes.json``, checked, as it's written."""
        self.check()
        reason = Reason()
        written = lib().flow_theme_package_json(self._live(), reason.ref())
        if not written:
            _done(-1, reason)
        return made(written, "the package")

    @property
    def file_name(self) -> str:
        """``<name>-<version>.tar.gz``, what a packed package is usually
        called."""
        return made(lib().flow_theme_package_file_name(self._live()), "the package")

    def write(self, dir: Path_) -> Path:
        """Writes it, checked, into the folder ``dir`` (made if it isn't
        there): ``themes.json``, and the files its themes bring in
        ``fonts/`` and ``logos/``. The folder."""
        reason = Reason()
        _done(lib().flow_theme_package_write(self._live(), _path(dir), reason.ref()), reason)
        return Path(dir)

    def pack(self, tarball: Optional[Path_] = None) -> Path:
        """Packs it, checked, as the ``.tar.gz`` ``tarball`` (by default
        :attr:`file_name` in the current folder), ready for ``flowapp
        install``. The file."""
        tarball = Path(self.file_name) if tarball is None else Path(tarball)
        reason = Reason()
        _done(lib().flow_theme_package_pack(self._live(), _path(tarball), reason.ref()), reason)
        return tarball
