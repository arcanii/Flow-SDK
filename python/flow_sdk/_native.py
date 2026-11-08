"""Finds Flow's C library (the flow-sdk-c crate) and declares its functions
for ctypes.

The library is looked for, in order:

1. at ``FLOW_SDK_LIB``: the library's own path, or a folder holding it
   (nothing else is tried when it's set, so a wrong one says so);
2. in Cargo's ``target/release`` and then ``target/debug`` under
   ``CARGO_TARGET_DIR``, if that's set;
3. in this package's own folder, where a wheel built for one platform
   carries it;
4. in ``target/release`` and then ``target/debug`` beside each folder above
   this package (the workspace's, when the package is used from Flow's
   repository);
5. where the system keeps libraries (``ctypes.util.find_library``).

It's loaded on first use, not on import, so the rest of the package (and
its tests of these rules) work before the library is built.
"""

import ctypes
import ctypes.util
import os
import sys
from pathlib import Path

from ctypes import (
    POINTER,
    Structure,
    c_bool,
    c_char_p,
    c_float,
    c_int,
    c_int32,
    c_size_t,
    c_uint8,
    c_uint16,
    c_uint32,
    c_void_p,
)

#: The interface this package was written for: ``FLOW_SDK_ABI`` in
#: flow_sdk.h. A library of another refuses to load.
ABI = 1

STEM = "flow_sdk_c"


def library_name(platform=None):
    """The library's file name on ``platform`` (this one's by default)."""
    platform = platform or sys.platform
    if platform == "darwin":
        return "lib%s.dylib" % STEM
    if platform.startswith("win") or platform == "cygwin":
        return "%s.dll" % STEM
    return "lib%s.so" % STEM


def candidates(environ=None, here=None, platform=None):
    """Where the library is looked for, in order (see the module's notes)."""
    environ = os.environ if environ is None else environ
    here = Path(__file__).resolve().parent if here is None else Path(here)
    name = library_name(platform)
    given = environ.get("FLOW_SDK_LIB")
    if given:
        path = Path(given)
        return [path / name if path.is_dir() else path]
    def built(targets):
        return [target / profile / name for target in targets for profile in ("release", "debug")]

    given_target = [Path(environ["CARGO_TARGET_DIR"])] if environ.get("CARGO_TARGET_DIR") else []
    return built(given_target) + [here / name] + built(folder / "target" for folder in [here, *here.parents])


class FlowSize(Structure):
    _fields_ = [("cols", c_uint16), ("rows", c_uint16), ("cell_width", c_float), ("cell_height", c_float)]


class FlowCaps(Structure):
    _fields_ = [
        ("pictures", c_bool),
        ("models", c_bool),
        ("flow", c_bool),
        ("bare", c_bool),
        ("piped", c_bool),
        ("cell_width", c_uint16),
        ("cell_height", c_uint16),
        ("area_width", c_uint32),
        ("area_height", c_uint32),
    ]


class FlowEvent(Structure):
    _fields_ = [
        ("kind", c_int32),
        ("key", c_uint32),
        ("mods", c_uint32),
        ("col", c_uint16),
        ("row", c_uint16),
        ("button", c_uint32),
    ]


class FlowImage(Structure):
    _fields_ = [
        ("format", c_int32),
        ("width", c_uint32),
        ("height", c_uint32),
        ("data", POINTER(c_uint8)),
        ("len", c_size_t),
    ]


class FlowStageLayout(Structure):
    _fields_ = [
        ("left", c_uint16),
        ("top", c_uint16),
        ("cols", c_uint16),
        ("rows", c_uint16),
        ("below", c_uint16),
        ("width", c_float),
        ("height", c_float),
    ]


class FlowTone(Structure):
    _fields_ = [
        ("hz", c_float),
        ("secs", c_float),
        ("wave", c_int32),
        ("volume", c_float),
        ("pan", c_float),
        ("attack", c_float),
        ("release", c_float),
    ]


class FlowMaterial(Structure):
    _fields_ = [
        ("color", c_float * 4),
        ("gloss", c_float),
        ("metal", c_float),
        ("glow", c_float * 3),
        ("glow_strength", c_float),
        ("coat", c_float),
        ("coat_gloss", c_float),
        ("unlit", c_bool),
    ]


class FlowLook(Structure):
    _fields_ = [
        ("animate", c_int32),
        ("color", c_int32),
        ("scale", c_float),
        ("depth", c_float),
        ("brightness", c_float),
        ("offset", c_float * 3),
        ("rotation", c_float * 3),
        ("scale3", c_float * 3),
    ]


_FLOATS3 = POINTER(c_float)  # any number of floats: a point, a color, a rotation
_TERMINAL = c_void_p
_STRING = c_void_p  # an owned `char *`, kept as a pointer to free
_ERROR = POINTER(c_void_p)  # a `char **` that may come back with a reason to free

#: Each function's result and arguments, as flow_sdk.h declares them. A
#: returned string is declared as a plain pointer, so it can go back to
#: flow_free once read (``c_char_p`` would copy it and lose the pointer).
SIGNATURES = {
    "flow_abi_version": (c_uint32, []),
    "flow_free": (None, [c_void_p]),
    "flow_in_flow": (c_bool, []),
    "flow_is_terminal": (c_bool, []),
    "flow_raw_mode": (c_int, [c_bool]),
    "flow_term_enter": (c_int, [c_bool]),
    "flow_term_leave": (None, []),
    "flow_term_size": (c_int, [POINTER(FlowSize)]),
    "flow_detect": (_TERMINAL, [c_uint32, c_uint32]),
    "flow_terminal_plain": (_TERMINAL, []),
    "flow_terminal_free": (None, [_TERMINAL]),
    "flow_terminal_caps": (c_int, [_TERMINAL, POINTER(FlowCaps)]),
    "flow_terminal_version": (_STRING, [_TERMINAL]),
    "flow_terminal_background": (c_int, [_TERMINAL, POINTER(c_uint32)]),
    "flow_is_light": (c_bool, [c_uint32]),
    "flow_terminal_cell_size": (c_int, [_TERMINAL, POINTER(c_float), POINTER(c_float)]),
    "flow_terminal_send": (c_int, [_TERMINAL, c_char_p, c_size_t]),
    "flow_terminal_send_effects": (c_int, [_TERMINAL, c_char_p, c_size_t]),
    "flow_write": (c_int, [c_char_p]),
    "flow_write_free": (c_int, [c_void_p]),
    "flow_next_event": (c_int, [c_uint32, POINTER(FlowEvent)]),
    "flow_picture_id": (c_uint32, [c_uint32]),
    "flow_picture_upload": (_STRING, [c_uint32, POINTER(FlowImage), c_uint32, c_uint16, c_uint16]),
    "flow_picture_transmit": (_STRING, [c_uint32, POINTER(FlowImage)]),
    "flow_picture_delete": (_STRING, [c_uint32]),
    "flow_picture_delete_shown": (_STRING, []),
    "flow_picture_placeholder_row": (_STRING, [c_uint32, c_uint32, c_uint16, c_uint16, c_uint16]),
    "flow_picture_placeholders": (_STRING, [c_uint32, c_uint32, c_uint16, c_uint16, c_uint16, c_uint16]),
    "flow_picture_bare_row": (_STRING, [c_uint32, c_uint16]),
    "flow_picture_show": (c_int, [_TERMINAL, c_uint32, POINTER(FlowImage), c_uint16, c_uint16, c_int32, c_int32]),
    "flow_look_init": (None, [POINTER(FlowLook)]),
    "flow_model_register": (_STRING, [c_uint32, c_int32, c_char_p, c_size_t, c_char_p]),
    "flow_model_register_chunks": (_STRING, [c_uint32, c_int32, c_char_p, c_size_t, c_char_p, c_size_t]),
    "flow_model_register_path": (_STRING, [c_uint32, c_int32, c_char_p]),
    "flow_model_place": (_STRING, [c_uint32, c_uint16, c_uint16, c_uint16, c_uint16, POINTER(FlowLook)]),
    "flow_model_place_over": (_STRING, [c_uint32, c_uint16, c_uint16, c_uint16, c_uint16, POINTER(FlowLook)]),
    "flow_model_update": (_STRING, [c_uint32, POINTER(FlowLook)]),
    "flow_model_delete": (_STRING, [c_uint32]),
    "flow_model_delete_all": (_STRING, []),
    "flow_mesh_new": (c_void_p, []),
    "flow_mesh_free": (None, [c_void_p]),
    "flow_mesh_face": (c_int, [c_void_p, _FLOATS3, _FLOATS3, c_size_t, _FLOATS3]),
    "flow_mesh_face_in": (c_int, [c_void_p, POINTER(FlowMaterial), _FLOATS3, c_size_t, _FLOATS3]),
    "flow_mesh_triangle": (c_int, [c_void_p, _FLOATS3, _FLOATS3, _FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_mesh_triangles": (c_size_t, [c_void_p]),
    "flow_mesh_glb": (c_void_p, [c_void_p, POINTER(c_size_t)]),
    "flow_mesh_glb_packed": (c_void_p, [c_void_p, POINTER(c_size_t)]),
    "flow_material_init": (c_int, [POINTER(FlowMaterial), _FLOATS3]),
    "flow_effect_new": (c_void_p, [c_int32, c_uint16, c_uint16]),
    "flow_effect_free": (None, [c_void_p]),
    "flow_effect_to": (c_int, [c_void_p, c_uint16, c_uint16]),
    "flow_effect_color": (c_int, [c_void_p, c_uint32]),
    "flow_effect_dir": (c_int, [c_void_p, c_float, c_float]),
    "flow_effect_size": (c_int, [c_void_p, c_float]),
    "flow_effect_strength": (c_int, [c_void_p, c_float]),
    "flow_effect_volume": (c_int, [c_void_p, c_float]),
    "flow_effect_sound": (c_int, [c_void_p, c_int32]),
    "flow_effect_quiet": (c_int, [c_void_p]),
    "flow_effect_escape": (_STRING, [c_void_p]),
    "flow_sound": (_STRING, [c_int32, c_int32, c_int32, c_float, c_float]),
    "flow_sound_named": (c_int, [c_char_p]),
    "flow_notes": (_STRING, [c_uint8, c_uint8, POINTER(c_uint8), c_size_t]),
    "flow_note": (c_int, [c_char_p]),
    "flow_tint": (_STRING, [c_uint32, c_float]),
    "flow_budget_new": (c_void_p, [c_float, c_float]),
    "flow_budget_free": (None, [c_void_p]),
    "flow_budget_take": (c_bool, [c_void_p, c_float, c_float]),
    "flow_budget_left": (c_float, [c_void_p, c_float]),
    "flow_settings_request": (_STRING, [c_char_p]),
    "flow_settings_answer": (_STRING, [c_char_p, c_size_t]),
    "flow_settings_ask": (_STRING, [c_char_p, c_uint32]),
    "flow_scene_new": (c_void_p, [c_float, c_float]),
    "flow_scene_free": (None, [c_void_p]),
    "flow_scene_polygon": (c_int, [c_void_p, _FLOATS3, _FLOATS3, c_size_t]),
    "flow_scene_polygon_in": (c_int, [c_void_p, POINTER(FlowMaterial), _FLOATS3, c_size_t]),
    "flow_scene_flat": (c_int, [c_void_p, _FLOATS3, _FLOATS3, c_size_t, _FLOATS3]),
    "flow_scene_flat_in": (c_int, [c_void_p, POINTER(FlowMaterial), _FLOATS3, c_size_t, _FLOATS3]),
    "flow_scene_triangles": (c_size_t, [c_void_p]),
    "flow_scene_glb": (c_void_p, [c_void_p, c_float, c_float, c_char_p, POINTER(c_size_t)]),
    "flow_scene_glb_packed": (c_void_p, [c_void_p, c_float, c_float, c_char_p, POINTER(c_size_t)]),
    "flow_camera_orbit": (c_void_p, [_FLOATS3, c_float, c_float, c_float, c_float, c_float]),
    "flow_camera_free": (None, [c_void_p]),
    "flow_camera_project": (c_int, [c_void_p, _FLOATS3, _FLOATS3]),
    "flow_camera_turn": (c_int, [c_void_p, _FLOATS3, _FLOATS3]),
    "flow_camera_face": (c_int, [c_void_p, c_void_p, _FLOATS3, _FLOATS3, c_size_t, _FLOATS3]),
    "flow_camera_face_in": (c_int, [c_void_p, c_void_p, POINTER(FlowMaterial), _FLOATS3, c_size_t, _FLOATS3]),
    "flow_lit_as": (c_int, [_FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_v3_add": (c_int, [_FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_v3_sub": (c_int, [_FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_v3_mul": (c_int, [_FLOATS3, c_float, _FLOATS3]),
    "flow_v3_dot": (c_float, [_FLOATS3, _FLOATS3]),
    "flow_v3_cross": (c_int, [_FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_v3_normalize": (c_int, [_FLOATS3, _FLOATS3]),
    "flow_v3_lerp": (c_int, [_FLOATS3, _FLOATS3, c_float, _FLOATS3]),
    "flow_smooth": (c_float, [c_float, c_float, c_float]),
    "flow_m3_rotation": (c_int, [_FLOATS3, c_float, _FLOATS3]),
    "flow_m3_apply": (c_int, [_FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_m3_compose": (c_int, [_FLOATS3, _FLOATS3, _FLOATS3]),
    "flow_rgb": (c_int, [c_uint32, _FLOATS3]),
    "flow_mix": (c_int, [_FLOATS3, _FLOATS3, c_float, _FLOATS3]),
    "flow_shade": (c_float, [_FLOATS3]),
    "flow_stage_new": (c_void_p, [POINTER(FlowSize), c_uint16, c_uint16, c_uint16]),
    "flow_stage_free": (None, [c_void_p]),
    "flow_stage_id": (c_int, [c_void_p, c_uint32]),
    "flow_stage_made_by": (c_int, [c_void_p, c_char_p]),
    "flow_stage_packed": (c_int, [c_void_p, c_bool]),
    "flow_stage_layout": (c_int, [c_void_p, POINTER(FlowStageLayout)]),
    "flow_stage_cell_at": (c_int, [c_void_p, c_float, c_float, POINTER(c_uint16), POINTER(c_uint16)]),
    "flow_stage_point_at": (c_int, [c_void_p, c_uint16, c_uint16, POINTER(c_float), POINTER(c_float)]),
    "flow_stage_show": (_STRING, [c_void_p, c_void_p]),
    "flow_stage_clear": (_STRING, [c_void_p]),
    "flow_stage_effect": (c_void_p, [c_void_p, c_int32, _FLOATS3, _FLOATS3, _FLOATS3, c_float, c_float]),
    "flow_screen_new": (c_void_p, [c_uint16, c_uint16]),
    "flow_screen_free": (None, [c_void_p]),
    "flow_screen_text": (c_int, [c_void_p, c_int32, c_int32, c_char_p, c_uint32, c_int32, c_bool]),
    "flow_screen_image": (c_int, [c_void_p, c_int32, c_int32, c_uint16, c_uint16, c_uint32]),
    "flow_screen_clear": (c_int, [c_void_p, c_int32, c_int32, c_int32, c_int32, c_int32]),
    "flow_screen_ground": (c_int, [c_void_p, c_int32, c_int32, c_int32, c_int32, c_uint32]),
    "flow_screen_row": (_STRING, [c_void_p, c_uint16]),
    "flow_screen_show": (_STRING, [c_void_p, c_bool]),
    "flow_screen_forget": (c_int, [c_void_p]),
    "flow_theme_new": (c_void_p, [c_char_p]),
    "flow_theme_free": (None, [c_void_p]),
    "flow_theme_name": (c_int, [c_void_p, c_char_p]),
    "flow_theme_name_in": (c_int, [c_void_p, c_char_p, c_char_p]),
    "flow_theme_fonts": (c_int, [c_void_p, POINTER(c_char_p), c_size_t]),
    "flow_theme_font_file": (c_int, [c_void_p, c_char_p]),
    "flow_theme_text": (c_int, [c_void_p, c_uint32]),
    "flow_theme_cursor": (c_int, [c_void_p, c_uint32]),
    "flow_theme_background": (c_int, [c_void_p, c_uint32]),
    "flow_theme_border": (c_int, [c_void_p, c_uint32]),
    "flow_theme_ansi": (c_int, [c_void_p, POINTER(c_uint32), c_size_t]),
    "flow_theme_phosphor": (c_int, [c_void_p, c_uint32]),
    "flow_theme_cursor_shape": (c_int, [c_void_p, c_char_p]),
    "flow_theme_blink": (c_int, [c_void_p, c_bool]),
    "flow_theme_cursor_speed": (c_int, [c_void_p, c_float]),
    "flow_theme_crt": (c_int, [c_void_p, c_char_p]),
    "flow_theme_start": (c_int, [c_void_p, POINTER(c_char_p), c_size_t]),
    "flow_theme_baud": (c_int, [c_void_p, c_uint32]),
    "flow_theme_prompt": (c_int, [c_void_p, c_char_p]),
    "flow_theme_sounds": (c_int, [c_void_p, c_char_p]),
    "flow_theme_screen": (c_int, [c_void_p, c_uint16, c_uint16]),
    "flow_theme_logo": (c_int, [c_void_p, c_char_p]),
    "flow_theme_key": (c_int, [c_void_p, c_char_p, c_char_p, c_char_p, _ERROR]),
    "flow_theme_check": (c_int, [c_void_p, _ERROR]),
    "flow_theme_package_new": (c_void_p, [c_char_p, c_char_p]),
    "flow_theme_package_free": (None, [c_void_p]),
    "flow_theme_package_add": (c_int, [c_void_p, c_void_p]),
    "flow_theme_package_check": (c_int, [c_void_p, _ERROR]),
    "flow_theme_package_json": (_STRING, [c_void_p, _ERROR]),
    "flow_theme_package_file_name": (_STRING, [c_void_p]),
    "flow_theme_package_write": (c_int, [c_void_p, c_char_p, _ERROR]),
    "flow_theme_package_pack": (c_int, [c_void_p, c_char_p, _ERROR]),
    "flow_audio_built": (c_bool, []),
    "flow_audio_plays": (c_bool, []),
    "flow_pitch": (c_float, [c_char_p]),
    "flow_tone_init": (c_int, [POINTER(FlowTone), c_float, c_float]),
    "flow_tune_new": (c_void_p, [c_char_p, c_float, POINTER(FlowTone), c_bool, _ERROR]),
    "flow_tune_length": (c_float, [c_void_p]),
    "flow_tune_free": (None, [c_void_p]),
    "flow_song_new": (c_void_p, [c_float]),
    "flow_song_line": (c_int, [c_void_p, c_int32, c_float, c_char_p, _ERROR]),
    "flow_song_length": (c_float, [c_void_p]),
    "flow_song_free": (None, [c_void_p]),
    "flow_audio_new": (c_void_p, []),
    "flow_audio_free": (None, [c_void_p]),
    "flow_audio_play": (c_int, [c_void_p, POINTER(FlowTone)]),
    "flow_audio_play_tune": (c_int, [c_void_p, c_void_p]),
    "flow_audio_play_song": (c_int, [c_void_p, c_void_p]),
    "flow_audio_stop_song": (c_int, [c_void_p]),
    "flow_audio_stop": (c_int, [c_void_p]),
    "flow_audio_set_volume": (c_int, [c_void_p, c_float]),
    "flow_mixer_new": (c_void_p, [c_float]),
    "flow_mixer_free": (None, [c_void_p]),
    "flow_mixer_play": (c_int, [c_void_p, POINTER(FlowTone)]),
    "flow_mixer_play_tune": (c_int, [c_void_p, c_void_p]),
    "flow_mixer_play_song": (c_int, [c_void_p, c_void_p]),
    "flow_mixer_stop_song": (c_int, [c_void_p]),
    "flow_mixer_stop": (c_int, [c_void_p]),
    "flow_mixer_set_volume": (c_int, [c_void_p, c_float]),
    "flow_mixer_busy": (c_bool, [c_void_p]),
    "flow_mixer_fill": (c_int, [c_void_p, POINTER(c_float), c_size_t, c_uint32]),
}

_lib = None


def lib():
    """The library, loaded and declared on the first call."""
    global _lib
    if _lib is None:
        _lib = _load()
    return _lib


def _load():
    tried = candidates()
    path = next((p for p in tried if p.is_file()), None)
    if path is None and not os.environ.get("FLOW_SDK_LIB"):
        found = ctypes.util.find_library(STEM)
        path = Path(found) if found else None
    if path is None:
        raise OSError(
            "Flow's C library (%s) wasn't found. Build it with `cargo build --release -p flow-sdk-c`, "
            "or name it in FLOW_SDK_LIB. Looked at: %s" % (library_name(), ", ".join(str(p) for p in tried))
        )
    library = ctypes.CDLL(str(path))
    library.flow_abi_version.restype = c_uint32
    library.flow_abi_version.argtypes = []
    version = library.flow_abi_version()
    if version != ABI:
        raise OSError("%s speaks version %d of Flow's C interface, and this package version %d" % (path, version, ABI))
    for name, (result, arguments) in SIGNATURES.items():
        try:
            function = getattr(library, name)
        except AttributeError:
            # The same interface, but a library from before the function
            # was added.
            raise OSError("%s has no %s: it's older than this package; build it again" % (path, name)) from None
        function.restype = result
        function.argtypes = arguments
    return library


def take(pointer):
    """A string a function handed out, read and freed; None for NULL."""
    if not pointer:
        return None
    try:
        return ctypes.string_at(pointer).decode("utf-8")
    finally:
        lib().flow_free(pointer)


def take_bytes(pointer, length):
    """A buffer a function handed out, read and freed; None for NULL."""
    if not pointer:
        return None
    try:
        return ctypes.string_at(pointer, length)
    finally:
        lib().flow_free(pointer)


def made(pointer, what):
    """A string a function made, or ValueError naming what was refused."""
    text = take(pointer)
    if text is None:
        raise ValueError("Flow's library refused %s" % what)
    return text


def floats(values, count):
    """``count`` floats for a ``const float *``."""
    values = [float(v) for v in values]
    if len(values) != count:
        raise ValueError("expected %d numbers, got %d" % (count, len(values)))
    return (c_float * count)(*values)


def points(values, width=3):
    """Points of ``width`` numbers each, flattened for a ``const float *``,
    and how many there are."""
    values = [list(p) for p in values]
    if any(len(p) != width for p in values):
        raise ValueError("a point is %d numbers" % width)
    return floats([v for p in values for v in p], width * len(values)), len(values)


class Reason:
    """A ``char **error`` for a function that may say why it refused."""

    def __init__(self):
        self.pointer = c_void_p()

    def ref(self):
        return ctypes.byref(self.pointer)

    def take(self):
        """The reason given, freed; None if none was."""
        return take(self.pointer.value)


def u16(value, name):
    """A cell count or position, which C takes as 16 bits."""
    value = int(value)
    if not 0 <= value <= 0xFFFF:
        raise ValueError("%s must be from 0 to 65535, not %d" % (name, value))
    return value


def i32(value, name):
    """A cell's column or row, which may lie off the screen: 32 bits, signed."""
    value = int(value)
    if not -0x80000000 <= value <= 0x7FFFFFFF:
        raise ValueError("%s must fit in 32 bits, not %d" % (name, value))
    return value


def u32(value, name):
    value = int(value)
    if not 0 <= value <= 0xFFFFFFFF:
        raise ValueError("%s must be from 0 to 4294967295, not %d" % (name, value))
    return value


def text(value):
    """``str`` or ``bytes`` as the bytes C reads."""
    return value.encode("utf-8") if isinstance(value, str) else bytes(value)


class Handle:
    """One of the library's objects, freed by its own ``_free`` (named in
    :attr:`FREE`) when it's closed, left as a context, or collected."""

    FREE = ""
    WHAT = "object"

    def __init__(self, handle, refused="it"):
        if not handle:
            raise ValueError("Flow's library refused %s" % refused)
        self._handle = handle

    def _live(self):
        if not self._handle:
            raise ValueError("the %s was closed" % self.WHAT)
        return self._handle

    def close(self) -> None:
        if getattr(self, "_handle", None):
            getattr(lib(), self.FREE)(self._handle)
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
