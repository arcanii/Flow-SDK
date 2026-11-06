# Flow-SDK backlog

What's planned for the SDK, its C library and Python package, and the demos.

## Demos: what's left

`flow-demo` plays eight demos (README, [The demos](README.md#the-demos)): the
bouncing ball, the rolling cube, a plasma sent as a kitty picture each
frame, a page of teletext in double lines and mosaics, a vector display's
glowing lines for the CRT, a keyboard of Flow's sounds and DECPS notes, a
day washing the glass's tint, and a balloon read from a model file. Still
to do:

- **Windows and Linux:** the six newest are untried in a live window there:
  on Windows the plasma's bare cells through Flow's pipe, teletext's double
  lines through ConPTY, the clock's `GetLocalTime` and the sounds through
  WASAPI (so far through ConPTY, by script); on Linux all six, the clock's
  `localtime_r` and the sounds through ALSA among them.
- **Snapshots of the demos:** the snapshot's own gem is deleted with every
  model, which takes a demo's too, so pictures of the demos in Flow need
  that step left out.
- **The cube in a tall, narrow pane:** pulled back, from about 4.65 s to
  its rest, it's cut off at both sides in a picture under about 0.6 as wide
  as it's tall (the gem-cut cube's camera did the same). Its far view could
  move back as the picture narrows, as the near view already does
  (`camera_at`, `fit`).
- **The cube's polish**, from its last check: close up in the rolls, glass's
  and jelly's rounds can show straight segments, since their see-through
  budget makes the largest rounds coarser first (the far or half-hidden
  blocks could go first, or the small cube keep its finest cut); metal's
  soft highlight in the middle of every face leans toward glossy plastic (a
  smaller `pillow`, or a highlight drawn out along the face); and stone's
  frames are about 135 KB at rest, twice the others'.

## An SDK for Flow apps: what's left

`flow-sdk` gives programs what Flow adds (README): detection with quiet fallbacks,
kitty pictures (and Flow's pipe on Windows), Ratty models, meshes and
scenes, a screen of cells sent as its difference, OSC 7731 effects and a
queue of them, the glass's tint and sounds, DECPS, audio behind a feature,
and input. The demos and both games are built on it, and it has a C library
(`c/`) and a Python package over that (`python/`), which have all of
it but the queue of effects (a few lines over a program's own clock). It's
licensed MIT OR Apache-2.0. Still to do:

- **Windows and Linux:** on Windows the games' pipe path and bare cells
  work through the SDK, and Python finds the C library's DLL; the music
  through WASAPI and the C library built by a C compiler there are
  untried. On Linux its tests (the C library built with `cc`, its scene
  example among them, and the Python package's) pass, with and without
  audio, and a song played from Python through the C library was heard
  through ALSA, but none of it has been tried in a live Flow window.
- **Published**, once its interfaces settle (`publish = false` now): this
  repository holds it; still to come are the owners on crates.io
  (`flow-sdk`, `flow-sdk-c`) and PyPI (`flow-terminal-sdk`), taking out
  `publish = false`, then wheels built and uploaded for
  each platform (`setup.py` marks one carrying the library), and for C,
  prebuilt libraries with the header for each OS (a signed dylib, a DLL
  with its import library), with CI on all three OSes. Built with Rust 1.88
  itself, which its `rust-version` names, it hasn't been.
