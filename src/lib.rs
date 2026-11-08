//! Flow's SDK: what Flow adds over a plain console, for any program that
//! runs in it. Every piece is an escape sequence a program writes, so the
//! crate is mostly about writing them as Flow reads them, finding out
//! first whether the terminal reads them at all, and falling back quietly
//! where it doesn't:
//!
//! - [`detect`]: what the terminal can do (kitty pictures, Ratty models, a
//!   cell's pixels, which terminal it is, its background), asked before
//!   anything reads the keys, and on Windows Flow's pipe for images
//!   ([`pipe`]).
//! - [`picture`]: pictures with the kitty graphics protocol, shown by
//!   Unicode placeholder cells.
//! - [`model`]: 3D models with the Ratty graphics protocol; [`scene`] and
//!   [`stage`] build a whole scene each frame and fit it to the cells.
//! - [`fx`]: the liquid's effects (OSC 7731), the glass's wash, Flow's named
//!   sounds and DECPS notes, and a queue of effects waiting for their
//!   moment.
//! - [`screen`]: a screen of cells drawn whole each frame and sent as its
//!   difference from the last, with pictures among the letters, on a
//!   ground of the program's own if it likes.
//! - [`term`] and [`input`]: raw keys on the alternate screen, the mouse,
//!   and the terminal's size, through crossterm (re-exported).
//! - `audio`, behind the `audio` feature: tones, tunes and songs for a
//!   small band of the program's own, played on the machine.
//! - `theme`, behind the `theme` feature: theme packages for Flow, as
//!   `flowapp install` takes them, written and packed.
//!
//! Like Flow's games and demos it uses none of Flow's crates, so it can be
//! published apart.
#![warn(missing_docs)]

pub mod detect;
pub mod fx;
pub mod input;
pub mod mesh;
pub mod model;
pub mod picture;
pub mod pipe;
pub mod scene;
pub mod screen;
pub mod settings;
pub mod stage;
pub mod term;
#[cfg(feature = "theme")]
pub mod theme;

#[cfg(feature = "audio")]
pub mod audio;
#[cfg(all(feature = "audio", unix, not(target_os = "macos")))]
mod alsa;

pub use crossterm;
pub use detect::{Caps, Query, Terminal};

/// What Flow sets `TERM_PROGRAM` to for the programs in its tabs.
pub const TERM_PROGRAM: &str = "flow-terminal";

/// Whether the program runs in Flow, by the name Flow gives its shells
/// (`TERM_PROGRAM`). It reaches a program where Flow's answers to
/// [`detect`]'s questions don't, as on Windows without the pipe; a program
/// run over ssh from a Flow tab doesn't see it.
pub fn in_flow() -> bool {
    std::env::var("TERM_PROGRAM").is_ok_and(|v| v == TERM_PROGRAM)
}
