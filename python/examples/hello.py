"""Flow's SDK from Python: builds a cut gem as a mesh, asks the terminal
what it can do, and where it shows 3D models places the gem turning in the
middle of the screen; in Flow it also ripples the liquid round it with a
chime. Any key, a click or a few seconds ends it. Outside a terminal, or in
one that shows none of this, it says so and stops.

    cargo build -p flow-sdk-c
    python3 -I python/examples/hello.py
"""

import sys
from pathlib import Path

# Run from Flow's repository without installing the package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import flow_sdk as flow  # noqa: E402
from flow_sdk import model  # noqa: E402


def cut_gem() -> flow.Mesh:
    """An octahedron, its upper faces gold and its lower ones amber."""
    gem = flow.Mesh()
    ring = [(1, 0, 0), (0, 0, 1), (-1, 0, 0), (0, 0, -1)]
    for a, b in zip(ring, ring[1:] + ring[:1]):
        gem.triangle(0xFACC4C, a, b, (0, 1, 0))
        gem.triangle(0xD9731A, b, a, (0, -1, 0))
    return gem


def main() -> int:
    with cut_gem() as gem:
        glb, triangles = gem.glb(), gem.triangles

    # Raw keys on the alternate screen before asking, so the answers aren't
    # echoed; it doesn't go on outside a terminal.
    with flow.screen() as entered:
        terminal = flow.detect(models=True, version=True)
        size = flow.size()
        if entered and size and size.cols >= 20 and size.rows >= 8 and (terminal.caps.models or terminal.caps.flow):
            show(terminal, glb, size)
            return 0
    print("This terminal shows no 3D models and isn't Flow, so the gem (%d triangles, %d bytes of glTF) goes unseen." % (triangles, len(glb)))
    return 0


def show(terminal: flow.Terminal, glb: bytes, size: flow.Size) -> None:
    col, row = size.cols // 2, size.rows // 2
    if terminal.caps.models:
        terminal.send(model.register(1, glb, "glb", name="gem.glb"))
        terminal.send(model.place(1, row, col, 16, 8, flow.Look(animate=True)))
    if terminal.caps.flow:
        terminal.send_effects(flow.Effect.ripple((col, row)).size(10).sound("chime").escape())
    flow.next_event(timeout=4)
    if terminal.caps.models:
        terminal.send(model.delete(1))


if __name__ == "__main__":
    sys.exit(main())
