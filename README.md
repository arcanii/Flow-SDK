# flow-sdk

What Flow adds over a plain console, for any program that runs in it:
pictures, 3D models, the liquid's effects and sound. Each of these is an
escape sequence the program writes, so the crate is mostly about writing
them as Flow reads them, finding out first whether the terminal reads them
at all, and falling back quietly where it doesn't. It uses none of Flow's
own code. [Flow](https://github.com/arcanii/Flow-releases) is a terminal
for macOS, Windows and Linux with a liquid-glass look and Claude built in.

This is its first version, in Rust, with the same for C and Python (below).
The demos here (`demos/`, `flow-demo`, which Flow carries) and Flow's games,
Flow Rogue and Flow Mines, are built on it; it began as their code, and they
send what they sent before it, byte for byte.

```toml
[dependencies]
flow-sdk = { git = "https://github.com/arcanii/Flow-SDK", tag = "v0.1.0" }
# with sound of the program's own:
flow-sdk = { git = "https://github.com/arcanii/Flow-SDK", tag = "v0.1.0", features = ["audio"] }
```

| Module | What it does |
| --- | --- |
| `detect` | Asks what the terminal can do: `Query`, `Caps`, and `Terminal`, which sends pictures and effects where they go |
| `term` | Raw keys on the alternate screen (with the mouse), handed back as it was; the size in cells and pixels |
| `input` | Keys and the mouse as Flow reports them, through crossterm (re-exported as `flow_sdk::crossterm`) |
| `picture` | Pictures with the kitty graphics protocol, shown by placeholder cells (bare ones on Windows) |
| `model` | 3D models with the Ratty graphics protocol |
| `scene`, `stage` | A whole 3D scene drawn each frame and fitted to the cells, its parts in colors or materials, and the vector maths to work it out |
| `screen` | A screen of cells drawn whole each frame and sent as its difference from the last, with pictures among the letters |
| `fx` | The liquid's effects (OSC 7731), a queue of them waiting for their moment and a budget that keeps under Flow's allowance, Flow's named sounds, DECPS notes, the glass's tint |
| `pipe` | Flow's pipe for images on Windows |
| `audio` | With the `audio` feature: tones, tunes and songs of the program's own, played on the machine |

From C, and from Python through ctypes, most of it is a library of its own:
see [C and Python](#c-and-python).

## Finding out what the terminal can do

A program asks before anything reads its keys (crossterm would take the
answers for keys) and with raw mode on (or the terminal echoes them):

```rust
use flow_sdk::{term, Query};

term::enter(true)?;                       // raw keys, alternate screen, mouse
let mut terminal = Query::new().pictures().models().cell().version().ask();
if terminal.caps.models { /* 3D */ } else if terminal.caps.pictures { /* pictures */ } else { /* letters */ }
```

`Query` writes the questions it was given in one go, DA1 last, and reads the
answers until DA1's, which every terminal sends, so a terminal that knows
none of the others still ends the wait at once. One that answers nothing at
all is given a second (`Query::wait` changes it).

| Question | Escape | Flow's answer | In `Caps` |
| --- | --- | --- | --- |
| kitty's query | `ESC _Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA ESC \` | `ESC _Gi=31;OK ESC \` | `pictures` |
| Ratty's `s` | `ESC _ratty;g;s ESC \` | `ESC _ratty;g;s;v=1;fmt=obj\|glb;path=1;payload=1;chunk=1;… ESC \` | `models` |
| a cell's pixels | `CSI 16 t` | `CSI 6 ; <height> ; <width> t`, whole pixels | `cell` |
| the text area's pixels | `CSI 14 t` | `CSI 4 ; <height> ; <width> t` | `area` |
| XTVERSION | `CSI > 0 q` | `ESC P >\|Flow 0.1.24 ESC \` | `version`, `flow` |
| OSC 11 | `ESC ] 11 ; ? BEL` | `ESC ] 11 ; rgb:rrrr/gggg/bbbb BEL`, Settings' Background | `background` |
| DA1 | `CSI c` | `CSI ? 62 ; 22 c` | (ends the answers) |

A terminal ignores what it doesn't know, so asking is safe anywhere, and
each answer missing means a quiet fallback: no pictures, letters; no
models, none; no cell size, the window's pixels over its cells (which Flow
gives the pty on macOS and Linux, `term::size().cell`) or none; no
background, a program that sets its own colors can't tell light from dark,
and does best to take the background as light.
`Caps::light_background` (`detect::is_light`) says which a background is:
past the middle of the eye's brightness, where dark letters read better
on it than light ones.
`Caps::cell_size` picks the best there is: the text area's size over the
cells (to a fraction of a pixel), the window's pixels, or `CSI 16 t`'s whole
ones.

Flow also names itself to its shells: `TERM_PROGRAM=flow-terminal` (and
`TERM_PROGRAM_VERSION`), which `flow_sdk::in_flow()` reads. It reaches a
program where Flow's answers don't, and `Query::ask` sets `caps.flow` from
it too.

### Windows: Flow's pipe for images

Windows puts a console layer between Flow and the program (ConPTY), which
passes on only what its console keeps: it drops kitty's and Ratty's APCs and
OSC 7731, and neither the questions nor their answers get through. So each
Flow tab on Windows opens a named pipe and names it in `FLOW_IMAGE_PIPE`.
What a program writes there Flow reads as if written to the terminal, but
only pictures, models, effects, sounds (named ones and DECPS notes) and the
questions above, whose answers come back up the pipe, DA1's last. OSC 11
is answered there since Flow 0.1.28 (with OSC 10, 12 and 4, as the
terminal answers them elsewhere); before, `caps.background` stayed none on
Windows.

`Query::ask` on Windows opens the pipe and asks there (with `CSI 14 t`
added, since ConPTY gives no pixels). If Flow says it shows pictures, the
`Terminal` keeps the pipe, `caps.bare` is set, and from then on
`Terminal::send` writes graphics commands down the pipe and waits for DA1's
answer after them, so they're in before the cells that show them, which go
the other way, through the terminal; `Terminal::send_effects` writes effects
there without waiting. In other terminals on Windows nothing is asked and
nothing shows. `Query::without_pipe` keeps a program that doesn't use the
pipe from opening it.

## Pictures (kitty graphics protocol)

Flow speaks the [kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
and keeps every picture as Unicode placeholder cells, so the way that works
best is the one the SDK uses: send a picture once as a *virtual placement*,
then show it with text.

```
ESC _G a=T,U=1,f=<24|32|100>,o=z,s=<width>,v=<height>,i=<id>,p=<placement>,c=<cols>,r=<rows>,q=2,m=<0|1> ; <base64> ESC \
```

`f` is 24 for RGB, 32 for RGBA (straight alpha) and 100 for a PNG; raw
pixels go zlib-compressed (`o=z`) with their size in `s` and `v`; `q=2`
keeps the terminal from answering. A payload longer than 4096 base64 bytes
goes in chunks, the first with all the keys and `m=1`, the rest with only
`m` and `q`, the last `m=0`. Sending an id again replaces its picture.

Then the picture shows wherever the program writes its cells: U+10EEEE,
each in the image id's color (`CSI 38;2;r;g;b m`, the id's low 24 bits)
and underlined in the placement's (`CSI 58;2;r;g;b m`), followed by two
combining marks from kitty's table numbering the cell's row and column in
the picture (and a third with the id's top byte, for ids past 24 bits). So
the picture scrolls, clears and goes into the scrollback as text does, and
moving or animating it is writing cells.

```rust
use flow_sdk::picture::{self, Image, PLACEMENT};

let image = Image::rgba(width, height, pixels).packed(); // RGB if opaque throughout
terminal.send(picture::upload(id, &image, PLACEMENT, cols, rows).as_bytes())?;
for row in 0..rows {
    print!("\x1b[{};{}H{}", top + row + 1, left + 1, picture::placeholder_row(id, PLACEMENT, row, 0, cols));
}
```

Packing a picture (compressing and encoding it) is the slow part, so a
program making many at once can pack them on threads of its own first
(`Image::pack`) and write the commands once their ids are known
(`picture::upload_packed`), as Flow Rogue does with a screen of new tiles.
`upload_as` and `upload_packed` take `Chunks::Short` to give the chunks
after the first `m` alone, the least kitty's spec asks (Flow goes by the
first chunk's `q`). `Ids::after(last)` goes on from an id the program
picked itself.

`picture::delete(id)` forgets a picture (`a=d,d=I`); `DELETE_SHOWN` takes
away what the screen in use shows (`a=d,d=a`), keeping the scrollback's and
the other screen's, as kitty's spec says. Flow keeps up to 320 MB of
pictures a tab, letting go of the oldest (those no longer shown first),
and takes pictures up to 10,000 pixels a side. Pick ids unlikely to clash
with another program's in the same tab: `picture::Ids` starts at a random
place in the 24 bits a color holds.

**Bare cells, on Windows.** ConPTY gives each combining mark a column of its
own and doesn't know the underline color, so through it the cells go bare:
U+10EEEE in the id's color and nothing else (`picture::bare_row`). Flow
takes a bare cell's column from the cell to its left (one more when that
shows the same image) and its row as the first, so each row of a taller
picture has to be an image of its own (`Image::strip`), and a picture right
of its double needs an id of its own. `examples/picture.rs` does both.

A program drawing pictures among letters, as Flow Rogue does, hands
`picture::bare_images` a screen row's `Part`s (which picture, and the
cell's row and column in it) and gets back the image each cell has to
name, or none where a cell can't show its part (something covers the
picture's start) and is best left blank; it asks the program (`Pieces`)
for each row's strip and, the first time one's wanted, a twin. A
[screen of cells](#a-screen-of-cells) does that for a whole screen
(`Screen::bare`).

## A screen of cells

A program that draws its whole screen each frame, as a game does, draws
into a `screen::Screen` and sends only what changed since the screen
before: the cursor moved only where a cell doesn't follow on from the one
before it, the colors and weight set only where they change, a reset
first so they start from the terminal's own. A cell is a letter in its
colors (`Cell::Text`, its background none for the terminal's own, which
in Flow is the glass and the liquid) or a piece of a picture
(`Cell::Image`: which picture, and the cell's row and column in it,
written as its placeholder cell, on the background of the cell it covered),
so pictures go among the letters, give way to a panel drawn over them and
move as letters do. Writing off the screen's edges does nothing.

A program that wants no cell left to the terminal's background, so a light
one can't wash out letters meant for a dark ground, lays its own:
`Screen::ground` puts a color under every cell of an area that has no
background of its own, blank cells, letters and pictures' cells alike.
Laid over part of the screen first (a map, say) and over the whole after,
it gives each part its ground. A picture's cell sends its background too,
which shows through the picture's clear pixels in terminals that draw a
cell's background under its picture, as kitty does; Flow draws a picture
over its glass alone, so a program paints its ground into the picture
itself as well (Flow lays a cell's background at 88% over the glass).

```rust
use flow_sdk::screen::Screen;

let mut last: Option<Screen> = None;
loop {
    let size = term::size()?;
    let mut screen = Screen::new(size.cols, size.rows);
    screen.panel(0, 0, 30, 5, [90, 104, 130], None);  // a frame, rounded
    screen.bold(2, 1, "Flow", [110, 194, 202]);
    screen.image(2, 2, 4, 2, id);                      // picture `id`, sent first at PLACEMENT over 4 × 2 cells
    screen.bar(8, 3, 20, 0.6, [140, 222, 120]);       // to an eighth of a cell
    screen.ground(0, 0, size.cols as i32, size.rows as i32, [12, 14, 20]); // nothing left to the terminal's background
    if terminal.caps.bare {
        screen.bare(&mut pieces);                      // each row's strip, a twin where it's needed
    }
    stdout.write_all(screen.diff(last.as_ref(), terminal.caps.bare).as_bytes())?;
    last = Some(screen);                               // None after a resize draws it all again
}
```

A screen of another size than the last, or none, is drawn afresh after a
clear. `Screen::diff_as` takes a `Spelling` for programs that send the
difference otherwise: `whole_styles` sets each change of colors whole from
a reset (`ESC [0;1;38;2;…m`) and resets them after the last cell, leaving
the terminal's colors as they were, and `blanks` writes a fresh screen's
blank cells too. Flow Mines spells its field that way, Flow Rogue the
default; each sends what it sent before the SDK had it.

## 3D models (Ratty graphics protocol)

Flow speaks the [Ratty graphics protocol](https://docs.rs/crate/ratty/0.4.1/source/protocols/graphics.md)
as Ratty, which began it, does, so a program written for one works in both:
`ESC _ ratty;g;<verb>;<key=value>… ESC \`, rows and columns counted on the
screen from 0.

| Verb | Escape | `model::` |
| --- | --- | --- |
| support | `ESC _ratty;g;s ESC \` | `QUERY` |
| register | `ESC _ratty;g;r;id=<id>;fmt=<obj\|glb>;source=payload;more=<0\|1>;name=<file>;<base64> ESC \` | `register`, `register_chunks` |
| register a file | `ESC _ratty;g;r;id=<id>;fmt=<obj\|glb>;path=<path> ESC \` | `register_path` |
| place | `ESC _ratty;g;p;id=<id>;row=<row>;col=<col>;w=<cols>;h=<rows>[;<look>] ESC \` | `Place` |
| update | `ESC _ratty;g;u;id=<id>;<look> ESC \` | `update` |
| delete | `ESC _ratty;g;d[;id=<id>] ESC \` | `delete`, `DELETE_ALL` |

A placement is centered on its cell (`Place::over` finds the middle cell of
a block, or the one past the middle) and fitted to `w` × `h` cells by its
box, with a tenth's margin. How it looks (`model::Look`): `animate=1` spins,
tumbles and bobs it; `rx`, `ry`, `rz` turn it (degrees); `scale` and `sx`,
`sy`, `sz` size it; `px`, `py` move it (pixels, y up); `color` tints an OBJ;
`brightness` lightens or darkens it; `depth` gives flat artwork thickness.
Placing an id again moves it. A file named by a path makes Flow ask the user
first, naming the tab and the file. Flow takes models up to 64 MB and 2
million triangles; a tab keeps 256 and shows 64 at once, each up to 200
cells and 4096 pixels a side, about 8 million pixels all told.

### A scene each frame

Placements can't overlap, since a cell shows one picture, so a scene of
several things moving about is one model, registered again under the same
id each frame: Flow draws the new one in the old one's place. `scene` and
`stage` do that, as `flow-demo` does:

```rust
use flow_sdk::scene::{rgb, Camera, Scene};
use flow_sdk::stage::Stage;

let mut stage = Stage::new(term::size()?, terminal.caps.cell, 1).expect("a big enough window");
loop {
    let mut scene = Scene::new(stage.width(), stage.height());
    let cam = Camera::orbit([0.0; 3], yaw, 0.3, 6.0, 0.7, stage.height());
    cam.face(&mut scene, rgb(0x7ce7ff), &square, normal); // and so on
    let mut bytes = Vec::new();
    stage.show(&scene, &mut bytes);                      // nothing if it's the last frame again
    stdout.write_all(&bytes)?;
}
```

What makes the model's picture the scene exactly:

- A `Scene` is in the picture's pixels, from its middle, y up, z only
  ordering what's in front. Polygons are convex, cut to the picture (what
  lay outside would grow the box), and wound counter-clockwise as the screen
  shows them: Flow lights a triangle wound the other way as its back.
- Each GLB carries four invisible corners (alpha 0, which Flow discards) at
  the picture's corners, so the model's box is the picture's.
- The stage places it with `scale=1.1111112` (1/0.9, `model::FILL`), which
  undoes the tenth's margin.
- The picture Flow draws has a cell's whole pixels (`CSI 16 t`) per cell,
  while the screen shows a cell a fraction of a pixel wider, so the scene is
  squashed by one over the other.
- The stage keeps inside Flow's limits (200 cells and 4096 pixels a side,
  8 million pixels) and centers itself above the rows a program keeps for
  words. `Stage::point_at` and `Stage::cell_at` map clicks and effects
  between the screen's cells and the picture.
- A `Camera` projects a world in perspective with a depth of one over the
  distance, which stays true when the terminal interpolates it across a
  triangle; `scene::lit_as` gives a face the color that shows as wanted once
  Flow's light (from above left) falls on it.

A polygon (or a mesh's face) is in a color, matte and lit, or in a
`scene::Material`, which Flow draws as glTF's materials say:

```rust
use flow_sdk::scene::{rgb, Material};

let hull = Material::plain(rgb(0x0c0c10)).gloss(0.9).metal(0.4).coat(0.8, 0.95); // black and shining
let lamp = Material::plain(rgb(0x301000)).glow([1.0, 0.45, 0.1], 2.0);             // light of its own
let beam = Material::plain([1.0, 0.2, 0.2, 0.5]).unlit();                           // its color alone
scene.flat(hull, &points, normal);
```

`gloss` is one less glTF's roughness (0, matte, is glTF's own default),
`metal` its metallic factor, `glow` its emission as shown (with
`KHR_materials_emissive_strength` past 1), `coat` a clear coat
(`KHR_materials_clearcoat`) and `unlit` `KHR_materials_unlit`. A color
alone is written exactly as before materials, so a scene of colors is the
same file. Indices go as 16-bit numbers wherever a part's vertices allow.
`Scene::glb_packed`, `Mesh::glb_packed` and a stage made `.packed(true)`
pack the points into 16-bit and the normals into 8-bit whole numbers
(`KHR_mesh_quantization`, which the file then requires), half the size,
which matters for a scene sent thirty times a second; Flow reads them as
of 0.1.27, but not every terminal does, so send them packed in Flow
(`Caps::flow`). The node scales every axis alike, so the normals keep
their way.

## The liquid and sound

A program sets Flow's liquid off at a cell of its own:

```
ESC ] 7731 ; fx=<kind> ; at=<col>,<row> [; to=<col>,<row>] [; color=<rrggbb>]
    [; dir=<dx>,<dy>] [; sound=<name>|none] [; volume=<0..1>] [; size=<columns>] [; strength=<0..1>] BEL
```

| Kind | What it does | Needs | Size unless given | Its sound |
| --- | --- | --- | --- | --- |
| `bolt` | lightning from `at` to `to` | `to` | 1 | thunder |
| `burst` | a splash, in `color` where the liquid takes color | | 3 | splash |
| `ripple` | a ring spreading | | 2 | drop |
| `stir` | the liquid pushed along `dir` (y down) | `dir` | 4 | whoosh |

`size` (0.5 to 40 columns) is how far it reaches and `strength` how hard,
0.5 unless given; `volume` turns its sound down without weakening it, for a
program's own volume for its effects (`Effect::volume`). Flow ignores keys it doesn't know and requests of an
unknown kind or without what their kind needs, and another terminal ignores
the whole escape, but send them only in Flow to be tidy (`in_flow()`). Flow
takes them from the tab in front only, about six a second after a burst of
eight, and only at cells on the screen shown (an effect asked for on the
alternate screen shows while that screen does). With Flow's sound on, each
also sounds as its kind does, or as `sound=` says.

```rust
use flow_sdk::fx::{Effect, Sound};

terminal.send_effects(Effect::burst((12, 3)).color(0xff8000).size(2.5).sound(Sound::Chime).escape().as_bytes())?;
terminal.send_effects(Effect::bolt((0, 0), (40, 10)).escape().as_bytes())?;
```

Over a stage, `stage::Fx` is the same in the picture's pixels, and
`Stage::effect` puts it in cells. `Effect::escape_as` writes the numbers
Flow reads either way as a program likes them (`fx::Spelling`): `dir` in
whole cells (`dir=1,-1`) and `volume` in tenths, as Flow Rogue does.

A program that sets off more than the allowance loses what goes over it
unseen. An `fx::Budget` spends a little under it (seven at once, five a
second after) on the program's own clock; `Budget::take(now, keep)`
spends one only if `keep` would be left for what matters more, so a hit's
lightning isn't lost to a wall's chips:

```rust
use flow_sdk::fx::Budget;

let mut budget = Budget::new();
if budget.take(t, 2.0) {
    terminal.send_effects(chip.escape().as_bytes())?; // only while two are left for hits
}
```

Effects that come a beat after what set them off (a blast's lightning
running out after the blast, a spell's splash as it lands) wait in an
`fx::Queue` for their moment. Each frame takes those due, in the order
they were added, to send after the screen that shows what set them off;
one that waited a second past its moment (behind a menu, say) is let go
(`Queue::late` changes how long), `Queue::next` says how long a program
waiting for keys may wait, and `Queue::shift` moves what's waiting when
what set it off moved.

```rust
use flow_sdk::fx::{Effect, Queue, Timed};

let mut queue: Queue = Queue::new();
queue.add(Timed::new(now, Effect::burst(mine).color(0xff5a1a).size(12.0)));
queue.add(Timed::new(now + Duration::from_millis(120), Effect::bolt(mine, near)));
// each frame, after the screen:
terminal.send_effects(queue.escapes(Instant::now()).as_bytes())?;
```

What waits can be a program's own (anything that's `fx::Due`), made into
effects as it goes out: Flow Rogue's wait on tiles of its map and are put
in cells where the map is drawn then (`Queue::take_due`).

Flow's sounds by name are `drop`, `splash`, `thunder`, `whoosh`, `chime`,
`low`, `ding`, `thud`, `click`, `boing` and `bell` (`fx::Sound`). One plays
alone, placed left or right by `at`'s column, and counting toward the
effects' allowance:

```
ESC ] 7731 ; fx=sound ; sound=<name> [; at=<col>,<row>] [; strength=<0..1>] BEL
```

A named sound takes `volume=` too, as an effect does
(`Sound::escape_with`).

Notes play as DECPS has them, as the VT520 did (Windows Terminal plays
them too):

```
ESC [ <volume> ; <length> ; <note> [; <note> …] , ~
```

with the volume 0 (silent) to 7, the length in 32nds of a second, and each
note from 1 (C5) up a semitone at a time to 25 (C7), 0 a rest. `fx::notes`
writes it, `fx::note("E5")` names a note. Notes follow one another, up to
ten seconds ahead. Flow plays all of these only while Sound is on in its
settings.

A program that holds the alternate screen can also wash the glass in a
color, as Flow Rogue's dusk and night do (and `flow-demo tint`'s day):

```
ESC ] 7731 ; fx=tint ; color=<rrggbb> ; strength=<0..1> BEL
```

The glass shifts toward the color and the liquid dims as far as `strength`
says, Flow easing into it; `strength=0` (`fx::UNTINT`) washes it off,
and so does leaving the alternate screen. `fx::tint` writes it; send it
when it changes, not each frame.

## Sound of the program's own

With the `audio` feature, `audio::Audio` plays tones and tunes made in code
on the machine, in any terminal or none: a few waves (sine, triangle,
square, saw, noise), an attack and a release, a pan, and tunes written as
`name/beats` (`"C4/1 E4/1 G4/2 r/1"`) at a tempo, once or looped. A thread
of its own opens the system's output (cpal: CoreAudio on macOS, WASAPI on
Windows; on Linux ALSA's default device, through ALSA's library loaded as
the program runs, so building needs no headers and a system without it is
silent) with the first sound and closes it after three seconds of silence;
`audio::Mixer` is the mix itself, rendered in the output's callback (on
Linux, a thread of the output's own), and runs in tests without an
output. Keep it for music and the like: Flow's own sounds,
above, follow the user's settings for sound in Flow.

A `Clip` is sound the program has made itself, sample by sample, at a
rate of its own (speech it synthesizes, say): `Audio::play_clip` plays it
once alongside the rest, the mix taking it to the output's rate. Its
samples are shared (`Arc<[f32]>`), so keep a clip to play it again; Flow
Invaders' radio voice is played this way. It isn't in the C library or
the Python package yet.

For music under a game, a `Song` is several lines played together and
looped, each on an `Instrument` made in code: a plucked lute (Karplus and
Strong's string), a breathy recorder, a drone of a note and its fifth, and
a tabor whose notes are `X` (a full beat) and `x` (a light one). Each
line's notes are written as a tune's are, all lines as many beats long.
`Audio::play_song` fades the new song in over a second as the one before
fades out, and `stop_song` fades it out; Flow Rogue's music is its tunes
played this way.

```rust
use flow_sdk::audio::{Audio, Instrument, Line, Song};

let dance = Song::new(330.0, vec![
    Line::parse(Instrument::Lute, 1.0, "A4/3 G4/1 F4/2 D5/2")?,
    Line::parse(Instrument::Drone, 1.0, "D3/8")?,
    Line::parse(Instrument::Tabor, 0.7, "X/3 x/2 x/1 r/2")?,
]);
let mut audio = Audio::new();
audio.play_song(dance);
```

From C and Python these are `FlowSong` and `audio.Song`, and the rest
alike: see [C and Python](#c-and-python).

## Keys and the mouse

`term::enter(true)` turns raw mode, the alternate screen and the mouse on
(clicks, drags and moves as SGR reports, which Flow sends; holding Shift
keeps the mouse Flow's own, for selecting), and `term::leave` undoes it,
which `term::leave_on_panic` makes a panic do first. `input::next_before`
waits for the next key (not the releases Windows also reports), click or
resize until a deadline, `input::is_quit` knows q, Esc and Ctrl+C, and
`input::left_click` says where a click went down. Anything more is
crossterm's own API, re-exported as `flow_sdk::crossterm`.

## Flow's settings

`settings::ask` asks Flow about its settings, or to change one, with OSC
7732, as `flow-settings` does: a JSON request such as
`{"do": "set", "key": "fx.style", "value": "rain"}` (or `list`, `get`,
`reset`, `reset-terminal`, `restart`), answered with JSON, `{"ok": true,
…}` or `{"ok": false, "error": …}` in Flow's language. On Windows it goes
down Flow's pipe, where Flow's own wait of three seconds or so bounds it;
elsewhere it goes to the program's terminal itself, opened by name, so it
reaches Flow however standard input and output are piped, redirected or
read by `$(…)`, and nothing is asked where none of the three standard
streams is a terminal. The answer comes in as keys, so it takes raw mode
on that terminal for the wait where it isn't on already, and nothing else
should be reading the keys meanwhile; what waits in the input is thrown
away before asking, so an earlier request's answer, come too late for its
wait, isn't taken for this one's. Flow decides what a program may change, and
says each change in a toast; the module's documentation has the whole
request. C and Python have it too (see [C and Python](#c-and-python)).

## Examples

```bash
cargo run -p flow-sdk --example picture                  # a picture below the prompt
cargo run -p flow-sdk --example scene                    # a cube turning over the window; click ripples
cargo run -p flow-sdk --example effect                   # effects and sounds round a word
cargo run -p flow-sdk --features audio --example tune    # a tune of the program's own
```

Run them in a Flow tab: elsewhere they say what's missing and stop.

## The demos

`demos/` holds `flow-demo`, which shows off what Flow draws, a program on
the SDK as yours would be. Flow carries it on its tabs' PATH; from here:

```bash
cargo run --release -p flow-demos -- boing   # or cube, plasma, teletext, vector, sounds, tint, model
cargo run --release -p flow-demos            # lists them
```

| Demo | What it is |
| --- | --- |
| `boing` | A checkered ball bouncing before a grid, after the Amiga's Boing demo: a 3D scene sent each frame. |
| `cube` | A small cube rolling over a big one of rounded blocks until an F lights, in glass, metal, stone or jelly (`m`, or 1 to 4). |
| `plasma` | The demoscene's plasma, made each frame and sent as a picture, its dark bands see-through. |
| `teletext` | Pages of teletext in DEC's double lines and its mosaics, with a clock and a boat; it needs 80 × 24 cells. |
| `vector` | Glowing lines as vector monitors drew them, at their best under Flow's CRT. |
| `sounds` | A keyboard of DECPS's notes and Flow's named sounds, playing a tune. |
| `tint` | A day over the hills, the sky Flow's glass washed in its color (`fx=tint`). |
| `model` | A balloon Flow reads from a file, after asking you first. |

Each sets Flow's liquid moving where things happen; `--no-effects` leaves it
be, and `q` quits. They're licensed as the SDK is.

## C and Python

`c/` (the `flow-sdk-c` crate) is the SDK as a C library, and
`python/` a Python package over it, pure Python through ctypes with
nothing to compile: the questions and what the terminal answered,
pictures, models, meshes, scenes with their camera, their vector maths
and the stage that sends them each frame, screens of cells sent as their
difference from the last, effects, Flow's sounds, DECPS notes and the
tint, Flow's settings seen and changed, raw keys on the alternate screen,
the terminal's size and its next key, click or resize, and the program's
own audio: tones, tunes and songs, a player and the mixer. The queue of
effects waiting for their moment isn't bound: in C or Python it's a few
lines over the program's own clock (a list of effects and when each is
due), and bound it would have the program keep time by the library's
clock instead of its own.

```bash
cargo build --release -p flow-sdk-c
```

leaves the shared library in `target/release` (`libflow_sdk_c.dylib` on
macOS, `libflow_sdk_c.so` on Linux, `flow_sdk_c.dll` on Windows) and a
static one beside it (`libflow_sdk_c.a`, or `flow_sdk_c.lib` on Windows). On macOS the shared library is
named `@rpath/libflow_sdk_c.dylib`, so a program linked against it finds it
by its rpath wherever it's copied.

**Audio is a feature of the library, on by default.** `flow-sdk-c` has an
`audio` feature, which turns on the SDK's, and `cargo build -p flow-sdk-c`
builds it; `--no-default-features` leaves it out. It's on because the
library is how a program in C or Python gets the SDK at all, and it costs
little: on macOS and Windows it needs nothing beyond Rust (cpal speaks to
CoreAudio and WASAPI through the system's own libraries), on Linux it
loads ALSA's library as it plays, so it needs no ALSA headers, and nothing
starts until a `FlowAudio` is asked to play (no thread, no output opened).
Left out, the library is smaller and links no sound system. Its audio
functions are there either way, so one header and one `FLOW_SDK_ABI`
serve both builds: built without it, each refuses (NULL, -1, 0 or false)
and `flow_audio_built()` says so. A program linking the static library with
audio on macOS also links `-framework AudioToolbox -framework CoreAudio
-framework Foundation -framework CoreFoundation -lobjc`;
`cargo rustc -p flow-sdk-c --lib -- --print native-static-libs` lists what
it needs on each OS, with or without the feature.

### From C

The header is `c/include/flow_sdk.h`, written by hand: a test checks
that it declares every function the crate exports and no other, that its
`#define`s are the crate's numbers, and (where there's a `cc`) that its
structs are laid out as the crate's. Everything is named `flow_…`,
`Flow…` or `FLOW_…`, and `flow_abi_version()` gives the interface's
version, which has to be the header's `FLOW_SDK_ABI`. Who owns what:

- A `char *` or `uint8_t *` a function returns is the caller's, and goes
  back to `flow_free` once used. Strings are NUL-terminated escapes, ready
  to write; a buffer's length comes back through a `size_t *`. NULL means
  the arguments were refused (a NULL handle, a format there's none of,
  pixels that don't fit their size).
- Handles (`FlowTerminal`, what the terminal said; `FlowMesh`;
  `FlowEffect` and `FlowBudget`; `FlowScene`, `FlowCamera` and `FlowStage`; `FlowScreen`;
  `FlowTune`, `FlowSong`, `FlowAudio` and `FlowMixer`) go back to their own
  `_free`, never to `flow_free`, and each `_free` takes NULL.
- A function that reads notes (`flow_tune_new`, `flow_song_line`) takes a
  `char **error` too: NULL for none wanted, or where to put the reason it
  refused (`"A4: a note needs its length"`), to free with `flow_free`.
- Pointers passed in are read during the call only and never kept; one
  with a length may be NULL when the length is 0.
- An `int` comes back 0 for done and -1 for refused. Nothing unwinds out of
  the library: a panic inside comes back as NULL or -1.
- The library writes standard output through its own buffer and flushes
  it each time, so `fflush(stdout)` before it when `printf`'s text has to
  come first.

Outside a terminal (input or output redirected, as in a test) the functions
that need one do nothing and return -1, and `flow_detect` asks nothing on
macOS and Linux, where reading the answers would take the program's own
input: a program falls back quietly. Asked with `FLOW_ASK_BACKGROUND`,
`flow_terminal_background` gives the background OSC 11 told as 0xRRGGBB
(-1 if none), and `flow_is_light` judges it as the SDK does.

```c
#include <string.h>
#include "flow_sdk.h"

flow_term_enter(false);                         /* raw keys on the alternate screen */
FlowTerminal *terminal = flow_detect(FLOW_ASK_MODELS | FLOW_ASK_VERSION, 0);
FlowCaps caps = {0};
flow_terminal_caps(terminal, &caps);
if (caps.flow) {
    FlowEffect *ripple = flow_effect_new(FLOW_FX_RIPPLE, 40, 12);
    flow_effect_sound(ripple, FLOW_SOUND_CHIME);
    char *escape = flow_effect_escape(ripple);
    flow_terminal_send_effects(terminal, escape, strlen(escape));
    flow_free(escape);
    flow_effect_free(ripple);
}
flow_term_leave();
flow_terminal_free(terminal);
```

A scene from C is a `FlowScene` built each frame, through a
`FlowCamera` (`flow_camera_orbit`, made anew if it moves), and a
`FlowStage` that sends it; a point of the picture is two floats, x and y
from its middle, and a stage's effects come back as `FlowEffect`s, their
color and sound set as any effect's. The scene's vector maths is there
too, each result written through an `out` pointer that may be one of the
arguments: `flow_v3_add`, `_sub`, `_mul`, `_dot`, `_cross`, `_normalize`
and `_lerp` on three floats, `flow_m3_rotation`, `_apply` and `_compose`
on rotations of nine (by rows, `FLOW_M3_IDENTITY` none), `flow_smooth`,
and `flow_rgb`, `flow_mix` and `flow_shade` for colors. A polygon or a
face in a material takes a `FlowMaterial` (`flow_material_init` makes one
a color alone; set its `gloss`, `metal`, `glow`, `coat` or `unlit` after)
through the functions ending `_in` (`flow_scene_flat_in`,
`flow_scene_polygon_in`, `flow_camera_face_in`, `flow_mesh_face_in`), and
`flow_stage_packed`, `flow_scene_glb_packed` and `flow_mesh_glb_packed`
pack the file for Flow. `FlowBudget` (`flow_budget_new`, `_take`,
`_left`) keeps a program under Flow's allowance of effects:

```c
FlowSize size;
flow_term_size(&size);
FlowStage *stage = flow_stage_new(&size, caps.cell_width, caps.cell_height, 1); /* NULL if too small */
FlowStageLayout layout;
flow_stage_layout(stage, &layout);
FlowScene *scene = flow_scene_new(layout.width, layout.height);      /* each frame */
const float target[3] = {0, 0, 0}, up[3] = {0, 1, 0};
FlowCamera *camera = flow_camera_orbit(target, 0.0f, 0.35f, 6.0f, 0.7f, layout.height);
float turn[9];
flow_m3_rotation(up, t * 0.8f, turn);                                /* the cube turns, not the camera */
flow_m3_apply(turn, normal, normal);                                 /* each corner and each face's way */
flow_camera_face(camera, scene, color, square, 4, normal);           /* and so on */
char *frame = flow_stage_show(stage, scene);                         /* "" if as before */
flow_terminal_send(terminal, frame, strlen(frame));
flow_free(frame);
flow_camera_free(camera);
flow_scene_free(scene);
```

A screen of cells from C is a `FlowScreen`, which keeps the screen being
drawn and the one shown last: `flow_screen_text`, `flow_screen_image` (a
picture sent first, at `FLOW_PLACEMENT`) and `flow_screen_clear` draw into
it, colors as 0xRRGGBB and a background that's negative the terminal's
own, `flow_screen_ground` lays a ground under every cell left without one,
and `flow_screen_show` hands back only what changed since the last
show, all of it the first time and after `flow_screen_forget`:

```c
FlowScreen *screen = flow_screen_new(size.cols, size.rows);          /* anew after a resize */
flow_screen_clear(screen, 0, 0, size.cols, size.rows, -1);           /* each frame */
flow_screen_text(screen, 2, 1, "hp 12", 0xf04040, -1, true);
flow_write_free(flow_screen_show(screen, caps.bare));
```

Flow's settings from C are JSON text both ways, the program's to write
and to read: `flow_settings_ask` writes the request and waits for the
answer (three seconds for a wait of 0, as `flow-settings` waits), and
gives it as JSON, `{"ok": false, …}` too, or NULL when nothing answered.
On macOS and Linux it asks the program's terminal itself, whatever
standard input and output are, and the answer comes in as keys, so it
turns raw mode on for the wait where it isn't on already and back off
after, and a program that took the terminal (`flow_term_enter`) keeps it
as it was; where none of standard input, output and error is a terminal
it asks nothing and gives NULL at once. Another terminal says
nothing, so ask only in Flow, or the whole wait passes. Don't cut the wait
short either: an answer that comes after it lands in the program's input,
echoed while the terminal isn't raw, and though the next ask throws away
what waits there before asking, one late enough could still be taken for
the next answer, so check that an answer to `get`, `set` or `reset` names
the key asked about. On Windows the question goes down Flow's pipe, which
can't be read against a clock: Flow's own wait, three seconds or so,
bounds it there, not the one given. For a program that reads its own
input, `flow_settings_request` gives the escape and `flow_settings_answer`
reads the answer out of what came in:

```c
if (flow_in_flow()) {
    char *answer = flow_settings_ask("{\"do\":\"get\",\"key\":\"fx.style\"}", 0);
    if (answer != NULL) {
        printf("%s\n", answer);                   /* {"ok":true,"key":"fx.style","value":"rain"} */
        flow_free(answer);
    }
}
```

A song is lines of notes on the instruments, played by a `FlowAudio` (the
machine's output) or rendered by a `FlowMixer` into the program's own
buffers:

```c
FlowSong *song = flow_song_new(150.0f);
char *error = NULL;
if (flow_song_line(song, FLOW_INSTRUMENT_LUTE, 1.0f, "D4/1 F4/1 A4/2 G4/1 F4/1 E4/2", &error) != 0) {
    fprintf(stderr, "%s\n", error);
    flow_free(error);
}
flow_song_line(song, FLOW_INSTRUMENT_TABOR, 0.7f, "X/2 x/1 x/1 X/2 x/2", NULL);
FlowAudio *audio = flow_audio_new();      /* NULL in a build without audio */
flow_audio_play_song(audio, song);        /* fades in, the one before out */
```

`c/examples/hello.c` builds a gem as a mesh, places it turning in the
middle of the screen and ripples the liquid round it; `scene.c` draws a
cube turning about two axes each frame, through a camera that stays put,
on a stage, to a dance on the lute, drone and tabor, and ripples the
liquid where it's clicked; `settings.c` asks Flow its liquid style and
text size, and given a style (`./settings rain`) puts it on:

```bash
cargo build -p flow-sdk-c
cc c/examples/hello.c -I c/include -L target/debug -lflow_sdk_c -Wl,-rpath,target/debug -o hello
./hello
cc c/examples/scene.c -I c/include -L target/debug -lflow_sdk_c -Wl,-rpath,target/debug -o scene
./scene
cc c/examples/settings.c -I c/include -L target/debug -lflow_sdk_c -Wl,-rpath,target/debug -o settings
./settings
```

### From Python

```python
import flow_sdk as flow
from flow_sdk import model

with flow.screen():                             # raw keys on the alternate screen
    terminal = flow.detect(models=True, version=True)
    size = flow.size()
    if terminal.caps.models:
        gem = flow.Mesh()
        for a, b in [((1, 0, 0), (0, 0, 1)), ((0, 0, 1), (-1, 0, 0)), ((-1, 0, 0), (0, 0, -1)), ((0, 0, -1), (1, 0, 0))]:
            gem.triangle(0xFACC4C, a, b, (0, 1, 0))
            gem.triangle(0xD9731A, b, a, (0, -1, 0))
        terminal.send(model.register(1, gem.glb(), "glb"))
        terminal.send(model.place(1, size.rows // 2, size.cols // 2, 16, 8, flow.Look(animate=True)))
    if terminal.caps.flow:
        terminal.send_effects(flow.Effect.ripple((size.cols // 2, size.rows // 2)).size(10).sound("chime").escape())
    flow.next_event(timeout=4)
```

`flow_sdk` has `detect`, `screen`, `raw_mode`, `size`, `next_event`,
`write`, `is_light` and `lit_as`, the classes `Terminal`, `Mesh`, `Scene`, `Camera`,
`Material`, `Stage`, `Screen`, `Effect`, `Budget`, `Look` and `Image`, and the modules `picture`,
`model` and `fx` for the escapes, which come back as `str` (a mesh's or a
scene's GLB as `bytes`), `scene` with the vector maths (`add`, `sub`,
`mul`, `dot`, `cross`, `normalize`, `lerp`, `smooth`, `rotation`, `apply`,
`compose`, `IDENTITY`, `mix`, `shade`), `settings` for Flow's settings,
and `audio`; a value the library refuses raises
`ValueError`, saying why where it can (notes that don't read). The
handles free themselves when collected, or at once with `close()` or as a
`with` block's context. `picture.show` sends a picture and writes its
cells, bare ones on Windows too.

```python
from flow_sdk.scene import apply, compose, rotation

stage = flow.Stage.fit(flow.size(), terminal.caps.cell, words=1)   # None if too small
with flow.Scene(stage.width, stage.height) as scene:                # each frame
    turn = compose(rotation((0, 1, 0), t * 0.8), rotation((1, 0, 0), t * 0.5))
    with flow.Camera((0, 0, 0), 0.0, 0.35, 6.0, 0.7, stage.height) as camera:
        normal = apply(turn, (0, 0, 1))                             # and each corner
        camera.face(scene, flow.lit_as(0x7CE7FF, camera.turn(normal)), square, normal)
    terminal.send(stage.show(scene))
at = stage.point_at(click.col, click.row)                           # None off the picture
terminal.send_effects(stage.effect("ripple", at, 160, 0.6).sound("drop").escape())

cells = flow.Screen(size.cols, size.rows)                           # anew after a resize
cells.clear()                                                       # each frame
cells.text(2, 1, "hp 12", 0xF04040, bold=True)
flow.write(cells.show(bare=terminal.caps.bare))                     # what changed

from flow_sdk import audio

dance = audio.Song(150, [audio.Line("lute", "D4/1 F4/1 A4/2 G4/1 F4/1 E4/2"), audio.Line("tabor", "X/2 x/1 x/1 X/2 x/2", gain=0.7)])
player = audio.Audio()
player.play_song(dance)
samples = audio.Mixer(48000).fill(480)                              # or mixed into a buffer
```

`flow_sdk.settings` takes and gives dicts: `settings.ask(request)` gives
Flow's answer as it came, ok or not, or None when nothing answered, and
`request` and `answer` are the escape and the answer read out of what
came in, for a program that reads its own input. `get(key)`,
`set(key, value)` and `reset(key)` (`"all"` for every one) give the value
it is now, `list()` every setting a program may see as a dict, and
`reset_terminal()` and `restart()` nothing; each raises
`settings.Refused`, whose message is Flow's reason in Flow's language, for
an answer that isn't ok, and `settings.NoAnswer` when nothing answered
within `wait` (three seconds unless given) or when what came names
another request (an earlier one's, too late for its own wait), both a
`settings.SettingsError`. As in C, the terminal itself is asked whatever
standard input and output are, raw for the wait where it isn't, nothing is
asked where none of the three standard streams is a terminal, another
terminal lets the whole wait pass, and on Windows Flow's own wait bounds
it:

```python
from flow_sdk import settings

if flow.in_flow():
    try:
        settings.set("fx.style", "rain")        # what it is now; Flow says so in a toast
        size = settings.get("font.size")
    except settings.NoAnswer:
        pass                                    # a Flow from before 0.1.30
    except settings.Refused as e:
        print(e)                                # why, in Flow's language
```

From a library built without audio, `audio.available()` is False and
making a `Song`, `Tune`, `Mixer` or `Audio` (or playing a `Tone`) raises
`audio.Unavailable` (an `OSError`), which says how to build it with audio,
so a program can catch it and go on without music; `audio.plays()` is
whether anything plays here at all. It needs Python 3.8
and the library, which it looks for:

1. at `FLOW_SDK_LIB`, the library's path or a folder holding it (and
   nowhere else once that's set);
2. in Cargo's `target/release`, then `target/debug`, under
   `CARGO_TARGET_DIR` if that's set;
3. in the package's own folder, where a wheel built for one platform
   carries it;
4. in `target/release`, then `target/debug`, beside each folder above the
   package: in this repo, the workspace's;
5. where the system keeps libraries.

It loads the library on first use, not on import, and refuses one whose
`flow_abi_version()` isn't the version it was written for. Run Python with
`-I`, which keeps the current folder and `PYTHON…` variables out:

```bash
python3 -I python/examples/hello.py                      # the gem, as hello.c shows it
python3 -I python/examples/scene.py                      # the cube turning to its dance, as scene.c has them
python3 -I -B -m unittest discover -s python/tests       # its tests, which cargo test -p flow-sdk-c runs too
```

The package installs with pip (`python/pyproject.toml`; on PyPI it would
be `flow-terminal-sdk`, as `flow-sdk` there is someone else's); installed, it
finds the library by `FLOW_SDK_LIB`, in its own folder, or on the system's
paths. A wheel can carry the library: copy the one built for a platform into
`python/flow_sdk/` before building it, and `setup.py` marks the wheel
for that platform alone (`py3-none-<platform>`, since ctypes asks nothing
of the Python it runs in); without one, the wheel is pure, as before.

## Publishing

None of it is published yet. What crates.io and PyPI ask that needs no
one's decision is done:

- `flow-sdk` and `flow-sdk-c` have their `readme`, `keywords`,
  `categories`, `rust-version` (1.88, for `as_chunks`) and docs.rs's
  `all-features`, and `flow-sdk-c` names `flow-sdk`'s version beside its
  path. `documentation` is left out, for docs.rs's own.
- `cargo package -p flow-sdk --allow-dirty` makes the crate and builds it,
  without the Python package; `-p flow-sdk-c --list` lists its files, and
  it builds once `flow-sdk` is on crates.io. Packaged, `flow-sdk-c`'s
  Python test skips, the package not being beside it.
- Every public item of `flow-sdk` is documented (`#![warn(missing_docs)]`
  keeps it so); `flow-sdk-c`'s functions are documented in its header.
- The Python package finds a library in its own folder, and `setup.py`
  marks a wheel that carries one for its platform.

The SDK is licensed under either of the MIT License (`LICENSE-MIT`) or the
Apache License, Version 2.0 (`LICENSE-APACHE`), at your option, as Rust's
own crates are; the repository's root, `c/` and `python/` each carry both,
so each package does, and the demos are under them too. Flow itself isn't
under either.

What waits: the owners on crates.io (`flow-sdk`, `flow-sdk-c`) and PyPI
(`flow-terminal-sdk`), taking out `publish = false`, and building and
uploading the wheels, one a platform. Until then, depend on a tag of this
repository, as above.

## Not yet

- `flow-demo` on Windows through Flow's pipe. A model placed through the
  pipe is drawn in cells ConPTY doesn't know of, which a repaint can take
  away, so `Query::without_pipe` keeps the demos off it.
- Kitty's animation and pictures under text, which Flow doesn't show yet.
- The C library and the Python package tried on Windows, and the C
  library's audio heard through WASAPI there.
