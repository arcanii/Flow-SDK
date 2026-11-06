//! Sets Flow's liquid off around a word in the middle of the screen: a
//! burst with a chime, lightning across with its thunder, a ring, a push
//! with one of Flow's sounds by name, then a run of notes with DECPS. Any
//! key, or a few seconds, ends it. Outside Flow it says so.
//!
//!   cargo run -p flow-sdk --example effect

use std::io::{self, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use flow_sdk::fx::{self, Effect, Sound};
use flow_sdk::{input, term, Query};

fn main() -> io::Result<()> {
    term::leave_on_panic();
    term::enter(false)?;
    let mut terminal = Query::new().version().ask();
    if !terminal.caps.flow {
        term::leave();
        println!("Not in Flow: its liquid's effects and sounds would go unseen and unheard here.");
        return Ok(());
    }
    let size = term::size()?;
    let (cols, rows) = (size.cols.max(20), size.rows.max(8));
    let middle = (cols / 2, rows / 2);
    let word = "flow";
    let mut out = io::stdout();
    write!(out, "\x1b[{};{}H\x1b[1m{word}\x1b[0m", middle.1 + 1, middle.0 + 1 - word.len() as u16 / 2)?;
    out.flush()?;

    // Each after the screen that shows what set it off, a beat apart: Flow
    // takes about six a second.
    let steps = [
        Effect::burst(middle).color(0x7ce7ff).size(6.0).strength(0.8).sound(Sound::Chime),
        Effect::bolt((2, middle.1), (cols - 3, middle.1)).strength(0.9),
        Effect::ripple(middle).size(10.0),
        Effect::stir(middle, (1.0, 0.0)).size(8.0).sound(Sound::Boing),
    ];
    for effect in steps {
        terminal.send_effects(effect.escape().as_bytes())?;
        sleep(Duration::from_millis(600));
    }
    // C major, up and down, an eighth of a second a note.
    let names = ["C5", "E5", "G5", "C6", "r", "G5", "E5", "C5"];
    let notes: Vec<u8> = names.iter().filter_map(|n| fx::note(n)).collect();
    terminal.send_effects(fx::notes(5, 4, &notes).as_bytes())?;

    input::next_before(Instant::now() + Duration::from_secs(3))?;
    term::leave();
    Ok(())
}
