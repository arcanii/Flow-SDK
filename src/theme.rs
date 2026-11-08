//! Theme packages for Flow (from 0.1.34): a folder holding `themes.json`
//! and the font files and logo pictures it names, packed as a `.tar.gz`,
//! which `flowapp install mine.tar.gz` installs in a Flow tab. Flow then
//! offers its themes after its own machines in Settings' Theme row and the
//! new-tab picker, and one with a built-in machine's id takes that one's
//! place.
//!
//! This is the writing side of what Flow reads (its `looks/packaged.rs`):
//! [`ThemeFile`] and [`ThemeEntry`] are `themes.json` as Flow's own types
//! hold it, and [`Theme`] and [`Package`] build them. Flow refuses a whole
//! package for one theme that's wrong, so [`Package::check`] makes the
//! same checks before anything is written, and [`Package::write`] and
//! [`Package::pack`] write nothing that fails them. They're a little
//! stricter than Flow where Flow would take a value and quietly do nothing
//! with it: a sound or a language by a name Flow hasn't, a font file that
//! isn't a font, a logo that isn't a PNG, a cursor speed Flow would clamp,
//! a look's setting in `keys` that Flow doesn't take from a theme, or one
//! of the wrong kind, which would leave all of `keys` off.
//!
//! ```no_run
//! use flow_sdk::theme::{Crt, Package, Sounds, Theme};
//!
//! let vt52 = Theme::new("vt52")
//!     .name("VT52")
//!     .name_in("ja", "VT52 端末")
//!     .fonts(["VT323", "Glass TTY VT220"])
//!     .text(0xE8F0F0)
//!     .cursor(0xE8F0F0)
//!     .background(0x0A0F0A)
//!     .phosphor(0xE8F0F0)
//!     .crt(Crt::Classic)
//!     .start(["", "^VT52 READY", ""])
//!     .baud(9600)
//!     .prompt("$ ")
//!     .sounds(Sounds::Terminal)
//!     .screen(80, 24);
//! Package::new("vt-themes", "1.0").theme(vt52).pack("vt-themes-1.0.tar.gz")?;
//! # Ok::<(), flow_sdk::theme::Error>(())
//! ```
//!
//! A theme is only data: its start screen and prompt are text Flow types,
//! with Flow's own `{…}` fields (`{logo}`, `{wait}`, `{tall}`, `{red}`,
//! `{#d5524d}`, `{clear}` and the rest its README lists under Theme), and
//! nothing in a package runs, so `flowapp` installs one of the user's own
//! unsigned, and refuses one holding `bin/`, which only the catalog's apps
//! may bring.
//!
//! [`Package::pack`] runs the system's `tar`, as Flow's own scripts do,
//! every entry owned by 0 with no names: `/usr/bin/tar` (bsdtar) on macOS
//! without the Mac's metadata, Windows' own `tar.exe`, and elsewhere the
//! `tar` on the PATH, with GNU tar's flags or, where it says it's bsdtar,
//! bsdtar's.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read as _};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

/// The JSON a theme's `keys` are values of, as [`Theme::key`] takes them.
pub use serde_json;

/// The file a package holds its themes in.
pub const FILE: &str = "themes.json";

/// The form of `themes.json` written here, the one Flow reads.
pub const FORMAT: u32 = 1;

/// The most `themes.json` may be, as Flow reads it.
pub const MOST_JSON: u64 = 1024 * 1024;

/// The most a packed package may be, as `flowapp` installs it.
pub const MOST_PACKED: u64 = 512 * 1024 * 1024;

/// The most a logo picture may be, as Flow shows it.
pub const MOST_LOGO: u64 = 8 << 20;

/// Flow's own text color, which a theme has until it's given one.
pub const TEXT: u32 = 0xE4E9F2;

/// Flow's own cursor color.
pub const CURSOR: u32 = 0x8FE3FF;

/// Flow's own background, the glass's tint.
pub const BACKGROUND: u32 = 0x080C16;

/// The folder in a package its font files go in.
pub const FONTS: &str = "fonts";

/// The folder in a package its logo pictures go in.
pub const LOGOS: &str = "logos";

/// The look's settings a theme may bring more of in its `keys`, by section,
/// as Flow's config names them (Flow's `THEME_KEYS`, as a saved theme
/// holds them): put on after the rest of the theme, so they win.
pub const KEYS: [(&str, &[&str]); 5] = [
    ("colors", &["text", "cursor", "background", "border", "ansi", "phosphor"]),
    ("cursor", &["shape", "blink", "speed"]),
    ("font", &["families"]),
    ("fx", &["style", "water_view", "sun", "electricity", "speed", "ambient", "rain", "text_ripple", "caustics", "palette", "opacity", "glow", "crt"]),
    ("crt", &["scanlines", "interlace", "glow", "decay", "grille", "vignette", "curvature"]),
];

/// An enum of the names `themes.json` takes for something, each variant's
/// name and the list of them all.
macro_rules! named {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident = $text:literal,)* }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vdoc])* $variant,)*
        }

        impl $name {
            /// Every one, in Flow's order.
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            /// Its name in `themes.json`.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$variant => $text,)*
                }
            }

            /// The one called `name`, if Flow has one.
            pub fn named(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|v| v.name() == name)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}

named! {
    /// The cursor's shape as a tab starts and as programs reset it.
    CursorShape {
        /// A block over the whole cell, Flow's own.
        Block = "block",
        /// A line under the cell.
        Underline = "underline",
        /// A bar at the cell's left.
        Bar = "bar",
    }
}

named! {
    /// One of the CRT presets built into Flow, which a theme's tube takes.
    Crt {
        /// A light touch.
        Subtle = "subtle",
        /// The default's look.
        Classic = "classic",
        /// Brighter, curved, a coarser mask.
        Arcade = "arcade",
        /// A long afterglow, as a monochrome tube's.
        Phosphor = "phosphor",
        /// A television's mask and curve.
        Television = "television",
        /// A home computer's own monitor: not interlaced, a finer mask, a
        /// gentler curve.
        Monitor = "monitor",
    }
}

named! {
    /// What the machine sounded like, heard with Flow's Machines' sounds
    /// on, as `[theme]`'s `sounds` names them.
    Sounds {
        /// Nothing of its own: Flow's look.
        Quiet = "quiet",
        /// A tube's hum, and its keys' clicks.
        Tube = "tube",
        /// A teletype: letters clack, its bell rings near a line's end, its
        /// carriage runs back, and its tape punch chatters with output.
        Teletype = "teletype",
        /// A tube, and a disk drive seeking as the machine starts.
        Disk = "disk",
        /// A disk drive's, and a fan whirring with the hum.
        Fan = "fan",
        /// A tube, and a printer printing each line of a command's output.
        Printer = "printer",
        /// A video terminal: a tube, sharper key clicks, and a beep for
        /// the bell.
        Terminal = "terminal",
        /// A tube, and a chord as the machine starts.
        Chime = "chime",
        /// A home computer's sound chip: a high beep as it starts, a low
        /// honk for the bell.
        Tones = "tones",
        /// A home computer's floppy drive, seeking as it starts and as its
        /// disk goes in, clicking while it's empty.
        Floppy = "floppy",
    }
}

/// A package's `themes.json`, as Flow reads it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeFile {
    /// [`FORMAT`].
    pub format: u32,
    /// Its name, as `flowapp list` shows it: lowercase letters, digits and
    /// dashes.
    pub package: String,
    /// Its version, as `flowapp list` shows it.
    pub version: String,
    /// Its themes, at least one, each with an id of its own.
    pub themes: Vec<ThemeEntry>,
}

/// One theme, as `themes.json` holds it. Its fields and their order are
/// Flow's, so it's written as Flow writes its own packages; a field left
/// at its default ([`ThemeEntry::default`]) is as Flow takes one left out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeEntry {
    /// Lowercase letters, digits and dashes, at most 40; a built-in
    /// machine's to take its place, never Flow's own (`flow`) or the user's
    /// own (`own`).
    pub id: String,
    /// Its name in English (the id shows where it has none).
    pub name: String,
    /// Its name in other languages, by their code ("ja").
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    /// Font families to try first, by name.
    pub fonts: Vec<String>,
    /// Font files in the package (`fonts/mine.ttf`), loaded with Flow's
    /// own, which `fonts` can then name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub font_files: Vec<String>,
    /// `#RRGGBB`.
    pub text: String,
    /// `#RRGGBB`.
    pub cursor: String,
    /// `#RRGGBB`, the glass's tint.
    pub background: String,
    /// `#RRGGBB` round the text, as a home computer's border; empty for
    /// none.
    pub border: String,
    /// Programs' sixteen colors (black, red, green, yellow, blue, magenta,
    /// cyan, white, then the bright ones); empty for Flow's own.
    pub ansi: Vec<String>,
    /// The one color everything shows in, as on a monochrome tube; empty
    /// for a screen in color.
    pub phosphor: String,
    /// "block", "underline" or "bar" ([`CursorShape`]).
    pub cursor_shape: String,
    /// Whether the cursor blinks.
    pub blink: bool,
    /// How fast the cursor glides to its cell: 1 as Flow's, 0.5 heavier, up
    /// to 4 quick.
    pub cursor_speed: f32,
    /// A built-in CRT preset's name ([`Crt`]); empty for no tube.
    pub crt: String,
    /// The start screen, a line each, typed into each new tab.
    pub start: Vec<String>,
    /// The bits a second it's typed at, ten to a letter; 0 shows it at once.
    pub baud: u32,
    /// The prompt it gives the shells (`\n` a new line, `{cwd}` the folder,
    /// `{n}` the command's number); none keeps the shell's own.
    pub prompt: Option<String>,
    /// What it sounds like ([`Sounds`]).
    pub sounds: String,
    /// Its screen, as columns and rows, which Flow's Fit window to offers
    /// first.
    pub screen: Option<(usize, usize)>,
    /// A PNG in the package (`logos/mine.png`) shown as the startup logo;
    /// empty for the one Settings picks.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub logo: String,
    /// More of the look's settings ([`KEYS`]), by section, put on after the
    /// rest: `{"fx": {"style": "water", "water_view": "below"}}`.
    #[serde(skip_serializing_if = "serde_json::Map::is_empty")]
    pub keys: serde_json::Map<String, serde_json::Value>,
}

impl Default for ThemeEntry {
    /// A theme as Flow takes one with nothing but its id: Flow's own
    /// colors, a block cursor, no tube, no start screen, quiet.
    fn default() -> Self {
        ThemeEntry {
            id: String::new(),
            name: String::new(),
            names: BTreeMap::new(),
            fonts: Vec::new(),
            font_files: Vec::new(),
            text: hex(TEXT),
            cursor: hex(CURSOR),
            background: hex(BACKGROUND),
            border: String::new(),
            ansi: Vec::new(),
            phosphor: String::new(),
            cursor_shape: CursorShape::Block.name().into(),
            blink: false,
            cursor_speed: 1.0,
            crt: String::new(),
            start: Vec::new(),
            baud: 0,
            prompt: None,
            sounds: Sounds::Quiet.name().into(),
            screen: None,
            logo: String::new(),
            keys: serde_json::Map::new(),
        }
    }
}

/// What went wrong making a package.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Flow would refuse it, for the reason given.
    Refused(String),
    /// Reading, writing or packing what's named failed.
    Io(String, io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Refused(why) => f.write_str(why),
            Error::Io(what, error) => write!(f, "{what}: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Refused(_) => None,
            Error::Io(_, error) => Some(error),
        }
    }
}

fn refused<T>(why: String) -> Result<T, Error> {
    Err(Error::Refused(why))
}

/// An [`Error::Io`] about `path`.
fn about(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |error| Error::Io(path.display().to_string(), error)
}

/// `rgb` (0xRRGGBB, higher bits ignored) as `themes.json` writes it.
pub fn hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

/// Whether `text` is a color as written here: `#` and six hex digits.
fn is_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Whether `id` is one a theme may have.
pub fn is_id(id: &str) -> bool {
    let fine = !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    fine && !matches!(id, "flow" | "own")
}

/// Whether `name` is one a package may have: lowercase letters, digits and
/// dashes, at most 64, starting with a letter or digit, and not a folder
/// `flowapp` keeps of its own beside the packages (`bin`, `first`).
pub fn is_package_name(name: &str) -> bool {
    let fine = !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    fine && !name.starts_with('-') && !matches!(name, "bin" | "first")
}

/// Whether `version` is one a package may have: letters, digits and
/// `.`, `-`, `+` and `_`, at most 64, not starting with a dot.
pub fn is_version(version: &str) -> bool {
    let fine = !version.is_empty() && version.len() <= 64 && version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'));
    fine && !version.starts_with('.')
}

/// Whether `code` names a language as Flow matches a theme's names: the
/// language's own two or three lowercase letters ("ja", not "ja-JP").
fn is_language(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase())
}

/// `file`, a path in a package, checked as Flow checks it: relative, every
/// part a name (no `..`), in no folder of programs.
fn in_package(id: &str, file: &str) -> Result<(), Error> {
    let path = Path::new(file);
    let parts: Vec<Component> = path.components().collect();
    if file.is_empty() || file.contains('\\') || !parts.iter().all(|c| matches!(c, Component::Normal(_))) {
        return refused(format!("{id}: “{file}” isn't a file in the package"));
    }
    if parts.first().is_some_and(|c| c.as_os_str() == "bin") || file == FILE {
        return refused(format!("{id}: “{file}” is a file the package can't hold"));
    }
    Ok(())
}

/// What a value in `keys` has to be, by its section and key, as Flow's
/// config holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Number,
    Bool,
    Text,
    /// `#RRGGBB`.
    Color,
    /// `#RRGGBB`, or empty for none.
    ColorOrNone,
    /// Sixteen colors, none (empty), or null for the machine's own.
    Colors,
    /// A color or empty, or null for the machine's own.
    Phosphor,
    /// Font families.
    Texts,
    /// A [`CursorShape`]'s name.
    Shape,
}

fn kind(section: &str, key: &str) -> Option<Kind> {
    use Kind::*;
    if !KEYS.iter().any(|(s, keys)| *s == section && keys.contains(&key)) {
        return None;
    }
    Some(match (section, key) {
        ("colors", "text" | "cursor" | "background") => Color,
        ("colors", "border") => ColorOrNone,
        ("colors", "ansi") => Colors,
        ("colors", "phosphor") => Phosphor,
        ("cursor", "shape") => Shape,
        ("cursor", "blink") | ("crt", "interlace") => Bool,
        ("font", "families") => Texts,
        ("fx", "style" | "water_view" | "palette") => Text,
        ("cursor", "speed") | ("fx", _) | ("crt", _) => Number,
        _ => return None,
    })
}

/// Whether `value` is what `kind` says.
fn fits(kind: Kind, value: &serde_json::Value) -> bool {
    use serde_json::Value;
    let color = |v: &Value| v.as_str().is_some_and(is_color);
    match kind {
        Kind::Number => value.as_f64().is_some_and(f64::is_finite),
        Kind::Bool => value.is_boolean(),
        Kind::Text => value.is_string(),
        Kind::Color => color(value),
        Kind::ColorOrNone => value.as_str().is_some_and(|s| s.is_empty() || is_color(s)),
        Kind::Colors => value.is_null() || value.as_array().is_some_and(|a| (a.is_empty() || a.len() == 16) && a.iter().all(color)),
        Kind::Phosphor => value.is_null() || value.as_str().is_some_and(|s| s.is_empty() || is_color(s)),
        Kind::Texts => value.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        Kind::Shape => value.as_str().is_some_and(|s| CursorShape::named(s).is_some()),
    }
}

impl ThemeEntry {
    /// Whether Flow would take this theme, saying what it wouldn't. Its
    /// font files and logo are checked as paths in a package only: whether
    /// they're there is [`Package::check`]'s.
    pub fn check(&self) -> Result<(), Error> {
        let id = self.id.as_str();
        if !is_id(id) {
            return refused(format!("“{id}” isn't an id a theme may have: lowercase letters, digits and dashes, at most 40, and not flow or own"));
        }
        for code in self.names.keys() {
            if !is_language(code) {
                return refused(format!("{id}: “{code}” isn't a language's code as Flow matches them (two or three lowercase letters, as \"ja\")"));
            }
        }
        for (what, color) in [("text", &self.text), ("cursor", &self.cursor), ("background", &self.background)] {
            if !is_color(color) {
                return refused(format!("{id}: its {what} “{color}” isn't #RRGGBB"));
            }
        }
        for (what, color) in [("border", &self.border), ("phosphor", &self.phosphor)] {
            if !color.is_empty() && !is_color(color) {
                return refused(format!("{id}: its {what} “{color}” isn't #RRGGBB, nor empty for none"));
            }
        }
        match self.ansi.len() {
            0 | 16 => {}
            n => return refused(format!("{id}: {n} colors for programs, not 16")),
        }
        if let Some(color) = self.ansi.iter().find(|c| !is_color(c)) {
            return refused(format!("{id}: its color “{color}” isn't #RRGGBB"));
        }
        if CursorShape::named(&self.cursor_shape).is_none() {
            return refused(format!("{id}: no cursor shape “{}” (block, underline or bar)", self.cursor_shape));
        }
        if !(0.5..=4.0).contains(&self.cursor_speed) {
            return refused(format!("{id}: a cursor speed of {} isn't from 0.5 to 4", self.cursor_speed));
        }
        if !self.crt.is_empty() && Crt::named(&self.crt).is_none() {
            return refused(format!("{id}: no CRT preset “{}”", self.crt));
        }
        if self.start.len() > 200 || self.start.iter().any(|l| l.len() > 400) || self.baud > 1_000_000 {
            return refused(format!("{id}: its start screen is too long, or typed too fast (200 lines of 400 bytes, 1,000,000 baud at most)"));
        }
        if Sounds::named(&self.sounds).is_none() {
            return refused(format!("{id}: no sounds “{}”", self.sounds));
        }
        if let Some((cols, rows)) = self.screen {
            if !(10..=500).contains(&cols) || !(4..=300).contains(&rows) {
                return refused(format!("{id}: no screen is {cols} × {rows} (10 to 500 columns, 4 to 300 rows)"));
            }
        }
        for file in &self.font_files {
            in_package(id, file)?;
        }
        if !self.logo.is_empty() {
            in_package(id, &self.logo)?;
        }
        for (section, keys) in &self.keys {
            let Some(keys) = keys.as_object() else {
                return refused(format!("{id}: its keys' “{section}” isn't a section of settings"));
            };
            for (key, value) in keys {
                let Some(kind) = kind(section, key) else {
                    return refused(format!("{id}: “{section}.{key}” isn't one of the look's settings a theme may bring"));
                };
                if !fits(kind, value) {
                    return refused(format!("{id}: “{section}.{key}” can't be {value}"));
                }
            }
        }
        Ok(())
    }
}

impl ThemeFile {
    /// Whether Flow would take this `themes.json`, saying what it wouldn't
    /// (its themes' files as [`ThemeEntry::check`] checks them).
    pub fn check(&self) -> Result<(), Error> {
        if self.format != FORMAT {
            return refused(format!("{FILE} is format {} (Flow reads {FORMAT})", self.format));
        }
        if !is_package_name(&self.package) {
            return refused(format!("“{}” isn't a package's name: lowercase letters, digits and dashes, at most 64", self.package));
        }
        if !is_version(&self.version) {
            return refused(format!("{}: “{}” isn't a version: letters, digits and . - + _, at most 64", self.package, self.version));
        }
        if self.themes.is_empty() {
            return refused(format!("{} holds no themes", self.package));
        }
        for (i, theme) in self.themes.iter().enumerate() {
            theme.check()?;
            if self.themes[..i].iter().any(|t| t.id == theme.id) {
                return refused(format!("two themes are “{}”", theme.id));
            }
        }
        let size = self.to_json().len() as u64;
        if size > MOST_JSON {
            return refused(format!("{FILE} would be {size} bytes, more than Flow reads ({MOST_JSON})"));
        }
        Ok(())
    }

    /// As Flow writes `themes.json` (pretty, ending in a new line), checked
    /// or not.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a theme file is always JSON") + "\n"
    }
}

/// What a file a theme brings is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Brought {
    Font,
    Logo,
}

/// A theme being built, with the files on disk that its font files and
/// logo are copied from. Each setter returns the theme, so they chain;
/// [`Package::theme`] takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    entry: ThemeEntry,
    /// Where each of `entry.font_files` comes from, in its order.
    fonts_from: Vec<PathBuf>,
    /// Where `entry.logo` comes from.
    logo_from: Option<PathBuf>,
}

impl Theme {
    /// A theme with id `id` (lowercase letters, digits and dashes; a
    /// built-in machine's, as `green`, to take its place), wearing Flow's
    /// own colors until it's given its own.
    pub fn new(id: impl Into<String>) -> Self {
        Theme { entry: ThemeEntry { id: id.into(), ..ThemeEntry::default() }, fonts_from: Vec::new(), logo_from: None }
    }

    /// `entry` as a theme, its font files and logo found in `dir` (the
    /// package it came from, unpacked), to change it or pack it again.
    pub fn from_entry(entry: ThemeEntry, dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let fonts_from = entry.font_files.iter().map(|f| dir.join(f)).collect();
        let logo_from = (!entry.logo.is_empty()).then(|| dir.join(&entry.logo));
        Theme { entry, fonts_from, logo_from }
    }

    /// The theme as `themes.json` will hold it.
    pub fn entry(&self) -> &ThemeEntry {
        &self.entry
    }

    /// Its id.
    pub fn id(&self) -> &str {
        &self.entry.id
    }

    /// Its name in English, as Settings shows it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.entry.name = name.into();
        self
    }

    /// Its name in the language with code `code` ("ja"), as Settings shows
    /// it while Flow speaks that language.
    pub fn name_in(mut self, code: impl Into<String>, name: impl Into<String>) -> Self {
        self.entry.names.insert(code.into(), name.into());
        self
    }

    /// The font families to try first, by name, in place of any before:
    /// installed ones, Flow's own, or ones the theme brings
    /// ([`Theme::font_file`]).
    pub fn fonts<S: Into<String>>(mut self, families: impl IntoIterator<Item = S>) -> Self {
        self.entry.fonts = families.into_iter().map(Into::into).collect();
        self
    }

    /// Brings the font file at `path` (TrueType, OpenType or a collection
    /// of them), copied into the package's `fonts/` under its own name, so
    /// [`Theme::fonts`] can name its family.
    pub fn font_file(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let at = in_folder(FONTS, path);
        match self.entry.font_files.iter().position(|f| *f == at) {
            Some(i) => self.fonts_from[i] = path.to_path_buf(),
            None => {
                self.entry.font_files.push(at);
                self.fonts_from.push(path.to_path_buf());
            }
        }
        self
    }

    /// Text that programs leave uncolored, 0xRRGGBB.
    pub fn text(mut self, rgb: u32) -> Self {
        self.entry.text = hex(rgb);
        self
    }

    /// The cursor's color, 0xRRGGBB.
    pub fn cursor(mut self, rgb: u32) -> Self {
        self.entry.cursor = hex(rgb);
        self
    }

    /// The background, the glass's tint, 0xRRGGBB.
    pub fn background(mut self, rgb: u32) -> Self {
        self.entry.background = hex(rgb);
        self
    }

    /// A border round the text in `rgb`, as a home computer's.
    pub fn border(mut self, rgb: u32) -> Self {
        self.entry.border = hex(rgb);
        self
    }

    /// Programs' sixteen colors: black, red, green, yellow, blue, magenta,
    /// cyan and white, then the bright ones.
    pub fn ansi(mut self, colors: [u32; 16]) -> Self {
        self.entry.ansi = colors.iter().map(|c| hex(*c)).collect();
        self
    }

    /// Everything shown in shades of `rgb`, as on a monochrome tube: text,
    /// pictures, models, effects and Flow's own interface.
    pub fn phosphor(mut self, rgb: u32) -> Self {
        self.entry.phosphor = hex(rgb);
        self
    }

    /// The cursor's shape.
    pub fn cursor_shape(mut self, shape: CursorShape) -> Self {
        self.entry.cursor_shape = shape.name().into();
        self
    }

    /// Whether the cursor blinks.
    pub fn blink(mut self, blink: bool) -> Self {
        self.entry.blink = blink;
        self
    }

    /// How fast the cursor glides to its cell: 1 as Flow's, 0.5 heavier, up
    /// to 4 quick.
    pub fn cursor_speed(mut self, speed: f32) -> Self {
        self.entry.cursor_speed = speed;
        self
    }

    /// A tube, as `preset` has it.
    pub fn crt(mut self, preset: Crt) -> Self {
        self.entry.crt = preset.name().into();
        self
    }

    /// The start screen, a line each, typed into each new tab, in place of
    /// any before: a line starting with `^` is centered, and Flow's `{…}`
    /// fields stand for what they say (`{version}`, `{logo}`, `{wait}`,
    /// `{tall}`, `{red}`, `{clear}` and the rest).
    pub fn start<S: Into<String>>(mut self, lines: impl IntoIterator<Item = S>) -> Self {
        self.entry.start = lines.into_iter().map(Into::into).collect();
        self
    }

    /// How fast the start screen is typed, in bits a second (300 a
    /// teletype's, 9600 a terminal's); 0 shows it at once.
    pub fn baud(mut self, baud: u32) -> Self {
        self.entry.baud = baud;
        self
    }

    /// The prompt it gives the shells: `\n` a new line, `{cwd}` the
    /// folder, `{n}` the command's number.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.entry.prompt = Some(prompt.into());
        self
    }

    /// What the machine sounds like.
    pub fn sounds(mut self, sounds: Sounds) -> Self {
        self.entry.sounds = sounds.name().into();
        self
    }

    /// Its screen, `cols` × `rows` characters (10 to 500, 4 to 300), which
    /// Fit window to offers first.
    pub fn screen(mut self, cols: usize, rows: usize) -> Self {
        self.entry.screen = Some((cols, rows));
        self
    }

    /// The PNG at `path` as the startup logo, copied into the package's
    /// `logos/` under its own name.
    pub fn logo(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        self.entry.logo = in_folder(LOGOS, path);
        self.logo_from = Some(path.to_path_buf());
        self
    }

    /// More of the look's settings: `key` of `section` (one of [`KEYS`]),
    /// as Flow's config holds it, put on after the rest of the theme, as
    /// `.key("fx", "style", "water")` gives the theme the water's liquid.
    pub fn key(mut self, section: &str, key: &str, value: impl Into<serde_json::Value>) -> Self {
        let section = self.entry.keys.entry(section.to_string()).or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if !section.is_object() {
            *section = serde_json::Value::Object(serde_json::Map::new());
        }
        if let serde_json::Value::Object(keys) = section {
            keys.insert(key.to_string(), value.into());
        }
        self
    }

    /// Whether Flow would take this theme, and whether the files it brings
    /// are there and are what they say.
    pub fn check(&self) -> Result<(), Error> {
        self.entry.check()?;
        for (at, from, brought) in self.files() {
            check_file(&self.entry.id, at, from, brought)?;
        }
        Ok(())
    }

    /// Each file it brings: where it goes in the package, where it comes
    /// from, and what it is.
    fn files(&self) -> impl Iterator<Item = (&str, &Path, Brought)> {
        let fonts = self.entry.font_files.iter().zip(&self.fonts_from).map(|(at, from)| (at.as_str(), from.as_path(), Brought::Font));
        let logo = self.logo_from.as_deref().map(|from| (self.entry.logo.as_str(), from, Brought::Logo));
        fonts.chain(logo)
    }
}

/// Where the file at `path` goes in `folder` of a package: under its own
/// name (made UTF-8 where it isn't), or nothing, which the checks refuse,
/// for a path that names no file.
fn in_folder(folder: &str, path: &Path) -> String {
    match path.file_name() {
        Some(name) => format!("{folder}/{}", name.to_string_lossy()),
        None => String::new(),
    }
}

/// Whether the file at `from`, going to `at` for theme `id`, is a file and
/// what it's brought as: a font Flow can load, or a PNG small enough to
/// show.
fn check_file(id: &str, at: &str, from: &Path, brought: Brought) -> Result<(), Error> {
    let shown = from.display();
    let meta = std::fs::metadata(from).map_err(about(from))?;
    if !meta.is_file() {
        return refused(format!("{id}: {shown} isn't a file"));
    }
    let mut start = [0u8; 8];
    let mut file = std::fs::File::open(from).map_err(about(from))?;
    let read = file.read(&mut start).map_err(about(from))?;
    let start = &start[..read];
    match brought {
        Brought::Font => {
            let font = [&b"\x00\x01\x00\x00"[..], b"OTTO", b"true", b"ttcf"].iter().any(|magic| start.starts_with(magic));
            if !font {
                return refused(format!("{id}: {shown} (for {at}) isn't a TrueType or OpenType font"));
            }
        }
        Brought::Logo => {
            if !start.starts_with(b"\x89PNG\r\n\x1a\n") {
                return refused(format!("{id}: {shown} (for {at}) isn't a PNG"));
            }
            if meta.len() > MOST_LOGO {
                return refused(format!("{id}: {shown} is {} bytes, more than Flow shows as a logo ({MOST_LOGO})", meta.len()));
            }
        }
    }
    Ok(())
}

/// A package of themes being built: its name, its version, and its themes.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    name: String,
    version: String,
    themes: Vec<Theme>,
}

impl Package {
    /// A package called `name` (lowercase letters, digits and dashes), at
    /// `version` ("1.0"), holding no themes yet.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Package { name: name.into(), version: version.into(), themes: Vec::new() }
    }

    /// The package unpacked in `dir`, its themes' files found there, to
    /// change it or pack it again: refused if its `themes.json` isn't one
    /// Flow would read.
    pub fn read(dir: impl AsRef<Path>) -> Result<Self, Error> {
        let dir = dir.as_ref();
        let path = dir.join(FILE);
        let size = std::fs::metadata(&path).map_err(about(&path))?.len();
        if size > MOST_JSON {
            return refused(format!("{} is {size} bytes, more than Flow reads ({MOST_JSON})", path.display()));
        }
        let text = std::fs::read_to_string(&path).map_err(about(&path))?;
        let file: ThemeFile = serde_json::from_str(&text).map_err(|e| Error::Refused(format!("{}: {e}", path.display())))?;
        if file.format != FORMAT {
            return refused(format!("{} is format {} (this SDK reads {FORMAT})", path.display(), file.format));
        }
        let themes = file.themes.into_iter().map(|entry| Theme::from_entry(entry, dir)).collect();
        Ok(Package { name: file.package, version: file.version, themes })
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Its themes, in the order Flow lists them.
    pub fn themes(&self) -> &[Theme] {
        &self.themes
    }

    /// With `theme` too.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.add(theme);
        self
    }

    /// Adds `theme`, in place of one with its id already in the package.
    pub fn add(&mut self, theme: Theme) {
        match self.themes.iter_mut().find(|t| t.entry.id == theme.entry.id) {
            Some(was) => *was = theme,
            None => self.themes.push(theme),
        }
    }

    /// The file a packed package is usually called: `<name>-<version>.tar.gz`.
    pub fn file_name(&self) -> String {
        format!("{}-{}.tar.gz", self.name, self.version)
    }

    /// Its `themes.json` as Flow reads it.
    pub fn file(&self) -> ThemeFile {
        ThemeFile { format: FORMAT, package: self.name.clone(), version: self.version.clone(), themes: self.themes.iter().map(|t| t.entry.clone()).collect() }
    }

    /// Whether Flow would take the package, saying what it wouldn't: its
    /// `themes.json`, and each file a theme brings there and what it says
    /// it is, with no two different files going to one place.
    pub fn check(&self) -> Result<(), Error> {
        self.file().check()?;
        let mut placed: BTreeMap<&str, &Path> = BTreeMap::new();
        for theme in &self.themes {
            for (at, from, brought) in theme.files() {
                match placed.get(at) {
                    Some(already) if *already != from => {
                        return refused(format!("{} and {} would both be the package's {at}", already.display(), from.display()));
                    }
                    Some(_) => {}
                    None => {
                        check_file(&theme.entry.id, at, from, brought)?;
                        placed.insert(at, from);
                    }
                }
            }
        }
        Ok(())
    }

    /// Its `themes.json`, checked, as [`Package::write`] writes it.
    pub fn json(&self) -> Result<String, Error> {
        self.check()?;
        Ok(self.file().to_json())
    }

    /// Writes the package, checked, into `dir` (made if it isn't there):
    /// `themes.json`, and the files its themes bring, copied into `fonts/`
    /// and `logos/`. Whatever else `dir` holds is left as it is.
    pub fn write(&self, dir: impl AsRef<Path>) -> Result<(), Error> {
        let dir = dir.as_ref();
        let json = self.json()?;
        std::fs::create_dir_all(dir).map_err(about(dir))?;
        let mut copied: Vec<&str> = Vec::new();
        for (at, from, _) in self.themes.iter().flat_map(Theme::files) {
            if copied.contains(&at) {
                continue;
            }
            let to = dir.join(at);
            if let Some(folder) = to.parent() {
                std::fs::create_dir_all(folder).map_err(about(folder))?;
            }
            std::fs::copy(from, &to).map_err(about(from))?;
            copied.push(at);
        }
        let path = dir.join(FILE);
        std::fs::write(&path, json).map_err(about(&path))
    }

    /// Packs the package, checked, as `tarball` (a `.tar.gz`, its folder
    /// made if it isn't there): written into a folder of its own under the
    /// system's temporary one, then packed from there with the system's
    /// `tar`, `./themes.json` and its files owned by 0 with no names, as
    /// `flowapp install` takes it.
    pub fn pack(&self, tarball: impl AsRef<Path>) -> Result<(), Error> {
        let given = tarball.as_ref();
        self.check()?;
        let tarball = std::path::absolute(given).map_err(about(given))?;
        if let Some(folder) = tarball.parent() {
            std::fs::create_dir_all(folder).map_err(about(folder))?;
        }
        let stage = Stage::new()?;
        self.write(&stage.0)?;
        let mut tar = tar();
        tar.arg("-czf").arg(&tarball).arg("-C").arg(&stage.0).arg(".").stdin(Stdio::null());
        let failed = |why: String| {
            let _ = std::fs::remove_file(&tarball);
            Err(Error::Io(format!("packing {}", tarball.display()), io::Error::other(why)))
        };
        let out = match tar.output() {
            Ok(out) => out,
            Err(error) => return failed(format!("running tar: {error}")),
        };
        if !out.status.success() {
            return failed(format!("tar {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim()));
        }
        let size = std::fs::metadata(&tarball).map_err(about(&tarball))?.len();
        if size > MOST_PACKED {
            let _ = std::fs::remove_file(&tarball);
            return refused(format!("{} would be {size} bytes, more than flowapp installs ({MOST_PACKED})", tarball.display()));
        }
        Ok(())
    }
}

/// A folder of its own under the system's temporary one, deleted with
/// what's in it when dropped.
struct Stage(PathBuf);

impl Stage {
    fn new() -> Result<Self, Error> {
        static N: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir();
        for _ in 0..100 {
            let n = N.fetch_add(1, Ordering::Relaxed);
            let dir = base.join(format!("flow-theme-{}-{n}", std::process::id()));
            // Made afresh, never one that's there already (another's).
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(Stage(dir)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(about(&dir)(e)),
            }
        }
        Err(Error::Io(base.display().to_string(), io::Error::other("no folder of its own could be made")))
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// bsdtar's flags for entries owned by 0 with no names.
const BSDTAR: [&str; 8] = ["--uid", "0", "--gid", "0", "--uname", "", "--gname", ""];

/// GNU tar's.
const GNU_TAR: [&str; 3] = ["--owner=0", "--group=0", "--numeric-owner"];

/// The system's `tar`, made to own every entry by 0 with no names.
fn tar() -> Command {
    if cfg!(target_os = "macos") {
        // The system's bsdtar, without the Mac's own metadata (its ._ files
        // and extended attributes).
        let mut tar = Command::new("/usr/bin/tar");
        tar.env("COPYFILE_DISABLE", "1").arg("--no-mac-metadata").args(BSDTAR);
        tar
    } else if cfg!(windows) {
        // Windows' own, bsdtar, rather than one an msys or Git's PATH puts
        // first.
        let root = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let mut tar = Command::new(root.join("System32").join("tar.exe"));
        tar.args(BSDTAR);
        tar
    } else {
        let bsd = Command::new("tar").arg("--version").stdin(Stdio::null()).output().is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("bsdtar"));
        let mut tar = Command::new("tar");
        if bsd {
            tar.args(BSDTAR);
        } else {
            tar.args(GNU_TAR);
        }
        tar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flow's own machines as `cargo run --example theme_package -- --version
    /// 0.1.0` in Flow wrote them, its `flow-core-themes` package.
    const FLOWS_OWN: &str = include_str!("../tests/fixtures/flow-core-themes.json");

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("flow-sdk-theme-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The smallest font file Flow would load, as far as its first bytes go.
    const FONT: &[u8] = b"\x00\x01\x00\x00\x00\x0a\x00\x80\x00\x03\x00\x20";

    /// A PNG's first bytes.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR";

    fn flows_own() -> ThemeFile {
        serde_json::from_str(FLOWS_OWN).unwrap()
    }

    #[test]
    fn flows_own_package_reads_and_writes_back_byte_for_byte() {
        let file = flows_own();
        assert_eq!(file.themes.len(), 10);
        assert_eq!(file.to_json(), FLOWS_OWN);
        file.check().unwrap();
        // Made again through a package, theme by theme.
        let mut package = Package::new(&file.package, &file.version);
        for entry in &file.themes {
            package.add(Theme::from_entry(entry.clone(), "."));
        }
        assert_eq!(package.json().unwrap(), FLOWS_OWN);
    }

    #[test]
    fn a_theme_built_holds_what_flows_own_does() {
        let file = flows_own();
        let green = Theme::new("green")
            .name("Green screen")
            .name_in("ja", "グリーンディスプレイ")
            .fonts(["VT323", "Glass TTY VT220", "IBM 3270", "3270 Nerd Font Mono"])
            .text(0x41FF6E)
            .cursor(0x41FF6E)
            .background(0x020B04)
            .ansi([0x0A2A12, 0x2FAF4F, 0x41FF6E, 0x7CFF9A, 0x2A8C44, 0x36C95A, 0x5CFF86, 0x9CFFB4, 0x1E5A2C, 0x4FD873, 0x6BFF8F, 0xACFFC2, 0x3AA35A, 0x5FE07F, 0x86FFA6, 0xD0FFDC])
            .phosphor(0x41FF6E)
            .blink(true)
            .cursor_speed(1.5)
            .crt(Crt::Phosphor)
            .start(["FLOW-11 MINICOMPUTER", "{memory} OF CORE", "", "BOOTING TIME-SHARING SYSTEM ...", ""])
            .baud(1200)
            .prompt("$ ")
            .sounds(Sounds::Terminal)
            .screen(80, 24);
        let flows = file.themes.iter().find(|t| t.id == "green").unwrap();
        assert_eq!(green.entry(), flows);
        // Written alone as Flow's own package writes it.
        let written = Package::new("flow-core-themes", "0.1.0").theme(green).json().unwrap();
        assert!(FLOWS_OWN.contains(&written[written.find("    {\n      \"id\": \"green\"").unwrap()..written.rfind("\n  ]").unwrap()]));
    }

    #[test]
    fn a_theme_with_only_its_id_is_written_with_flows_own_defaults() {
        let written = serde_json::to_value(Theme::new("plain").entry()).unwrap();
        let flows = serde_json::json!({
            "id": "plain", "name": "", "fonts": [], "text": "#E4E9F2", "cursor": "#8FE3FF", "background": "#080C16",
            "border": "", "ansi": [], "phosphor": "", "cursor_shape": "block", "blink": false, "cursor_speed": 1.0,
            "crt": "", "start": [], "baud": 0, "prompt": null, "sounds": "quiet", "screen": null,
        });
        assert_eq!(written, flows);
        // And one left out reads as Flow reads it: as its default.
        let read: ThemeEntry = serde_json::from_str(r#"{"id": "plain"}"#).unwrap();
        assert_eq!(read, *Theme::new("plain").entry());
    }

    #[test]
    fn the_names_flow_takes_are_the_ones_named_here() {
        assert_eq!(CursorShape::ALL.iter().map(|s| s.name()).collect::<Vec<_>>(), ["block", "underline", "bar"]);
        assert_eq!(Crt::ALL.iter().map(|c| c.name()).collect::<Vec<_>>(), ["subtle", "classic", "arcade", "phosphor", "television", "monitor"]);
        assert_eq!(Sounds::ALL.len(), 10);
        assert_eq!(Sounds::named("floppy"), Some(Sounds::Floppy));
        assert_eq!(Sounds::named("organ"), None);
        // Each of Flow's own machines names only these.
        for theme in flows_own().themes {
            assert!(Sounds::named(&theme.sounds).is_some() && (theme.crt.is_empty() || Crt::named(&theme.crt).is_some()), "{}", theme.id);
        }
        for (section, keys) in KEYS {
            for key in keys {
                assert!(kind(section, key).is_some(), "{section}.{key}");
            }
        }
        assert_eq!(kind("fx", "liquid"), None, "a tab's, not a theme's");
    }

    #[test]
    fn what_flow_refuses_is_refused_saying_what() {
        let fine = || Theme::new("fine");
        let bad: Vec<(Theme, &str)> = vec![
            (Theme::new("flow"), "Flow's own look"),
            (Theme::new("own"), "your own"),
            (Theme::new("Big Name"), "an id of other letters"),
            (Theme::new(""), "no id"),
            (Theme::new("a".repeat(41)), "an id too long"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), text: "green".into(), ..ThemeEntry::default() }, "."), "a color by name"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), border: "#12345".into(), ..ThemeEntry::default() }, "."), "a border too short"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), ansi: vec!["#000000".into(); 15], ..ThemeEntry::default() }, "."), "fifteen colors"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), cursor_shape: "heart".into(), ..ThemeEntry::default() }, "."), "a cursor shape"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), crt: "mine".into(), ..ThemeEntry::default() }, "."), "a CRT preset Flow hasn't"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), sounds: "organ".into(), ..ThemeEntry::default() }, "."), "sounds Flow hasn't"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), font_files: vec!["../../etc/passwd".into()], ..ThemeEntry::default() }, "."), "a file outside it"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), logo: "/etc/hosts".into(), ..ThemeEntry::default() }, "."), "a file anywhere"),
            (Theme::from_entry(ThemeEntry { id: "a".into(), logo: "bin/flow".into(), ..ThemeEntry::default() }, "."), "a file among programs"),
            (fine().screen(5000, 24), "a screen too wide"),
            (fine().screen(80, 2), "a screen too short"),
            (fine().start(vec!["x"; 201]), "a start screen too long"),
            (fine().start(["x".repeat(401)]), "a line too long"),
            (fine().baud(2_000_000), "typed too fast"),
            (fine().cursor_speed(f32::NAN), "a speed that isn't a number, which JSON would write as null"),
            (fine().cursor_speed(9.0), "a speed Flow would clamp"),
            (fine().name_in("ja-JP", "x"), "a language Flow never matches"),
            (fine().key("fx", "liquid", false), "a tab's setting, not a theme's"),
            (fine().key("ai", "provider", "x"), "a section Flow never takes"),
            (fine().key("fx", "style", 3), "a style that isn't a name"),
            (fine().key("colors", "ansi", serde_json::json!(["#000000"])), "one color for programs"),
            (fine().key("cursor", "shape", "heart"), "a cursor shape among the keys"),
        ];
        for (theme, what) in bad {
            assert!(matches!(theme.check(), Err(Error::Refused(_))), "{what}");
            let package = Package::new("mine", "1.0").theme(fine()).theme(theme);
            assert!(matches!(package.check(), Err(Error::Refused(_))), "{what}");
        }
        let one = |name: &str, version: &str| Package::new(name, version).theme(fine());
        assert!(one("mine", "1.0").check().is_ok());
        for (name, version, what) in [("Mine", "1.0", "a capital"), ("my.themes", "1.0", "a dot in the name"), ("bin", "1.0", "flowapp's own folder"), ("first", "1.0", "flowapp's other"), ("", "1.0", "no name"), ("mine", "", "no version"), ("mine", ".1", "a hidden folder"), ("mine", "1/0", "a folder in the version"), ("mine", "1.0 beta", "a space")] {
            assert!(one(name, version).check().is_err(), "{what}");
        }
        assert!(Package::new("mine", "1.0").check().is_err(), "no themes at all");
        let twice = ThemeFile { format: FORMAT, package: "mine".into(), version: "1.0".into(), themes: vec![fine().entry().clone(), fine().entry().clone()] };
        assert!(twice.check().is_err(), "two of one id");
        assert_eq!(Package::new("mine", "1.0").theme(fine()).theme(fine().name("Fine")).themes().len(), 1, "a package keeps the last of an id");
        assert!(ThemeFile { format: 2, ..flows_own() }.check().is_err(), "another format");
        let error = Package::new("mine", "1.0").theme(fine().cursor_speed(9.0)).check().unwrap_err();
        assert_eq!(error.to_string(), "fine: a cursor speed of 9 isn't from 0.5 to 4");
    }

    #[test]
    fn a_theme_may_bring_more_of_the_looks_settings() {
        let aquarium = Theme::new("aquarium").key("fx", "style", "water").key("fx", "water_view", "below").key("fx", "sun", 0.8).key("crt", "interlace", false).key("colors", "ansi", serde_json::Value::Null);
        aquarium.check().unwrap();
        let written = serde_json::to_value(aquarium.entry()).unwrap();
        assert_eq!(written["keys"], serde_json::json!({ "fx": { "style": "water", "water_view": "below", "sun": 0.8 }, "crt": { "interlace": false }, "colors": { "ansi": null } }));
    }

    #[test]
    fn files_are_copied_in_checked_and_never_two_to_one_place() {
        let dir = scratch("files");
        let (font, logo, other_font) = (dir.join("Mine.ttf"), dir.join("mine.png"), dir.join("other").join("Mine.ttf"));
        std::fs::create_dir_all(other_font.parent().unwrap()).unwrap();
        std::fs::write(&font, FONT).unwrap();
        std::fs::write(&other_font, FONT).unwrap();
        std::fs::write(&logo, PNG).unwrap();
        let theme = Theme::new("mine").font_file(&font).fonts(["Mine Mono"]).logo(&logo);
        assert_eq!((theme.entry().font_files.clone(), theme.entry().logo.as_str()), (vec!["fonts/Mine.ttf".to_string()], "logos/mine.png"));
        let package = Package::new("mine", "1.0").theme(theme.clone()).theme(Theme::new("also").font_file(&font));
        let out = dir.join("out");
        package.write(&out).unwrap();
        assert_eq!(std::fs::read(out.join("fonts/Mine.ttf")).unwrap(), FONT);
        assert_eq!(std::fs::read(out.join("logos/mine.png")).unwrap(), PNG);
        // Read back from what was written, it's the same package, its files
        // now found there.
        let read = Package::read(&out).unwrap();
        assert_eq!(read.file(), package.file());
        assert_eq!(read.themes()[0].files().map(|(_, from, _)| from.to_path_buf()).collect::<Vec<_>>(), [out.join("fonts/Mine.ttf"), out.join("logos/mine.png")]);
        // Another file of the same name would go to the same place.
        let clash = Package::new("mine", "1.0").theme(theme.clone()).theme(Theme::new("other").font_file(&other_font));
        assert!(clash.check().unwrap_err().to_string().contains("would both be the package's fonts/Mine.ttf"));
        // A file that isn't there, or isn't what it's brought as.
        assert!(matches!(Theme::new("gone").font_file(dir.join("gone.ttf")).check(), Err(Error::Io(..))));
        assert!(matches!(Theme::new("png").font_file(&logo).check(), Err(Error::Refused(_))), "a picture as a font");
        assert!(matches!(Theme::new("ttf").logo(&font).check(), Err(Error::Refused(_))), "a font as a logo");
        assert!(matches!(Theme::new("dir").logo(&dir).check(), Err(Error::Refused(_))), "a folder");
        // What's refused writes nothing.
        let nothing = dir.join("nothing");
        assert!(Package::new("mine", "1.0").theme(Theme::new("Bad")).write(&nothing).is_err());
        assert!(!nothing.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Each entry of the `.tar.gz` at `path`: its name, its owner and group
    /// by number and by name.
    fn entries(path: &Path) -> Vec<(String, u64, u64, String, String)> {
        let gz = std::fs::read(path).unwrap();
        assert_eq!(&gz[..3], b"\x1f\x8b\x08", "gzip, deflated");
        let flags = gz[3];
        let mut at = 10;
        if flags & 4 != 0 {
            at += 2 + usize::from(u16::from_le_bytes([gz[at], gz[at + 1]]));
        }
        for bit in [8, 16] {
            if flags & bit != 0 {
                at += gz[at..].iter().position(|&b| b == 0).unwrap() + 1;
            }
        }
        if flags & 2 != 0 {
            at += 2;
        }
        let tar = miniz_oxide::inflate::decompress_to_vec(&gz[at..gz.len() - 8]).unwrap();
        let text = |b: &[u8]| String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned();
        let octal = |b: &[u8]| u64::from_str_radix(text(b).trim(), 8).unwrap_or(0);
        let mut found = Vec::new();
        let mut block = 0;
        while (block + 1) * 512 <= tar.len() {
            let header = &tar[block * 512..(block + 1) * 512];
            if header.iter().all(|&b| b == 0) {
                break;
            }
            let size = octal(&header[124..136]) as usize;
            let kind = header[156];
            // Real entries only: a pax header or a long name is about the
            // next one.
            if matches!(kind, b'0' | 0 | b'5') {
                let prefix = text(&header[345..500]);
                let name = if prefix.is_empty() { text(&header[..100]) } else { format!("{prefix}/{}", text(&header[..100])) };
                found.push((name, octal(&header[108..116]), octal(&header[116..124]), text(&header[265..297]), text(&header[297..329])));
            }
            block += 1 + size.div_ceil(512);
        }
        found
    }

    #[test]
    fn a_packed_package_holds_its_json_and_its_files_owned_by_no_one() {
        let dir = scratch("pack");
        let (font, logo) = (dir.join("Mine.ttf"), dir.join("mine.png"));
        std::fs::write(&font, FONT).unwrap();
        std::fs::write(&logo, PNG).unwrap();
        let package = Package::new("mine", "1.0").theme(Theme::new("mine").name("Mine").font_file(&font).logo(&logo).sounds(Sounds::Tube));
        let tarball = dir.join("out").join(package.file_name());
        package.pack(&tarball).unwrap();
        let entries = entries(&tarball);
        let mut names: Vec<&str> = entries.iter().map(|e| e.0.as_str()).collect();
        names.sort();
        assert_eq!(names, ["./", "./fonts/", "./fonts/Mine.ttf", "./logos/", "./logos/mine.png", "./themes.json"]);
        for (name, uid, gid, user, group) in &entries {
            assert_eq!((*uid, *gid, user.as_str(), group.as_str()), (0, 0, "", ""), "{name}");
        }
        // Unpacked as flowapp unpacks it, it's the package.
        let unpacked = dir.join("unpacked");
        std::fs::create_dir_all(&unpacked).unwrap();
        let ran = Command::new("tar").arg("-xzf").arg(&tarball).arg("-C").arg(&unpacked).status().unwrap();
        assert!(ran.success());
        assert_eq!(std::fs::read_to_string(unpacked.join(FILE)).unwrap(), package.json().unwrap());
        assert_eq!(Package::read(&unpacked).unwrap().file(), package.file());
        // What's refused packs nothing, and leaves no folder behind.
        let refused = dir.join("refused.tar.gz");
        assert!(Package::new("mine", "1.0").pack(&refused).is_err());
        assert!(!refused.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
