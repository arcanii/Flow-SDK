"""A theme package for Flow, from Python: a video terminal in white
phosphor, typed in at 9600 baud, and an aquarium that brings the water's
liquid seen from below, packed as a .tar.gz ready for ``flowapp install``
in a Flow tab, which puts them in Settings' Theme row and the new-tab
picker. It needs no terminal; the package goes where it's told, else into
the current folder.

    cargo build -p flow-sdk-c
    python3 -I python/examples/theme.py [out.tar.gz]
"""

import sys
from pathlib import Path

# Run from Flow's repository without installing the package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from flow_sdk import theme  # noqa: E402


def main() -> int:
    vt52 = (
        theme.Theme("vt52")
        .name("VT52")
        .name_in("ja", "VT52 端末")
        .fonts(["VT323", "Glass TTY VT220"])
        .text(0xE8F0F0)
        .cursor(0xE8F0F0)
        .background(0x0A0F0A)
        .phosphor(0xE8F0F0)
        .blink()
        .crt("classic")
        .start(["", "^{tall}VT52 DECscope", "^{tall}VT52 DECscope", "", "^READY", ""])
        .baud(9600)
        .prompt("$ ")
        .sounds("terminal")
        .screen(80, 24)
    )
    aquarium = (
        theme.Theme("aquarium")
        .name("Aquarium")
        .text(0xD8F4FF)
        .cursor(0x7CE7FF)
        .background(0x02202E)
        .cursor_shape("bar")
        .key("fx", "style", "water")
        .key("fx", "water_view", "below")
        .key("fx", "sun", 0.8)
    )
    package = theme.Package("flow-sdk-examples", "1.0", [vt52, aquarium])

    # What Flow wouldn't take is refused before anything's written, saying why.
    try:
        theme.Package("flow-sdk-examples", "1.0", [theme.Theme("VT100")]).check()
    except theme.Refused as why:
        print("Not packed: %s" % why)

    try:
        tarball = package.pack(sys.argv[1] if len(sys.argv) > 1 else None)
    except (theme.Refused, OSError) as why:
        print("The package wasn't made: %s" % why, file=sys.stderr)
        return 1
    print("Packed %s: in a Flow tab, flowapp install %s" % (tarball, tarball))
    return 0


if __name__ == "__main__":
    sys.exit(main())
