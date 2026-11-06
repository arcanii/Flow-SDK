"""Flow's SDK for Python: what Flow adds over a plain console, for any
program that runs in it. Pictures (``flow_sdk.picture``), 3D models
(``flow_sdk.model``), meshes built in code (:class:`Mesh`), scenes worked
out each frame (:class:`Scene`, :class:`Camera`, and the vector maths in
``flow_sdk.scene``) and fitted to the cells (:class:`Stage`), screens of
cells sent as their difference from the last, with pictures among the
letters (:class:`Screen`), the liquid's effects and Flow's sounds
(:class:`Effect`, ``flow_sdk.fx``), found out first by asking the terminal
(:func:`detect`) and falling back quietly where it can't show them; Flow's
settings, seen and changed (``flow_sdk.settings``); and audio of the
program's own, songs for a small band played on the machine
(``flow_sdk.audio``).

It's pure Python over Flow's C library (flow-sdk-c) through ctypes; see
``flow_sdk._native`` for where the library is looked for. The escapes come
back as strings, to write with :func:`write` or send with
:meth:`Terminal.send` (graphics) and :meth:`Terminal.send_effects`::

    import flow_sdk as flow

    with flow.screen():
        terminal = flow.detect(models=True, version=True)
        if terminal.caps.flow:
            terminal.send_effects(flow.Effect.ripple((10, 5)).sound("chime").escape())
"""

from . import audio, cells, fx, model, picture, scene, settings, stage
from ._native import ABI
from .fx import Budget, Effect
from .mesh import Material, Mesh, rgb
from .model import Look
from .picture import Image
from .cells import Screen
from .scene import Camera, Scene, lit_as
from .stage import Stage
from .term import (
    Caps,
    Event,
    Size,
    Terminal,
    detect,
    in_flow,
    is_light,
    is_terminal,
    next_event,
    raw_mode,
    screen,
    size,
    write,
)

__version__ = "0.1.0"

__all__ = [
    "ABI",
    "Budget",
    "Camera",
    "Caps",
    "Effect",
    "Event",
    "Image",
    "Look",
    "Material",
    "Mesh",
    "Scene",
    "Screen",
    "Size",
    "Stage",
    "Terminal",
    "audio",
    "cells",
    "detect",
    "fx",
    "in_flow",
    "is_light",
    "is_terminal",
    "lit_as",
    "model",
    "next_event",
    "picture",
    "raw_mode",
    "rgb",
    "scene",
    "screen",
    "settings",
    "size",
    "stage",
    "write",
]
