"""Meshes: objects built of flat convex faces in code (y up, any units),
made into the GLB bytes that :func:`flow_sdk.model.register` sends, for
the terminal to turn and light itself. A face is in a color (matte and
lit) or a :class:`Material`, which says how polished, metal, glowing,
coated or unlit it is, as a scene's polygons are.
"""

import ctypes
from typing import Optional, Sequence, Tuple, Union

from . import _native
from ._native import floats, lib

Point = Sequence[float]


def rgb(hex: int, alpha: float = 1.0) -> Tuple[float, float, float, float]:
    """0xRRGGBB as red, green, blue and alpha from 0 to 1."""
    return (((hex >> 16) & 0xFF) / 255.0, ((hex >> 8) & 0xFF) / 255.0, (hex & 0xFF) / 255.0, alpha)


def _rgba(color) -> Tuple[float, float, float, float]:
    if isinstance(color, int):
        color = rgb(color)
    color = list(color)
    if len(color) == 3:
        color.append(1.0)
    if len(color) != 4:
        raise ValueError("a color is 0xRRGGBB, or red, green, blue and alpha, not %r" % (color,))
    return tuple(float(c) for c in color)


def _color(color):
    return floats(_rgba(color), 4)


class Material:
    """How a surface looks: its ``color`` (0xRRGGBB, or red, green, blue and
    alpha from 0 to 1), how polished it is (``gloss``, 0 matte to 1), how
    much a ``metal`` (0 to 1), the light it gives off (``glow``, 0xRRGGBB
    or red, green and blue as shown, at ``glow_strength``), a clear
    ``coat`` over it (0 to 1, polished to ``coat_gloss``), or ``unlit``,
    its color alone with no light falling on it. A color alone is matte
    and lit, and written as a color always was."""

    def __init__(self, color, gloss: float = 0.0, metal: float = 0.0, glow=None, glow_strength: float = 1.0, coat: float = 0.0, coat_gloss: float = 0.0, unlit: bool = False):
        self.color = _rgba(color)
        self.gloss, self.metal = float(gloss), float(metal)
        self.glow = (0.0, 0.0, 0.0) if glow is None else _rgba(glow)[:3]
        self.glow_strength = float(glow_strength)
        self.coat, self.coat_gloss = float(coat), float(coat_gloss)
        self.unlit = bool(unlit)

    def _struct(self) -> _native.FlowMaterial:
        m = _native.FlowMaterial()
        if lib().flow_material_init(ctypes.byref(m), floats(self.color, 4)) != 0:
            raise ValueError("Flow's library refused the color %r" % (self.color,))
        m.gloss, m.metal, m.glow_strength, m.coat, m.coat_gloss, m.unlit = self.gloss, self.metal, self.glow_strength, self.coat, self.coat_gloss, self.unlit
        m.glow[:] = self.glow
        return m

    def __eq__(self, other):
        return isinstance(other, Material) and vars(self) == vars(other)

    def __repr__(self):
        return "Material(%s)" % ", ".join("%s=%r" % kv for kv in vars(self).items())


Color = Union[int, Sequence[float]]
#: What a face is in: a color, or a :class:`Material`.
Surface = Union[Color, Material]


def _material(look) -> Optional[_native.FlowMaterial]:
    """The material for ``look``, or None for a color alone."""
    return look._struct() if isinstance(look, Material) else None


class Mesh:
    """An object being built. Each face is flat and convex and faces away
    from ``inside`` (the object's middle, for one that's solid), whichever
    way its points go round."""

    def __init__(self):
        self._handle = lib().flow_mesh_new()
        if not self._handle:
            raise MemoryError("Flow's library made no mesh")

    def _live(self):
        if not self._handle:
            raise ValueError("the mesh was closed")
        return self._handle

    def face(self, color: Surface, points: Sequence[Point], inside: Point = (0.0, 0.0, 0.0)) -> "Mesh":
        """A face of three points or more, in ``color`` (0xRRGGBB, or red,
        green, blue and alpha from 0 to 1) or a :class:`Material`."""
        points = [list(p) for p in points]
        if len(points) < 3:
            raise ValueError("a face has three points at least, not %d" % len(points))
        if any(len(p) != 3 for p in points):
            raise ValueError("a point is x, y and z")
        flat = floats([v for p in points for v in p], 3 * len(points))
        material = _material(color)
        if material is not None:
            done = lib().flow_mesh_face_in(self._live(), ctypes.byref(material), flat, len(points), floats(inside, 3))
        else:
            done = lib().flow_mesh_face(self._live(), _color(color), flat, len(points), floats(inside, 3))
        if done != 0:
            raise ValueError("Flow's library refused the face")
        return self

    def triangle(self, color: Surface, a: Point, b: Point, c: Point, inside: Point = (0.0, 0.0, 0.0)) -> "Mesh":
        if isinstance(color, Material):
            return self.face(color, [a, b, c], inside)
        if lib().flow_mesh_triangle(self._live(), _color(color), floats(a, 3), floats(b, 3), floats(c, 3), floats(inside, 3)) != 0:
            raise ValueError("Flow's library refused the triangle")
        return self

    @property
    def triangles(self) -> int:
        return lib().flow_mesh_triangles(self._live())

    def glb(self, packed: bool = False) -> bytes:
        """The object as a GLB, a primitive for each material; ``packed``
        into whole numbers (KHR_mesh_quantization), half the size, for
        Flow, which reads them where not every terminal does."""
        length = ctypes.c_size_t()
        made = lib().flow_mesh_glb_packed if packed else lib().flow_mesh_glb
        data = _native.take_bytes(made(self._live(), ctypes.byref(length)), length.value)
        if data is None:
            raise MemoryError("Flow's library made no GLB")
        return data

    def close(self) -> None:
        if self._handle:
            lib().flow_mesh_free(self._handle)
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
