//! Plays a short tune of the program's own on the machine, with the
//! `audio` feature: a plucky square-wave line over a held triangle note,
//! then a chord. It plays on macOS, Windows and Linux (where ALSA's library
//! is).
//!
//!   cargo run -p flow-sdk --features audio --example tune

use std::thread::sleep;
use std::time::Duration;

use flow_sdk::audio::{self, pitch, Audio, Tone, Tune, Wave};

fn main() {
    if !audio::AVAILABLE {
        println!("Nothing plays on this system yet.");
        return;
    }
    let mut audio = Audio::new();
    let line = Tune::parse("C5/1 E5/1 G5/1 C6/2 r/1 B5/1 G5/1 D5/1 C5/4", 300.0).expect("the notes read");
    let length = line.length();
    audio.play_tune(line.sound(Tone::new(0.0, 0.0).wave(Wave::Square).volume(0.25).envelope(0.005, 0.06)));
    audio.play(Tone::new(pitch("C3").unwrap_or(130.8), length).wave(Wave::Triangle).volume(0.3).envelope(0.2, 0.6));
    sleep(Duration::from_secs_f32(length + 0.3));
    for (i, name) in ["C4", "E4", "G4", "C5"].iter().enumerate() {
        let pan = i as f32 / 1.5 - 1.0;
        audio.play(Tone::new(pitch(name).unwrap_or(440.0), 1.2).volume(0.2).pan(pan).envelope(0.01, 0.8));
    }
    sleep(Duration::from_secs(2));
}
