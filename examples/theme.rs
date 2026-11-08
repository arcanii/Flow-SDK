//! Writes a package of two themes for Flow and packs it, ready for
//! `flowapp install`: a video terminal in white phosphor, typed in at 9600
//! baud, and an aquarium that brings the water's liquid seen from below.
//! It needs no terminal; the package goes where it's told, or into
//! `target/themes/`.
//!
//!   cargo run -p flow-sdk --features theme --example theme [-- <file.tar.gz>]
//!
//! then, in a Flow tab, `flowapp install <file.tar.gz>`, and the two are
//! in Settings' Theme row and the new-tab picker once Flow is in front
//! again.

use std::path::PathBuf;

use flow_sdk::theme::{Crt, CursorShape, Package, Sounds, Theme};

fn main() {
    let vt52 = Theme::new("vt52")
        .name("VT52")
        .name_in("ja", "VT52 端末")
        .fonts(["VT323", "Glass TTY VT220"])
        .text(0xE8F0F0)
        .cursor(0xE8F0F0)
        .background(0x0A0F0A)
        .phosphor(0xE8F0F0)
        .cursor_shape(CursorShape::Block)
        .blink(true)
        .crt(Crt::Classic)
        .start(["", "^{tall}VT52 DECscope", "^{tall}VT52 DECscope", "", "^READY", ""])
        .baud(9600)
        .prompt("$ ")
        .sounds(Sounds::Terminal)
        .screen(80, 24);
    let aquarium = Theme::new("aquarium")
        .name("Aquarium")
        .text(0xD8F4FF)
        .cursor(0x7CE7FF)
        .background(0x02202E)
        .cursor_shape(CursorShape::Bar)
        .key("fx", "style", "water")
        .key("fx", "water_view", "below")
        .key("fx", "sun", 0.8);
    let package = Package::new("flow-sdk-examples", "1.0").theme(vt52).theme(aquarium);
    let tarball = std::env::args().nth(1).map_or_else(|| PathBuf::from("target/themes").join(package.file_name()), PathBuf::from);
    match package.pack(&tarball) {
        Ok(()) => {
            let themes: Vec<&str> = package.themes().iter().map(|t| t.id()).collect();
            println!("{}: {} ({})", tarball.display(), package.name(), themes.join(", "));
            println!("In a Flow tab: flowapp install {}", tarball.display());
        }
        Err(error) => {
            eprintln!("The package wasn't made: {error}");
            std::process::exit(1);
        }
    }
}
