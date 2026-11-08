# CLAUDE.md

Notes for working in this repository: Flow's SDK, the `flow-sdk` crate at
its root, the same as a C library in `c/` (`flow-sdk-c`) with a Python
package over it in `python/`, and the demos built on it in `demos/`
(`flow-demos`, the `flow-demo` program). README.md is the manual and
BACKLOG.md what's planned. Flow, the terminal
([its releases](https://github.com/arcanii/Flow-releases)), takes both by
a tag of this repository and carries `flow-demo` on its tabs' PATH, so a
new tag reaches Flow's games and demos when Flow moves to it.

## Commands

```bash
cargo test --workspace                     # the SDK, the C library (its header kept in step, hello.c and scene.c built with cc, the Python package's tests through python3 -I) and the demos
cargo test -p flow-sdk --features audio    # the SDK with its mixer
cargo test -p flow-sdk --features theme    # the SDK with its theme packages (the C library turns it on, so --workspace has it)
cargo test -p flow-sdk-c --no-default-features   # the C library without its audio
cargo clippy --workspace --all-targets
cargo run -p flow-sdk --example scene      # its examples: picture, scene, effect, tune (--features audio), theme (--features theme)
cargo run --release -p flow-demos -- boing # a demo (boing, cube, plasma, teletext, vector, sounds, tint, model; none lists them)
```

- Don't run `cargo fmt`: the code isn't rustfmt-formatted (lines often
  pass 100 columns), so it would rewrite every file. Match the style around
  a change by hand.
- Every public item of `flow-sdk` is documented (`#![warn(missing_docs)]`),
  and `flow-sdk-c`'s functions in its header.
- Comments and docs are plain English that say why; each module starts
  with a `//!` summary. Tests are named as sentences.
- A version goes out as a tag, `v<version>`; Flow then moves its
  workspace dependencies (`flow-sdk`, `flow-demos`) to it. Bump the demos'
  own version (`demos/Cargo.toml`) with each change to them.
- Run what draws in a Flow tab: elsewhere the examples and demos say what's
  missing and stop.

## The SDK

`flow-sdk`, at the repository's root, is for programs built
for Flow, depending on none of Flow's crates; the demos here and Flow's games are
built on it, the games' screens of cells (`flow_sdk::screen`) and queues of
effects (`fx::Queue`) too. It keeps every public item documented
(`#![warn(missing_docs)]`), and its and `flow-sdk-c`'s manifests are ready
to publish but for what waits on the owner (README.md's
Publishing). `c/` (`flow-sdk-c`) is the same as a C library (cdylib and staticlib)
whose header, `c/include/flow_sdk.h`, is written by hand and kept in
step with its `#[no_mangle]` functions by `c/tests/header.rs`; what it
returns goes back to `flow_free`, its handles to their own `_free`, and
nothing unwinds out (`guard`). Its `audio` feature (on by default) is the
SDK's: `c/src/audio.rs` exports every function either way (their bodies
behind `with_audio!`), so one header serves both builds, and without it
they refuse, `flow_audio_built` says so and Python's `flow_sdk.audio`
raises `Unavailable`. The scene's vector maths is bound as
`flow_v3_*`, `flow_m3_*` (rotations as nine floats by rows,
`FLOW_M3_IDENTITY`, checked by `c/tests/header.rs`) and the like, and the
screen as `FlowScreen` (`c/src/screen.rs`, Python's `flow_sdk.cells.Screen`,
a module not named `screen`, which is the function that takes the
terminal); the queue of effects isn't, since it would keep the library's
clock. `detect` asks OSC 11 too (`Query::background`, `Caps::background`,
`detect::is_light`; Flow's pipe answers it on Windows), and a
`screen::Screen` lays a program's own ground under cells left without one
(`Screen::ground`; a picture's cell carries a `bg` too, keeping what it
covered). C has `FLOW_ASK_BACKGROUND`, `flow_terminal_background`,
`flow_is_light` and `flow_screen_ground`; Python has
`detect(background=)`, `Caps.background`/`light_background`,
`is_light` and `Screen.ground`. A scene's polygon or a mesh's face takes a
color or a `scene::Material` (gloss, metal, glow, clear coat, unlit; a
color alone is written exactly as before, so the demos' files don't
change), indices go as 16-bit where they fit, and `glb_packed` (a stage
made `.packed(true)`, as Flow Invaders and the demos do in Flow) packs points and
normals into whole numbers (KHR_mesh_quantization) under one uniform node
scale, so normals survive Flow's inverse transpose; `fx::Budget` keeps a
program under Flow's allowance of effects (`flow-demo sounds` uses it). C
has `FlowMaterial` with the `_in` functions, the `_packed` ones and
`FlowBudget`; Python has `Material`, `glb(packed=)`, `Stage.packed` and
`fx.Budget`. Flow's settings (`flow_sdk::settings`) are JSON text in C
(`c/src/settings.rs`: `flow_settings_request`, `flow_settings_answer` and
`flow_settings_ask`) and
dicts in Python's `flow_sdk.settings` (`ask`, `request`, `answer`, and
`get`, `set`, `reset`, `list`, `reset_terminal` and `restart`, which raise
`Refused` with Flow's reason or `NoAnswer`, also for an answer naming
another request, `_answers`). On Unix `settings::ask` asks the program's
terminal itself, not its standard input and output, so piped, redirected
or `$(…)` it still reaches Flow: `Tty` opens it by name (`ttyname_r` of
the first of stdin, stderr and stdout that's a terminal, none of them one
asking nothing; macOS's `poll` can't wait on `/dev/tty`), raw for the wait
where it isn't (`ICANON` and `ECHO` off, put back on drop), throws away the
input waiting there (`tcflush`), where an earlier request's answer, come
too late for its wait, would be taken for this one's, and reads with
`detect::read_answers`, which stops where `poll` says a descriptor can't
be waited on; on Windows it puts DA1
after the request, since the pipe can't be read against a clock, so Flow's
own three seconds bound it. Theme packages for Flow (`flow_sdk::theme`,
behind the `theme` feature, which brings serde and serde_json, so programs
that only draw don't build them) write what Flow's `looks/packaged.rs`
reads, a `.tar.gz` of `themes.json` and its fonts and logos that `flowapp
install` takes: `ThemeFile` and `ThemeEntry` are Flow's own types field
for field (their order, defaults and `skip_serializing_if`s too), checked
by `check` as Flow's `ThemeEntry::look` and `read` check them, and a little
stricter where Flow would take a value and do nothing with it (sounds or a
language it hasn't, a cursor speed it clamps, `keys` outside `KEYS`, Flow's
`THEME_KEYS`, or of the wrong `Kind`, which leaves them all off, a font
that isn't one, a logo that isn't a PNG; a package named `bin` or `first`,
folders of `flowapp`'s own); `Theme` and `Package` build them, `write`
copies the files in (`fonts/`, `logos/`) and `pack` writes into a `Stage`
under the temporary folder and runs the system's tar as Flow's
`examples/theme_package.rs` does, owned by 0 with no names (bsdtar's flags
on macOS and Windows, GNU tar's elsewhere unless `tar --version` says
bsdtar). `tests/fixtures/flow-core-themes.json` is Flow's own package,
written by `cargo run --example theme_package -- --version 0.1.0 --out
<dir>` in Flow, which the tests read back byte for byte: write it again
when Flow's format or its machines change, and the checks with it. The C
library always has it (`c/src/theme.rs`: `FlowTheme` set a field at a
time, names as strings, `FlowThemePackage`, reasons in `char **error`, and
`FLOW_THEME_FAILED`, -2, for an `Error::Io` where a refusal is -1), and
Python's `flow_sdk.theme` raises `Refused` or `OSError` by it. `python/` is a pure-Python ctypes package
over it, finding the library by `FLOW_SDK_LIB`, `CARGO_TARGET_DIR`, its
own folder (where a platform's wheel carries it: `setup.py`) or Cargo's
`target/` above it; an SDK change that C and Python should see goes in
all three.

## The demos

`demos/` (`flow-demos`, one binary `flow-demo` that takes the
demo's name, whose program is `app.rs`, `flow_demos::app::main`) shows off what Flow draws, and like the games uses none of
Flow's crates; it's built on `flow-sdk`, whose `scene`, `stage`, `detect`,
`fx` and `term` began as its own, as `picture` and `audio` began as the
games'. A demo is a `Program` (`lib.rs`): `Staged`, a `Demo` drawn on a
stage above its rows of words, each frame a `Show` (a scene or a picture),
or `Text`, a `TextDemo` that writes the screen's own cells and may place
models of its own among them (`Needs::Models`): its `Written` is the text,
then its Ratty commands (after the text, since a model's cells are Flow's
to write), effects and sounds; `TextDemo::quit` says what goes as it quits.
Each is `Told` as it starts whether it's in Flow and may set the liquid off:
`--no-effects` drops the effects only, while sounds and notes
(`Written::sounds`) and the glass's wash (`Demo::wash`, sent when it
changes, and `UNTINT` on the way out) go in Flow either way. A scene
demo draws each
frame's `Scene` (`flow_sdk::scene`) in the picture's pixels: convex polygons, cut to the picture,
wound counter-clockwise (Flow lights a clockwise triangle as its back), one
glTF part a color or `Material`, and a `Camera` that projects a world in perspective with
a depth of one over the distance. Placements can't overlap, so the whole
scene is one object (`flow_sdk::stage`): its GLB is registered again under the same
id each frame (not when it's unchanged), packed into whole numbers in Flow
(`Stage::packed`, half the size) and plain to other terminals, placed once
over the most cells Flow allows, scaled by 1/0.9 to undo Flow's margin, with four invisible
corners (alpha 0) making the model's box the picture's, and squashed from
the screen's fractional cells (the pty's pixels) to the whole pixels
(`CSI 16 t`) the picture has per cell. Colors meant to show as given on a
face that looks straight out go through `lit_as`, which undoes Flow's light;
`vector.rs` turns its lines' faces toward the light instead, so they can
show at full strength, each line a solid core inside see-through halos
(glTF's `BLEND`, which Flow draws after the solid parts). A picture demo's
frame (`Shows::Pictures`) goes to `pictures::Easel`, which sends it with
kitty's protocol under the same id each frame and writes its placeholder
cells once, the picture before the cells; where cells go bare
(`Caps::bare`) each row is a picture of its own (`Image::strip`), so
`Demo::cells` tells the plasma its rows and it makes each row whole pixels.
A picture sent each frame is kept small (the plasma's is about 40,000
pixels, which Flow scales smoothly), as it's decoded and uploaded each
frame. `teletext.rs` writes forty
columns of DEC's double lines (`ESC # 6`, and `ESC # 3` over `ESC # 4` with
the same cells), so it needs 80 × 24 cells, and writes only the rows that
changed, never past the half of the screen a double line shows, where
Flow keeps the line's mark; its pictures are drawn on `mosaic::Pixels` in
teletext's colors and turned into sextants, a color over a ground color or
over nothing, and its clock is the local time from `clock.rs` (the C
library's `localtime_r`, or kernel32's `GetLocalTime`, which std links
already, so no crate). Effects go out with OSC 7731 in Flow only, naming one
of Flow's sounds (`sound=`) where their kind's isn't the one. `flow_sdk::detect`
asks for the protocol and the cell's pixels before crossterm reads the
keys; on Windows it asks down Flow's pipe for images, where the scenes, the
pictures and the effects go too (`Terminal::send`, `send_effects`), after each
frame's words go through ConPTY, so the scene's cells aren't painted over
(a picture goes before the cells that show it, which the program writes).
Teletext asks for pictures only so that Flow keeps the pipe for its effects
there. The SDK's
`mesh` is for objects the terminal turns itself (faces wound outward, no
corners), as Flow Rogue's treasures are; `scene` for pictures worked out
each frame; both write GLB through `scene::write_glb`. `boing.rs` is
the bouncing ball, `cube.rs` the rolling cube in glass, metal, rock or
jelly (`Stuff`; the see-through ones' triangles kept under 2,048 and not
small, since Flow cuts and sorts them as it loads each frame: `cut`,
`fanned`; stone's lit faces cracked embers, `ember_face`; drawn ten times a
second once it's still, `interval`), `plasma.rs` the plasma,
`teletext.rs` the pages of teletext, `vector.rs` the vector display,
`sounds.rs` the keyboard of Flow's sounds, `tint.rs` the day over the hills
and `balloon.rs` the model from a file; their timing and layout are pure,
with tests. `sounds.rs` draws on `cells::Grid`, written a changed row at a
time by `cells::Screen`; it sends each tune note as DECPS when it's due,
detached (`length`), and keeps Flow's queue of notes (`Queue`: each starts
when the last ends), so a key's note makes the tune skip one rather than
fall behind, and a held key's repeats don't pile up; it keeps under Flow's
allowance for effects and named sounds (`Budget`), sounds before splashes.
`tint.rs` leaves the sky empty, so the glass is the sky, washed per quarter
of an hour from `WASH`, and turns every face toward Flow's light
(`TO_LIGHT`, as `vector.rs` does), since a face turned straight out shows
at most four fifths of a color's brightness. `balloon.rs` builds a
`flow_sdk::mesh`, writes it into `flow-demo-<pid>` under the user's
runtime folder (`XDG_RUNTIME_DIR`) or else the temporary one (`ModelFile`:
a folder made afresh, the user's alone, passing over a name already taken,
since /tmp is shared on Linux; deleted with its folder when dropped), names it with
`path=` once and places it at once (Flow draws a placement whose file isn't
read yet as nothing), then sends `u` each frame; on Don't Allow Flow reads
nothing and answers nothing, so the demo can't tell, and its words say so.
A tab closed under a demo hangs up, which ended it on the spot, file and
all, so `hangup.rs` has SIGHUP and SIGTERM (the C library's `signal`)
set a flag (`hangup::caught`) that `flow-demo`'s loops end on as on q,
and wake a thread of its own (`watch`, through a socket the handler
writes a byte to): crossterm's read spins on a hung-up terminal and
never returns to the loop, and a terminal that has stopped reading holds
the loop's write and std's lock on stdout with it, so after `GRACE` that
thread deletes the files noted with `hangup::delete_at_close`
(`ModelFile` notes its own), hands the terminal back on a thread of its
own if that's done within `HAND_BACK` (`hangup::within`), and ends the
process by the signal itself (`end_by`: `SIG_DFL` and `raise`), so the
shell puts back the tty's modes as for any job a signal ends. On
Windows the console's close (`SetConsoleCtrlHandler`,
`CTRL_CLOSE_EVENT`) deletes them from its handler. As it starts,
`balloon::sweep` deletes what runs killed outright left: a folder named
`flow-demo-<pid>` (or `-1` to `-15`) for a process that's gone
(`hangup::running`: `kill(pid, 0)` saying ESRCH, or `OpenProcess`), not
this one's, not a link, the user's alone (`hangup::mine`) and holding
the model's file alone, and only that file and the folder.
Flow carries `flow-demo` on its tabs' PATH in every release, built in its
own workspace from a tag of this repository by a package of one call
into `flow_demos::app::main`, so the program goes there as it is here.
