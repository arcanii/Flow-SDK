"""Flow's SDK from Python: a scene. A cube turning about two axes is drawn
each frame through a camera that stays put, and sent over the window's
cells as one model (a stage), while a dance for a lute, a drone and a tabor
plays where the machine plays sound. A click ripples Flow's liquid where it
lands; any key ends it. Outside a terminal, or in one that shows no 3D
models, it says what goes unseen (and unheard) and stops.

    cargo build -p flow-sdk-c
    python3 -I python/examples/scene.py
"""

import sys
import time
from pathlib import Path
from typing import Optional

# Run from Flow's repository without installing the package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import flow_sdk as flow  # noqa: E402
from flow_sdk import audio  # noqa: E402
from flow_sdk.scene import add, apply, compose, mul, rotation  # noqa: E402

#: Each face: which way it faces, the two ways across it, and its color.
FACES = (
    ((1, 0, 0), (0, 0, 1), (0, 1, 0), 0xE8231C),
    ((-1, 0, 0), (0, 0, 1), (0, 1, 0), 0x2A9DF4),
    ((0, 1, 0), (1, 0, 0), (0, 0, 1), 0xF4F4F4),
    ((0, -1, 0), (1, 0, 0), (0, 0, 1), 0xF2C12E),
    ((0, 0, 1), (1, 0, 0), (0, 1, 0), 0x7CE7FF),
    ((0, 0, -1), (1, 0, 0), (0, 1, 0), 0xB04AD0),
)
SQUARE = ((-1, -1), (1, -1), (1, 1), (-1, 1))
#: Where the cube is turned to when it starts: three of its faces in view.
START = 1.0


def cube(t: float, width: float, height: float) -> flow.Scene:
    """The cube as it's turned ``t`` seconds in, in a picture ``width`` ×
    ``height`` pixels: each corner and each face's way turned as the cube
    is, for the camera to turn into the picture's pixels and leave out the
    faces turned away from it."""
    scene = flow.Scene(width, height)
    turn = compose(rotation((0, 1, 0), t * 0.8), rotation((1, 0, 0), t * 0.5))
    with flow.Camera((0, 0, 0), 0.0, 0.35, 6.0, 0.7, height) as camera:
        for n, u, v, rgb in FACES:
            corners = [apply(turn, add(n, add(mul(u, a), mul(v, b)))) for a, b in SQUARE]
            normal = apply(turn, n)
            # Each face shows its color as given, whichever way it's turned
            # under Flow's light.
            camera.face(scene, flow.lit_as(rgb, camera.turn(normal)), corners, normal)
    return scene


def dance() -> Optional[audio.Song]:
    """Eight beats at 150 a minute, or None from a library without audio."""
    if not audio.available():
        return None
    return audio.Song(150, [
        audio.Line("lute", "D4/1 F4/1 A4/2 G4/1 F4/1 E4/2"),
        audio.Line("drone", "D3/8"),
        audio.Line("tabor", "X/2 x/1 x/1 X/2 x/2", gain=0.7),
    ])


def stage_now(terminal: flow.Terminal) -> Optional[flow.Stage]:
    """A stage for the window as it is now, with the words below it."""
    size = flow.size()
    if size is not None and size.cell is None:
        size = size._replace(cell=terminal.cell_size())
    stage = flow.Stage.fit(size, terminal.caps.cell, words=1)
    flow.write("\x1b[0m\x1b[2J")
    if stage is not None:
        flow.write("\x1b[%d;1Hclick to ripple the liquid, any key ends it" % (stage.below + 1))
    return stage


def main() -> int:
    song = dance()
    # Raw keys on the alternate screen before asking, so the answers aren't
    # echoed, and the mouse's clicks; it doesn't go on outside a terminal.
    with flow.screen(mouse=True) as entered:
        if entered:
            # Clicks, not every move of the mouse: ?1003 off leaves ?1000's
            # clicks and ?1002's drags.
            flow.write("\x1b[?1003l")
        # Models only through the terminal: on Windows, Flow writes a
        # model's cells where ConPTY doesn't know of them.
        terminal = flow.detect(models=True, cell=True, pipe=False)
        stage = stage_now(terminal) if entered and terminal.caps.models else None
        if stage is not None:
            play(terminal, stage, song)
            return 0
    with cube(START, 800, 480) as first:
        triangles = first.triangles
    heard = ", and its dance (%.1f s) unheard." % song.length if song else ", with no audio in this build."
    print("This terminal shows no 3D models here, so the cube (%d triangles a frame) goes unseen%s" % (triangles, heard))
    return 0


def play(terminal: flow.Terminal, stage: flow.Stage, song: Optional[audio.Song]) -> None:
    player = audio.Audio() if song is not None and audio.plays() else None
    if player is not None:
        player.set_volume(0.6)
        player.play_song(song)
    start = time.monotonic()
    try:
        while True:
            if stage is not None:
                with cube(START + time.monotonic() - start, stage.width, stage.height) as scene:
                    terminal.send(stage.show(scene))
            event = flow.next_event(timeout=0.033)
            if event is None:
                continue
            if event.kind == "key":
                break
            if event.kind == "resize":
                if stage is not None:
                    terminal.send(stage.clear())
                stage = stage_now(terminal)
            elif event.kind == "click" and terminal.caps.flow and stage is not None:
                at = stage.point_at(event.col, event.row)
                if at is not None:
                    terminal.send_effects(stage.effect("ripple", at, 160, 0.6).sound("drop").escape())
    finally:
        if stage is not None:
            terminal.send(stage.clear())
        if player is not None:
            player.close()


if __name__ == "__main__":
    sys.exit(main())
