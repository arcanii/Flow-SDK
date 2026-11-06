//! Model: a hot-air balloon that Flow reads from a file. The demo builds it
//! as a mesh (`flow_sdk::mesh`), writes it as a GLB into a folder of its
//! own in the system's temporary one, and names the file with the Ratty
//! protocol's `path=` rather than sending it (`model::register_path`),
//! placing it at once. Flow reads a file a program names only once the
//! user allows it: it asks, naming the tab and the file, and on Allow reads
//! it on a thread of its own into the placement already made. On Don't
//! Allow it reads nothing and answers nothing, so the program can't tell
//! the difference and the cells stay empty, the glass showing through; the
//! words below say so. A placement waiting for its file draws nothing, and
//! the question waits as long as the user does.
//!
//! The demo turns the balloon itself, about its upright, tipped a little
//! toward the eye and bobbing, sending how it looks each frame
//! (`model::update`, Ratty's `u`), which Flow keeps for the model before
//! it's read too. Its burner fires now and then, and on a click: the
//! balloon brightens, and the liquid bursts orange under it and is pushed
//! up with a whoosh. ← and → turn it faster one way or the other; space
//! stops it. Quitting takes the model away and deletes the file and its
//! folder (a question still open then finds the file gone, and Flow says
//! it couldn't read it), and so does a tab closed under it, which hangs its
//! terminal up (`hangup`). What a run killed outright left, the next one
//! sweeps away as it starts ([`sweep`]).

use std::f32::consts::TAU;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use flow_sdk::fx::Effect;
use flow_sdk::input::KeyCode;
use flow_sdk::mesh::Mesh;
use flow_sdk::model::{self, Format, Look, Place};
use flow_sdk::scene::{rgb, V3};
use flow_sdk::term::Size;

use crate::{hangup, Needs, TextDemo, Told, Written};

/// The object's id in the tab.
const ID: u32 = 1;
const FILE: &str = "balloon.glb";
/// What starts the name of the demo's folder, before its process's id.
const PREFIX: &str = "flow-demo-";
/// Thirty frames a second: each sends how it's turned.
const INTERVAL: Duration = Duration::from_micros(33_333);
/// Rows of words below the balloon, with one between.
const WORDS: u16 = 5;
const MIN_COLS: u16 = 40;
const MIN_ROWS: u16 = WORDS + 2 + 8;
/// Degrees a second it turns to begin with, the fastest either way, and a
/// step of the arrows.
const SPEED: f32 = 30.0;
const FASTEST: f32 = 120.0;
const STEP: f32 = 15.0;
/// Degrees it's tipped toward the eye, so its crown shows.
const TIP: f32 = 14.0;
/// Seconds between the burner's firings (the first this far in), and how
/// long the balloon glows after one.
const BURN_EVERY: f32 = 7.0;
const FIRST_BURN: f32 = 3.0;
const GLOW: f32 = 0.8;
/// The most pixels a side of its block may have: Flow takes a placement up
/// to 4096 a side and about 8 million in all.
const MAX_SIDE: f32 = 2800.0;
const TEXT: u32 = 0x9aa0b4;
const PATH: u32 = 0x7ce7ff;
const FLAME: u32 = 0xff8a20;

/// The envelope's outline from its crown down to its mouth: how far out
/// it is at each height.
const PROFILE: [(f32, f32); 12] = [(0.0, 1.0), (0.3, 0.97), (0.52, 0.88), (0.67, 0.74), (0.75, 0.57), (0.77, 0.4), (0.72, 0.22), (0.62, 0.05), (0.48, -0.12), (0.34, -0.28), (0.22, -0.42), (0.15, -0.52)];
/// Its panels round, and the band of rows about its widest.
const GORES: usize = 16;
const BAND: std::ops::RangeInclusive<usize> = 4..=5;

/// A box from `lo` to `hi`, its faces wound outward.
fn cuboid(mesh: &mut Mesh, color: u32, lo: V3, hi: V3) {
    let c = rgb(color);
    let middle = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
    let at = |x: usize, y: usize, z: usize| [[lo[0], hi[0]][x], [lo[1], hi[1]][y], [lo[2], hi[2]][z]];
    for face in [
        [at(0, 0, 0), at(0, 1, 0), at(0, 1, 1), at(0, 0, 1)],
        [at(1, 0, 0), at(1, 1, 0), at(1, 1, 1), at(1, 0, 1)],
        [at(0, 0, 0), at(1, 0, 0), at(1, 0, 1), at(0, 0, 1)],
        [at(0, 1, 0), at(1, 1, 0), at(1, 1, 1), at(0, 1, 1)],
        [at(0, 0, 0), at(1, 0, 0), at(1, 1, 0), at(0, 1, 0)],
        [at(0, 0, 1), at(1, 0, 1), at(1, 1, 1), at(0, 1, 1)],
    ] {
        mesh.face(c, &face, middle);
    }
}

/// A rope from `a` to `b`, a thin square rod.
fn rope(mesh: &mut Mesh, color: u32, a: V3, b: V3, e: f32) {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
    let d = [d[0] / len, d[1] / len, d[2] / len];
    // Two ways across it.
    let u = {
        let u = [d[1], -d[0], 0.0];
        let k = (u[0] * u[0] + u[1] * u[1]).sqrt().max(1e-6);
        [u[0] / k * e, u[1] / k * e, 0.0]
    };
    let v = [(d[1] * u[2] - d[2] * u[1]), (d[2] * u[0] - d[0] * u[2]), (d[0] * u[1] - d[1] * u[0])];
    let off = |p: V3, s: f32, t: f32| [p[0] + u[0] * s + v[0] * t, p[1] + u[1] * s + v[1] * t, p[2] + u[2] * s + v[2] * t];
    let middle = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
    let c = rgb(color);
    for (s0, t0, s1, t1) in [(1.0, 1.0, 1.0, -1.0), (1.0, -1.0, -1.0, -1.0), (-1.0, -1.0, -1.0, 1.0), (-1.0, 1.0, 1.0, 1.0)] {
        mesh.face(c, &[off(a, s0, t0), off(a, s1, t1), off(b, s1, t1), off(b, s0, t0)], middle);
    }
}

/// The balloon: its envelope in panels of red and gold, a band of blue and
/// cream about its widest (one panel of it lighter), its mouth, the burner,
/// the ropes and the basket.
pub fn balloon() -> Mesh {
    let mut mesh = Mesh::new();
    let point = |i: usize, j: usize| {
        let (r, y) = PROFILE[i];
        let a = TAU * j as f32 / GORES as f32;
        [r * a.cos(), y, r * a.sin()]
    };
    for i in 0..PROFILE.len() - 1 {
        let inside = [0.0, (PROFILE[i].1 + PROFILE[i + 1].1) * 0.5, 0.0];
        for j in 0..GORES {
            // One panel of the band in Flow's own blue, so its turning shows.
            let color = match (BAND.contains(&i), j % 2) {
                (true, _) if j == 0 => 0x38b8e0,
                (true, 0) => 0x2a5ad8,
                (true, _) => 0xf2e8d0,
                (false, 0) => 0xd8342c,
                (false, _) => 0xf0b828,
            };
            let quad = [point(i, j), point(i, j + 1), point(i + 1, j + 1), point(i + 1, j)];
            // At the crown the first two are the one point.
            let face: &[V3] = if i == 0 { &quad[1..] } else { &quad };
            mesh.face(rgb(color), face, inside);
        }
    }
    let mouth: Vec<V3> = (0..GORES).map(|j| point(PROFILE.len() - 1, j)).collect();
    mesh.face(rgb(0x3a2018), &mouth, [0.0, 0.0, 0.0]);
    cuboid(&mut mesh, 0x6a6a72, [-0.05, -0.6, -0.05], [0.05, -0.56, 0.05]);
    let throat = PROFILE[PROFILE.len() - 1];
    for (sx, sz) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let k = throat.0 * std::f32::consts::FRAC_1_SQRT_2;
        rope(&mut mesh, 0xc8b89a, [k * sx, throat.1, k * sz], [0.085 * sx, -0.66, 0.085 * sz], 0.008);
    }
    cuboid(&mut mesh, 0x8a5a2a, [-0.09, -0.8, -0.09], [0.09, -0.66, 0.09]);
    cuboid(&mut mesh, 0x5a3a1a, [-0.1, -0.675, -0.1], [0.1, -0.655, 0.1]);
    mesh
}

/// The model's file, deleted with its folder when it's let go.
#[derive(Debug)]
pub struct ModelFile {
    dir: PathBuf,
    pub path: PathBuf,
    pub bytes: usize,
}

impl ModelFile {
    /// Writes `glb` as `balloon.glb` into the folder `dir`, made afresh for
    /// it and, on Unix, open to this user alone. In a temporary folder every
    /// user shares, as /tmp is on Linux, a folder of that name could be
    /// another's, with a link where the file goes to have something else
    /// written over: so a name already taken is passed over for the next
    /// (`dir-1`, `dir-2`…), and the file is only ever made new.
    pub fn write(dir: &Path, glb: &[u8]) -> io::Result<Self> {
        let dir = own_dir(dir)?;
        let path = dir.join(FILE);
        // Where they're deleted from if the demo ends where its own thread
        // can't get to them.
        hangup::delete_at_close(&path, &dir);
        let written = fs::OpenOptions::new().write(true).create_new(true).open(&path).and_then(|mut f| f.write_all(glb));
        if let Err(e) = written {
            let _ = fs::remove_dir(&dir);
            hangup::deleted(&dir);
            return Err(e);
        }
        Ok(Self { dir, path, bytes: glb.len() })
    }
}

/// Deletes the folders that earlier runs left in `parent`, killed outright
/// or crashed before they could (a tab closed under one ends it as q does:
/// `hangup`). Says how many went.
pub fn sweep(parent: &Path) -> usize {
    sweep_by(parent, std::process::id(), hangup::running)
}

/// [`sweep`], with `own` this process's id and `running` saying which
/// processes still run. A folder goes only if it's one the demo leaves:
/// named for a process that's gone and isn't this one (`flow-demo-<pid>`,
/// or one of the names [`own_dir`] passes on to), a folder itself and not a
/// link to one, this user's alone, and holding nothing but the model's
/// file, itself a file and not a link, or nothing at all. Only that file
/// and the folder are deleted, so nothing else ever is, even if someone
/// changes the folder meanwhile.
fn sweep_by(parent: &Path, own: u32, running: impl Fn(u32) -> bool) -> usize {
    let Ok(entries) = fs::read_dir(parent) else { return 0 };
    let mut swept = 0;
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(pid_of) else { continue };
        let dir = entry.path();
        if pid == own || !left_by_the_demo(&dir) || running(pid) {
            continue;
        }
        let _ = fs::remove_file(dir.join(FILE));
        if fs::remove_dir(&dir).is_ok() {
            swept += 1;
        }
    }
    swept
}

/// The process a folder of the demo's is named for: `flow-demo-<pid>`, or
/// `flow-demo-<pid>-<n>` with `n` from 1 to 15, written as the demo writes
/// them (no sign, no leading nought).
fn pid_of(name: &str) -> Option<u32> {
    let number = |s: &str| s.parse::<u32>().ok().filter(|v| v.to_string() == s);
    let rest = name.strip_prefix(PREFIX)?;
    let pid = match rest.split_once('-') {
        Some((pid, n)) if number(n).is_some_and(|n| (1..16).contains(&n)) => pid,
        Some(_) => return None,
        None => rest,
    };
    number(pid).filter(|&pid| pid > 0)
}

/// Whether `dir` is a folder as the demo leaves one: itself a folder, not a
/// link, this user's alone, and holding the model's file alone, itself a
/// file and not a link, or nothing.
fn left_by_the_demo(dir: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(dir) else { return false };
    if !meta.is_dir() || !hangup::mine(&meta) {
        return false;
    }
    let Ok(entries) = fs::read_dir(dir) else { return false };
    let mut seen = 0;
    for entry in entries {
        let Ok(entry) = entry else { return false };
        seen += 1;
        // An entry's type is its own, not what a link points to.
        if seen > 1 || entry.file_name() != FILE || !entry.file_type().is_ok_and(|t| t.is_file()) {
            return false;
        }
    }
    true
}

/// Makes a new folder at `dir`, or the first of `dir-1` to `dir-15` that
/// isn't there yet.
fn own_dir(dir: &Path) -> io::Result<PathBuf> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    let mut taken = None;
    for n in 0..16 {
        let mut name = dir.as_os_str().to_owned();
        if n > 0 {
            name.push(format!("-{n}"));
        }
        let candidate = PathBuf::from(name);
        match builder.create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => taken = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(taken.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)))
}

impl Drop for ModelFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        // Only if nothing else is in it.
        let _ = fs::remove_dir(&self.dir);
        hangup::deleted(&self.dir);
    }
}

/// How `path` goes in the protocol, if it can: as text, without the `;`
/// that ends a key's value, or an escape.
pub fn nameable(path: &Path) -> Option<&str> {
    path.to_str().filter(|p| !p.chars().any(|c| c == ';' || c.is_control()))
}

/// The block of cells the balloon is placed over: above the words, about
/// as wide in pixels as it's tall, within what Flow takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub left: u16,
    pub top: u16,
    pub cols: u16,
    pub rows: u16,
}

impl Block {
    /// For a screen of `size` (a cell 9 × 19 pixels where it doesn't say);
    /// none if it's under 40 × 15.
    pub fn new(size: Size) -> Option<Self> {
        if size.cols < MIN_COLS || size.rows < MIN_ROWS {
            return None;
        }
        let (cw, ch) = size.cell.unwrap_or((9.0, 19.0));
        let (cw, ch) = (cw.floor().max(1.0), ch.floor().max(1.0));
        // A row above it and one below, before the words.
        let free = size.rows - WORDS - 2;
        let side = (f32::from(free) * ch).min(f32::from(size.cols - 2) * cw).min(MAX_SIDE);
        let rows = ((side / ch) as u16).clamp(1, free).min(model::MAX_SPAN);
        let cols = ((side / cw).round() as u16).clamp(1, size.cols - 2).min(model::MAX_SPAN);
        Some(Self { left: (size.cols - cols) / 2, top: 1 + (free - rows) / 2, cols, rows })
    }

    /// Where the burner fires: below the envelope's mouth, over the basket.
    pub fn burner(&self) -> (u16, u16) {
        (self.left + self.cols / 2, self.top + (f32::from(self.rows) * 0.85) as u16)
    }

    pub fn contains(&self, col: u16, row: u16) -> bool {
        (self.left..self.left + self.cols).contains(&col) && (self.top..self.top + self.rows).contains(&row)
    }
}

pub struct Balloon {
    t: f32,
    /// Degrees it's turned, and a second.
    yaw: f32,
    speed: f32,
    stopped: bool,
    told: Told,
    /// The folder the file goes in, the file once written, or why it isn't.
    dir: PathBuf,
    file: Option<ModelFile>,
    error: Option<String>,
    registered: bool,
    /// The screen it was laid out for, and its block there.
    size: Option<(u16, u16)>,
    block: Option<Block>,
    /// A cell's height in pixels, for how far it bobs.
    cell_height: f32,
    /// The words on the screen.
    shown: Vec<String>,
    burned: f32,
    burn: bool,
}

impl Balloon {
    /// With its file in a folder of the demo's own: in the user's own
    /// runtime folder where there's one (`XDG_RUNTIME_DIR`, on Linux), else
    /// in the system's temporary one, where what earlier runs left is swept
    /// away first.
    pub fn new() -> Self {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|d| d.is_absolute() && d.is_dir());
        let parent = runtime.unwrap_or_else(std::env::temp_dir);
        sweep(&parent);
        Self::in_dir(parent.join(format!("{PREFIX}{}", std::process::id())))
    }

    pub fn in_dir(dir: PathBuf) -> Self {
        Self {
            t: 0.0,
            yaw: 20.0,
            speed: SPEED,
            stopped: false,
            told: Told { flow: true, effects: true },
            dir,
            file: None,
            error: None,
            registered: false,
            size: None,
            block: None,
            cell_height: 19.0,
            shown: Vec::new(),
            burned: FIRST_BURN - BURN_EVERY,
            burn: false,
        }
    }

    /// How it looks now: turned, tipped, bobbing, and glowing after the
    /// burner fires.
    fn look(&self) -> Look {
        let glow = 1.0 - ((self.t - self.burned) / GLOW).clamp(0.0, 1.0);
        let bob = (self.t * 1.3).sin() * self.cell_height * f32::from(self.block.map_or(10, |b| b.rows)) * 0.012;
        let round = |v: f32| (v * 100.0).round() / 100.0;
        Look { rotation: [Some(TIP), Some(round(self.yaw.rem_euclid(360.0))), None], offset: [None, Some(round(bob)), None], brightness: Some(round(1.0 + 0.35 * glow)), ..Look::default() }
    }

    /// The words below the balloon, each fitted to `cols`.
    fn words(&self, cols: u16) -> Vec<String> {
        let cols = usize::from(cols);
        let fit = |s: String| -> String {
            let n = s.chars().count();
            if n <= cols {
                s
            } else {
                // A path keeps its end, where the file's name is.
                format!("…{}", s.chars().skip(n + 1 - cols).collect::<String>())
            }
        };
        let file = match (&self.file, &self.error) {
            (_, Some(error)) => error.clone(),
            (Some(f), None) => format!("{} (GLB, {} KB)", f.path.display(), f.bytes.div_ceil(1024)),
            (None, None) => String::new(),
        };
        let (ask, no) = if self.told.flow {
            ("Flow asks you first. Allow: it reads the file, and the balloon turns here.", "Don't Allow: Flow reads nothing and answers nothing, and this stays empty.")
        } else {
            ("Not in Flow, which asks you before it reads a file a program names:", "this terminal reads it, or doesn't, its own way.")
        };
        let keys = if self.stopped { "Stopped · space turns it again · click: the burner · q quits and deletes the file" } else { "← → turn it · space stops it · click: the burner · q quits and deletes the file" };
        ["A balloon from a file, named with the Ratty protocol's path= instead of sent".to_string(), file, ask.to_string(), no.to_string(), keys.to_string()].into_iter().map(fit).collect()
    }

    /// The burner: the balloon glows, the liquid bursts under it and is
    /// pushed up, with a whoosh (a stir's own sound).
    fn fire(&mut self, glass: &mut Vec<String>) {
        self.burned = self.t;
        if let Some(block) = self.block {
            let at = block.burner();
            let size = f32::from(block.cols) * 0.18;
            glass.push(Effect::burst(at).color(FLAME).size(size).strength(0.55).quiet().escape());
            glass.push(Effect::stir(at, (0.0, -1.0)).size(size * 1.5).strength(0.45).escape());
        }
    }

    /// [`TextDemo::frame`], without writing the file: tests hand it one.
    fn step(&mut self, dt: f32, size: Size) -> Written {
        let mut w = Written::default();
        self.t += dt;
        if let Some((_, ch)) = size.cell {
            self.cell_height = ch.floor();
        }
        // The file named once, whatever the window's size: Flow asks then.
        if let (Some(file), false) = (&self.file, self.registered) {
            match nameable(&file.path) {
                Some(path) => {
                    w.graphics += &model::register_path(ID, Format::Glb, path);
                    self.registered = true;
                }
                None => self.error = Some(format!("{}: a path the protocol can't carry (a ; or not text)", file.path.display())),
            }
        }
        let mut placed = false;
        if self.size != Some((size.cols, size.rows)) {
            self.size = Some((size.cols, size.rows));
            self.block = Block::new(size);
            self.shown.clear();
            w.text += "\x1b[0m\x1b[2J";
            // Placed again after the clear, which took its cells: Flow
            // moves a placement placed again.
            if let (Some(block), true) = (self.block, self.registered) {
                w.graphics += &Place::over(ID, (block.top, block.left), (block.cols, block.rows)).look(self.look()).escape();
                placed = true;
            }
        }
        let Some(block) = self.block else {
            if self.shown.is_empty() {
                let text = format!("A bigger window, please: {MIN_COLS} × {MIN_ROWS} at least · q quits");
                w.text += &format!("\x1b[{};{}H\x1b[0;38;2;154;160;180m{text}\x1b[0m", size.rows / 2 + 1, usize::from(size.cols).saturating_sub(text.chars().count()) / 2 + 1);
                self.shown = vec![text];
            }
            return w;
        };
        let glowing = self.t - self.burned < GLOW + 0.1;
        if !self.stopped {
            self.yaw += self.speed * dt;
            if self.t - self.burned >= BURN_EVERY {
                self.fire(&mut w.glass);
            }
        }
        if std::mem::take(&mut self.burn) {
            self.fire(&mut w.glass);
        }
        // Placed this frame, it looks as it was placed.
        if self.registered && !placed && (!self.stopped || glowing) {
            w.graphics += &model::update(ID, &self.look());
        }
        let words = self.words(size.cols);
        if words != self.shown {
            let first = block.top + block.rows + 1;
            for (i, line) in words.iter().enumerate() {
                let color = match i {
                    1 if self.error.is_none() => PATH,
                    _ => TEXT,
                };
                let [r, g, b] = [color >> 16, (color >> 8) & 0xff, color & 0xff];
                let bold = if i == 0 { ";1" } else { "" };
                let col = usize::from(size.cols).saturating_sub(line.chars().count()) / 2;
                w.text += &format!("\x1b[{}H\x1b[0m\x1b[2K\x1b[{}G\x1b[0;38;2;{r};{g};{b}{bold}m{line}\x1b[0m", first + i as u16 + 1, col + 1);
            }
            self.shown = words;
        }
        w
    }
}

impl Default for Balloon {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDemo for Balloon {
    fn needs(&self) -> Needs {
        Needs::Models
    }

    fn told(&mut self, told: Told) {
        self.told = told;
    }

    fn interval(&self) -> Duration {
        INTERVAL
    }

    fn frame(&mut self, dt: f32, size: Size) -> Written {
        if self.file.is_none() && self.error.is_none() {
            match ModelFile::write(&self.dir, &balloon().glb()) {
                Ok(file) => self.file = Some(file),
                Err(e) => self.error = Some(format!("Couldn't write the model into {}: {e}", self.dir.display())),
            }
        }
        self.step(dt, size)
    }

    fn key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Left => self.speed = (self.speed - STEP).max(-FASTEST),
            KeyCode::Right => self.speed = (self.speed + STEP).min(FASTEST),
            KeyCode::Char(' ') => self.stopped = !self.stopped,
            _ => {}
        }
    }

    fn click(&mut self, col: u16, row: u16) {
        if self.block.is_some_and(|b| b.contains(col, row)) {
            self.burn = true;
        }
    }

    fn quit(&mut self) -> Written {
        // The file goes with it, and its folder.
        self.file = None;
        Written { graphics: if self.registered { model::delete(ID) } else { String::new() }, ..Written::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(cols: u16, rows: u16) -> Size {
        Size { cols, rows, cell: Some((9.5, 19.0)) }
    }

    /// A folder of the test's own, so tests side by side don't share one.
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("flow-demo-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn the_balloon_is_wound_outward_and_tall() {
        let mesh = balloon();
        assert!(mesh.wound_outward());
        assert!((300..800).contains(&mesh.triangles()), "{}", mesh.triangles());
        let glb = mesh.glb();
        assert_eq!(&glb[..4], b"glTF");
        assert!(glb.len() < 64 * 1024, "{} bytes", glb.len());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_already_there_is_passed_over_and_nothing_written_through_a_link() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("planted");
        let target = scratch("planted-target");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_file(&target);
        fs::create_dir(&dir).unwrap();
        fs::write(&target, "someone's file").unwrap();
        std::os::unix::fs::symlink(&target, dir.join(FILE)).unwrap();
        let file = ModelFile::write(&dir, b"glTF").unwrap();
        assert_ne!(file.path.parent(), Some(dir.as_path()), "the folder someone made is left alone");
        assert_eq!(fs::read(&file.path).unwrap(), b"glTF");
        assert_eq!(fs::metadata(file.path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700, "for this user alone");
        assert_eq!(fs::read_to_string(&target).unwrap(), "someone's file", "nothing written through the link");
        drop(file);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn its_file_is_named_by_path_placed_turned_and_deleted_on_quitting() {
        let dir = scratch("named");
        let mut b = Balloon::in_dir(dir.clone());
        let first = b.frame(0.0, screen(100, 40));
        let path = dir.join(FILE);
        assert!(path.is_file(), "written");
        assert_eq!(fs::read(&path).unwrap(), balloon().glb());
        let g = &first.graphics;
        assert!(g.starts_with(&format!("\x1b_ratty;g;r;id=1;fmt=glb;path={}\x1b\\", path.display())), "{g:?}");
        assert!(!g.contains("source=payload"), "named, not sent");
        let block = b.block.unwrap();
        assert!(g.contains(&format!("\x1b_ratty;g;p;id=1;row={};col={};w={};h={};", block.top + (block.rows - 1).div_ceil(2), block.left + (block.cols - 1).div_ceil(2), block.cols, block.rows)));
        assert!(g.contains("rx=14;ry=20") && !g.contains(";g;u;"), "placed as it looks: {g:?}");
        assert!(first.text.starts_with("\x1b[0m\x1b[2J") && first.text.contains("Flow asks you first") && first.text.contains("Don't Allow"));
        // Then only how it's turned, each frame, and the words not again.
        let next = b.frame(0.5, screen(100, 40));
        assert!(next.graphics.starts_with("\x1b_ratty;g;u;id=1;") && next.graphics.contains(";rx=14;ry=35") && !next.graphics.contains(";g;r;"), "{:?}", next.graphics);
        assert!(next.text.is_empty());
        // A resize places it again, but doesn't name the file again (Flow
        // would ask again).
        let resized = b.frame(0.03, screen(120, 40));
        assert!(resized.graphics.contains(";g;p;id=1;") && !resized.graphics.contains(";g;r;"));
        let gone = b.quit();
        assert_eq!(gone.graphics, "\x1b_ratty;g;d;id=1\x1b\\");
        assert!(!path.exists() && !dir.exists(), "the file and its folder are gone");
    }

    /// A scratch folder holding folders named `names`, each made as the
    /// demo makes its own and holding `files`.
    fn folders(name: &str, names: &[String], files: &[&str]) -> PathBuf {
        let parent = scratch(name);
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir(&parent).unwrap();
        for name in names {
            let dir = own_dir(&parent.join(name)).unwrap();
            for file in files {
                fs::write(dir.join(file), b"glTF").unwrap();
            }
        }
        parent
    }

    #[test]
    fn a_folder_left_by_a_process_thats_gone_is_swept_and_this_ones_own_is_kept() {
        let own = std::process::id();
        // Made-up processes, one gone and one running.
        let (dead, alive) = (4_000_001, 4_000_002);
        let running = |pid| pid == own || pid == alive;
        let gone = [format!("flow-demo-{dead}"), format!("flow-demo-{dead}-3")];
        let kept = [format!("flow-demo-{own}"), format!("flow-demo-{own}-1"), format!("flow-demo-{alive}")];
        let parent = folders("sweep-gone", &[&gone[..], &kept[..]].concat(), &[FILE]);
        // One whose run died before it wrote the file.
        own_dir(&parent.join("flow-demo-4000003")).unwrap();
        assert_eq!(sweep_by(&parent, own, running), 3);
        assert!(gone.iter().all(|name| !parent.join(name).exists()) && !parent.join("flow-demo-4000003").exists(), "the dead run's folders are gone");
        assert!(kept.iter().all(|name| parent.join(name).join(FILE).is_file()), "this process's own and a running one's are kept");
        // Nothing left to sweep.
        assert_eq!(sweep_by(&parent, own, running), 0);
        fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn a_name_thats_not_the_demos_or_a_folder_holding_something_else_is_left_alone() {
        let dead = |_| false;
        let names = ["flow-demo-notes", "flow-demo-04000001", "flow-demo-+4000001", "flow-demo-4000001-16", "flow-demo-4000001-0", "flow-demo-4000001-x", "flow-demo-4000001-1-1", "flow-demo-0", "flow-demo-", "flow-demo-test-4000001", "other-4000001"].map(String::from);
        let parent = folders("sweep-names", &names, &[FILE]);
        assert_eq!(sweep_by(&parent, 1, dead), 0);
        assert!(names.iter().all(|name| parent.join(name).join(FILE).is_file()), "names the demo never makes");
        fs::remove_dir_all(&parent).unwrap();
        // The model's file and something else, or something else alone.
        let more = folders("sweep-more", &["flow-demo-4000001".into()], &[FILE, "notes.txt"]);
        let other = folders("sweep-other", &["flow-demo-4000001".into()], &["notes.txt"]);
        // A folder where the file goes, and a file where the folder goes.
        let nested = folders("sweep-nested", &["flow-demo-4000001".into()], &[]);
        fs::create_dir(nested.join("flow-demo-4000001").join(FILE)).unwrap();
        fs::write(nested.join("flow-demo-4000002"), b"glTF").unwrap();
        for parent in [&more, &other, &nested] {
            assert_eq!(sweep_by(parent, 1, dead), 0, "{parent:?}");
        }
        assert!(more.join("flow-demo-4000001/notes.txt").is_file() && more.join("flow-demo-4000001").join(FILE).is_file());
        assert!(other.join("flow-demo-4000001/notes.txt").is_file());
        assert!(nested.join("flow-demo-4000001").join(FILE).is_dir() && nested.join("flow-demo-4000002").is_file());
        for parent in [more, other, nested] {
            fs::remove_dir_all(parent).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn links_and_folders_open_to_others_are_never_swept() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let parent = folders("sweep-links", &["target".into()], &[FILE]);
        let target = parent.join("target");
        // A link to a folder that looks like the demo's.
        symlink(&target, parent.join("flow-demo-4000001")).unwrap();
        // A folder whose file is a link to someone's file.
        let elsewhere = parent.join("someone's.glb");
        fs::write(&elsewhere, b"someone's").unwrap();
        let linked = own_dir(&parent.join("flow-demo-4000002")).unwrap();
        symlink(&elsewhere, linked.join(FILE)).unwrap();
        // One open to everyone, which the demo never makes.
        let open = own_dir(&parent.join("flow-demo-4000003")).unwrap();
        fs::write(open.join(FILE), b"glTF").unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(sweep_by(&parent, 1, |_| false), 0);
        assert!(target.join(FILE).is_file() && parent.join("flow-demo-4000001").exists(), "nothing followed through the link");
        assert_eq!(fs::read(&elsewhere).unwrap(), b"someone's");
        assert!(linked.join(FILE).symlink_metadata().is_ok() && open.join(FILE).is_file());
        fs::remove_dir_all(&parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_run_that_ended_is_swept_by_what_the_system_says() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let dead = child.id();
        child.wait().unwrap();
        let names = [format!("flow-demo-{dead}"), format!("flow-demo-{}", std::process::id())];
        let parent = folders("sweep-real", &names, &[FILE]);
        assert_eq!(sweep(&parent), 1);
        assert!(!parent.join(&names[0]).exists() && parent.join(&names[1]).join(FILE).is_file());
        fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn its_block_is_square_in_pixels_and_within_flows_limits() {
        let block = Block::new(screen(100, 40)).unwrap();
        assert_eq!(block.rows, 40 - WORDS - 2);
        let (w, h) = (f32::from(block.cols) * 9.0, f32::from(block.rows) * 19.0);
        assert!((w / h - 1.0).abs() < 0.05, "{w} × {h}");
        assert!(block.top + block.rows + 1 + WORDS <= 40);
        // A huge window keeps under Flow's limits on a placement.
        let huge = Block::new(Size { cols: 400, rows: 160, cell: Some((18.0, 38.0)) }).unwrap();
        assert!(huge.cols <= model::MAX_SPAN && huge.rows <= model::MAX_SPAN);
        assert!(u64::from(huge.cols) * 18 * u64::from(huge.rows) * 38 <= model::MAX_PIXELS);
        // A narrow one gives a block as wide as it can.
        let narrow = Block::new(screen(40, 60)).unwrap();
        assert!(narrow.cols <= 38 && narrow.rows < 40);
        assert_eq!(Block::new(screen(39, 40)), None);
        assert!(block.contains(block.left, block.top) && !block.contains(block.left + block.cols, block.top));
    }

    #[test]
    fn the_burner_fires_now_and_then_and_on_a_click() {
        let mut b = Balloon::in_dir(scratch("burner"));
        b.frame(0.0, screen(100, 40));
        let mut glass = Vec::new();
        for _ in 0..(10.25 / 0.05) as usize {
            glass.extend(b.frame(0.05, screen(100, 40)).glass);
        }
        // At three seconds and at ten.
        assert_eq!(glass.iter().filter(|g| g.contains("fx=burst")).count(), 2, "{glass:?}");
        assert!(glass.iter().any(|g| g.contains("fx=stir") && g.contains("dir=0.00,-1.00") && !g.contains("sound=")), "pushed up, with a stir's whoosh");
        let block = b.block.unwrap();
        b.click(block.left + 1, block.top + 1);
        let clicked = b.frame(0.05, screen(100, 40));
        assert!(clicked.glass.iter().any(|g| g.contains(&format!("color={FLAME:06x}"))));
        assert!(clicked.graphics.contains("brightness=1.3"), "it glows: {:?}", clicked.graphics);
        // A click off it does nothing.
        b.click(0, 0);
        assert!(b.frame(0.05, screen(100, 40)).glass.is_empty());
        b.quit();
    }

    #[test]
    fn stopped_it_sends_nothing_and_the_arrows_turn_it() {
        let mut b = Balloon::in_dir(scratch("stopped"));
        b.frame(0.0, screen(100, 40));
        b.key(KeyCode::Char(' '));
        b.frame(0.1, screen(100, 40));
        let still = b.frame(0.1, screen(100, 40));
        assert!(still.graphics.is_empty() && still.glass.is_empty());
        b.key(KeyCode::Char(' '));
        for _ in 0..20 {
            b.key(KeyCode::Left);
        }
        assert_eq!(b.speed, -FASTEST);
        let yaw = b.yaw;
        b.frame(0.5, screen(100, 40));
        assert!((b.yaw - (yaw - 60.0)).abs() < 1e-3, "the other way, fastest");
        b.quit();
    }

    #[test]
    fn its_words_say_what_flow_does_and_fit_the_screen() {
        let mut b = Balloon::in_dir(scratch("words"));
        b.frame(0.0, screen(80, 24));
        let words = b.words(80);
        assert!(words.iter().enumerate().all(|(i, w)| i == 1 || !w.starts_with('…')), "only a long path is cut to fit: {words:?}");
        assert!(words[2].contains("asks you first") && words[3].contains("answers nothing"));
        assert!(words[1].ends_with("balloon.glb (GLB, 1 KB)") || words[1].contains("balloon.glb (GLB, "), "{}", words[1]);
        let narrow = b.words(40);
        assert!(narrow[1].starts_with('…') && narrow[1].chars().count() == 40, "a long path keeps its end: {}", narrow[1]);
        b.told(Told { flow: false, effects: false });
        assert!(b.words(80)[2].starts_with("Not in Flow"));
        assert!(b.frame(0.0, screen(30, 10)).text.contains("A bigger window"));
        b.quit();
        // A path the protocol can't carry is said, not sent.
        assert_eq!(nameable(Path::new("/tmp/a;b/balloon.glb")), None);
        assert_eq!(nameable(Path::new("/tmp/flow/balloon.glb")), Some("/tmp/flow/balloon.glb"));
    }
}
