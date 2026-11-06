"""Scenes: a picture worked out afresh each frame, for the terminal to draw
as a 3D model. A :class:`Scene` is in the picture's pixels as the screen
shows them, from its middle, x right and y up (z only orders what's in
front, nearer being more), built of flat convex polygons cut to the
picture; a :class:`Camera` turns a world in 3D into those pixels, in
perspective; and :func:`lit_as` gives a face the color that shows as wanted
once Flow's light falls on it. A :class:`flow_sdk.Stage` sends a scene each
frame and fits it to the cells. A polygon is in a color, matte and lit, or
a :class:`Material`: polished, metal, glowing, coated or unlit.

The vector maths to work a scene out with is the SDK's own, through the
library, so a point comes out as the Rust SDK has it: :func:`add`,
:func:`sub`, :func:`mul`, :func:`dot`, :func:`cross`, :func:`normalize`,
:func:`lerp` and :func:`smooth` for points and numbers, :func:`rotation`,
:func:`apply` and :func:`compose` (and :data:`IDENTITY`) for turning them,
:func:`mix` and :func:`shade` for colors. Points come back as tuples of three,
rotations as three rows of three::

    from flow_sdk.scene import apply, compose, rotation

    turn = compose(rotation((0, 1, 0), t * 0.8), rotation((1, 0, 0), t * 0.5))
    corner = apply(turn, (1, 1, 1))
"""

import ctypes
from typing import Optional, Sequence, Tuple

from . import _native
from ._native import Handle, floats, lib
from .mesh import Color, Surface, Material, Point, _color, _material, rgb

#: A polygon's corner: where it is, and which way the surface faces there.
Corner = Tuple[Point, Point]
Triple = Tuple[float, float, float]
#: A rotation, by rows.
Rotation = Tuple[Triple, Triple, Triple]

__all__ = [
    "IDENTITY",
    "Camera",
    "Material",
    "Scene",
    "add",
    "apply",
    "compose",
    "cross",
    "dot",
    "lerp",
    "lit_as",
    "mix",
    "mul",
    "normalize",
    "rgb",
    "rotation",
    "shade",
    "smooth",
    "sub",
]

#: No turn at all.
IDENTITY: Rotation = ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))


class Scene(Handle):
    """A frame's picture, ``width`` × ``height`` pixels as the screen shows
    them (a stage's :attr:`~flow_sdk.Stage.width` and
    :attr:`~flow_sdk.Stage.height`). Its polygons are wound as Flow lights
    them, whichever way their corners go round."""

    FREE = "flow_scene_free"
    WHAT = "scene"

    def __init__(self, width: float, height: float):
        super().__init__(lib().flow_scene_new(float(width), float(height)), "a scene %r × %r: its sides are above 0" % (width, height))
        self.width, self.height = float(width), float(height)

    def polygon(self, color: Surface, corners: Sequence[Corner]) -> "Scene":
        """A flat convex polygon of three corners or more, each ``(at,
        normal)``: where it is and which way the surface faces there (z
        toward the viewer), so a curved surface shades smoothly. It's in a
        color or a :class:`Material`."""
        data, count = _native.points([list(at) + list(normal) for at, normal in corners], 6)
        if count < 3:
            raise ValueError("a polygon has three corners at least, not %d" % count)
        material = _material(color)
        if material is not None:
            done = lib().flow_scene_polygon_in(self._live(), ctypes.byref(material), data, count)
        else:
            done = lib().flow_scene_polygon(self._live(), _color(color), data, count)
        if done != 0:
            raise ValueError("Flow's library refused the polygon")
        return self

    def flat(self, color: Surface, points: Sequence[Point], normal: Point = (0.0, 0.0, 1.0)) -> "Scene":
        """A polygon of three points or more facing ``normal`` all over."""
        data, count = _native.points(points)
        if count < 3:
            raise ValueError("a polygon has three points at least, not %d" % count)
        material = _material(color)
        if material is not None:
            done = lib().flow_scene_flat_in(self._live(), ctypes.byref(material), data, count, floats(normal, 3))
        else:
            done = lib().flow_scene_flat(self._live(), _color(color), data, count, floats(normal, 3))
        if done != 0:
            raise ValueError("Flow's library refused the polygon")
        return self

    @property
    def triangles(self) -> int:
        return lib().flow_scene_triangles(self._live())

    def glb(self, squash: Tuple[float, float] = (1.0, 1.0), generator: Optional[str] = None, packed: bool = False) -> bytes:
        """The scene as a GLB, its points stretched by ``squash`` and named
        as ``generator`` made it ("flow-sdk" unless given); ``packed`` into
        whole numbers (KHR_mesh_quantization), half the size, for Flow. A
        stage does this itself."""
        length = ctypes.c_size_t()
        name = None if generator is None else generator.encode("utf-8")
        made = lib().flow_scene_glb_packed if packed else lib().flow_scene_glb
        pointer = made(self._live(), float(squash[0]), float(squash[1]), name, ctypes.byref(length))
        data = _native.take_bytes(pointer, length.value)
        if data is None:
            raise ValueError("Flow's library refused the squash %r (each above 0)" % (squash,))
        return data


class Camera(Handle):
    """A camera in a world whose y is up, seeing it in perspective: looking
    at ``target`` from ``distance`` away, turned ``yaw`` radians to the
    right of looking from in front (+z) and ``pitch`` above it, seeing
    ``fov`` radians up and down in a picture ``height`` pixels tall (the
    SDK's ``Camera::orbit``). Make one each frame it moves."""

    FREE = "flow_camera_free"
    WHAT = "camera"

    def __init__(self, target: Point, yaw: float, pitch: float, distance: float, fov: float, height: float):
        handle = lib().flow_camera_orbit(floats(target, 3), float(yaw), float(pitch), float(distance), float(fov), float(height))
        super().__init__(handle, "the camera: its numbers are finite")

    def project(self, p: Point) -> Optional[Triple]:
        """Where point ``p`` of the world shows in the picture (x, y and a
        depth growing as one over the distance), or None behind the eye."""
        out = (ctypes.c_float * 3)()
        if lib().flow_camera_project(self._live(), floats(p, 3), out) != 0:
            return None
        return (out[0], out[1], out[2])

    def turn(self, n: Point) -> Triple:
        """Direction ``n`` of the world as the picture has it: x right, y
        up, z toward the eye."""
        out = (ctypes.c_float * 3)()
        if lib().flow_camera_turn(self._live(), floats(n, 3), out) != 0:
            raise ValueError("Flow's library refused the direction")
        return (out[0], out[1], out[2])

    def face(self, scene: Scene, color: Surface, points: Sequence[Point], normal: Point) -> None:
        """A flat face of the world turned ``normal`` way, in a color or a
        :class:`Material`, drawn into ``scene`` if it faces the eye and is
        all in front of it."""
        data, count = _native.points(points)
        if count < 3:
            raise ValueError("a face has three points at least, not %d" % count)
        material = _material(color)
        if material is not None:
            done = lib().flow_camera_face_in(self._live(), scene._live(), ctypes.byref(material), data, count, floats(normal, 3))
        else:
            done = lib().flow_camera_face(self._live(), scene._live(), _color(color), data, count, floats(normal, 3))
        if done != 0:
            raise ValueError("Flow's library refused the face")


def lit_as(color: Color, normal: Point) -> Tuple[float, float, float, float]:
    """The color to give a face turned ``normal`` way (x right, y up, z
    toward the viewer) for it to show as ``color`` once Flow's light, from
    above left, falls on it. Other terminals may light models otherwise."""
    out = _color(color)
    if lib().flow_lit_as(out, floats(normal, 3), out) != 0:
        raise ValueError("Flow's library refused the color")
    return (out[0], out[1], out[2], out[3])


def _floats(count):
    return (ctypes.c_float * count)()


def _done(status, what):
    if status != 0:
        raise ValueError("Flow's library refused %s" % what)


def _triple(out) -> Triple:
    return (out[0], out[1], out[2])


def _matrix(m: Sequence[Sequence[float]]):
    """A rotation's three rows as the nine floats C takes."""
    rows = [list(row) for row in m]
    if len(rows) != 3 or any(len(row) != 3 for row in rows):
        raise ValueError("a rotation is three rows of three numbers")
    return floats([v for row in rows for v in row], 9)


def _rows(out) -> Rotation:
    return ((out[0], out[1], out[2]), (out[3], out[4], out[5]), (out[6], out[7], out[8]))


def add(a: Point, b: Point) -> Triple:
    """``a`` + ``b``."""
    out = _floats(3)
    _done(lib().flow_v3_add(floats(a, 3), floats(b, 3), out), "the points")
    return _triple(out)


def sub(a: Point, b: Point) -> Triple:
    """``a`` less ``b``."""
    out = _floats(3)
    _done(lib().flow_v3_sub(floats(a, 3), floats(b, 3), out), "the points")
    return _triple(out)


def mul(a: Point, k: float) -> Triple:
    """``a`` times ``k``."""
    out = _floats(3)
    _done(lib().flow_v3_mul(floats(a, 3), float(k), out), "the point")
    return _triple(out)


def dot(a: Point, b: Point) -> float:
    """The dot product of ``a`` and ``b``."""
    return lib().flow_v3_dot(floats(a, 3), floats(b, 3))


def cross(a: Point, b: Point) -> Triple:
    """The cross product of ``a`` and ``b``, the right-hand way."""
    out = _floats(3)
    _done(lib().flow_v3_cross(floats(a, 3), floats(b, 3), out), "the points")
    return _triple(out)


def normalize(a: Point) -> Triple:
    """``a`` made a length of 1; (0, 0, 1) for one too short to have a way."""
    out = _floats(3)
    _done(lib().flow_v3_normalize(floats(a, 3), out), "the point")
    return _triple(out)


def lerp(a: Point, b: Point, t: float) -> Triple:
    """The point ``t`` of the way from ``a`` to ``b``."""
    out = _floats(3)
    _done(lib().flow_v3_lerp(floats(a, 3), floats(b, 3), float(t), out), "the points")
    return _triple(out)


def smooth(a: float, b: float, t: float) -> float:
    """Eases from 0 to 1 as ``t`` goes from ``a`` to ``b``, still at both ends."""
    return lib().flow_smooth(float(a), float(b), float(t))


def rotation(axis: Point, angle: float) -> Rotation:
    """Turning by ``angle`` radians about ``axis``, the right-hand way."""
    out = _floats(9)
    _done(lib().flow_m3_rotation(floats(axis, 3), float(angle), out), "the axis")
    return _rows(out)


def apply(m: Rotation, v: Point) -> Triple:
    """Rotation ``m`` turning ``v``."""
    out = _floats(3)
    _done(lib().flow_m3_apply(_matrix(m), floats(v, 3), out), "the rotation")
    return _triple(out)


def compose(a: Rotation, b: Rotation) -> Rotation:
    """Turning by ``b`` and then by ``a``, as one rotation."""
    out = _floats(9)
    _done(lib().flow_m3_compose(_matrix(a), _matrix(b), out), "the rotations")
    return _rows(out)


def mix(a: Color, b: Color, t: float) -> Tuple[float, float, float, float]:
    """The color ``t`` of the way from ``a`` to ``b``, alpha too."""
    out = _floats(4)
    _done(lib().flow_mix(_color(a), _color(b), float(t), out), "the colors")
    return (out[0], out[1], out[2], out[3])


def shade(normal: Point) -> float:
    """How much of Flow's light a face turned ``normal`` way (x right, y up,
    z toward the viewer) gets: what :func:`lit_as` divides by."""
    return lib().flow_shade(floats(normal, 3))
