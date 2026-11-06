//! `flow-demo`'s program: lists the demos, or takes the terminal and plays
//! one until it's quit. In the library, so a program elsewhere (Flow's
//! build, which carries `flow-demo`) can be `flow-demo` by calling [`main`].

use std::io::{self, Write};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::hangup;
use crate::pictures::Easel;
use crate::{Demo, Line, Needs, Program, Show, Shows, TextDemo, Told, DEMOS};
use flow_sdk::fx::UNTINT;
use flow_sdk::input::{self, Event};
use flow_sdk::picture::Ids;
use flow_sdk::stage::Stage;
use flow_sdk::{term, Query, Terminal};

fn usage() -> String {
    let mut s = String::from("flow-demo: demos of what Flow draws, over its liquid\n\n");
    for (name, what) in DEMOS {
        s += &format!("  flow-demo {name:<8} {what}\n");
    }
    s + "\n  --no-effects         leave Flow's liquid be (sounds and the glass's wash still go)\n  --version\n\nboing, cube, vector, tint and model need a terminal that shows 3D models\nthrough the Ratty graphics protocol, and plasma one that shows kitty's\npictures, as Flow does; teletext and sounds play anywhere, at their best in\nFlow, whose sounds, wash and liquid are its own. In a demo, q quits."
}

/// `flow-demo`, with the process's arguments: the demo named, or the list.
pub fn main() {
    let refuse = |why: String| -> ! {
        // Not `eprintln!`, which panics where the terminal has gone.
        let _ = writeln!(io::stderr(), "flow-demo: {why}");
        std::process::exit(1);
    };
    let args: Vec<String> = std::env::args_os().skip(1).map(|a| a.into_string().unwrap_or_else(|a| refuse(format!("{} isn't text", a.to_string_lossy())))).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", usage());
        return;
    }
    if args.iter().any(|a| a == "--version") {
        println!("flow-demo {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let (mut name, mut effects) = (None, true);
    for arg in &args {
        match arg.as_str() {
            "--no-effects" => effects = false,
            a if a.starts_with('-') || a.is_empty() => refuse(format!("there's no {a}: --help lists what there is")),
            a if name.is_none() => name = Some(a),
            a => refuse(format!("{a}: one demo at a time")),
        }
    }
    let Some(name) = name else {
        println!("{}", usage());
        return;
    };
    let Some(program) = crate::demo(name) else { refuse(format!("there's no demo called {name}: flow-demo lists them")) };

    // A tab closed under the demo ends it as q does, deleting what it
    // made, rather than on the spot.
    hangup::catch();
    // A panic must not leave the terminal in raw mode.
    term::leave_on_panic();
    if let Err(e) = term::enter(true) {
        term::leave();
        refuse(format!("can't use this terminal: {e}"));
    }
    let flow = flow_sdk::in_flow();
    let effects = effects && flow;
    let told = Told { flow, effects };
    // On Windows ConPTY drops the protocols, so there the questions go down
    // Flow's pipe for images, and so do the scenes, pictures and effects.
    let result = match program {
        Program::Staged(mut demo) => {
            let shows = demo.shows();
            let mut terminal = match shows {
                Shows::Scenes => Query::new().models().cell(),
                Shows::Pictures => Query::new().pictures().cell(),
            }
            .ask();
            let (shown, what) = match shows {
                Shows::Scenes => (terminal.caps.models, "3D models through the Ratty graphics protocol"),
                Shows::Pictures => (terminal.caps.pictures, "pictures through kitty's graphics protocol"),
            };
            if !shown {
                term::leave();
                refuse(format!("this terminal doesn't show {what}; play it in Flow"));
            }
            let whole = terminal.caps.cell;
            demo.told(told);
            run(demo.as_mut(), &mut terminal, whole, told)
        }
        Program::Text(mut demo) => {
            // Text needs nothing asked, but on Windows Flow keeps its pipe,
            // where the effects go, for a program that asked for pictures;
            // a demo that places models asks for them.
            let mut terminal = match demo.needs() {
                Needs::Text => Query::new().pictures().ask(),
                Needs::Models => Query::new().models().cell().ask(),
            };
            if demo.needs() == Needs::Models && !terminal.caps.models {
                term::leave();
                refuse("this terminal doesn't show 3D models through the Ratty graphics protocol; play it in Flow".into());
            }
            demo.told(told);
            run_text(demo.as_mut(), &mut terminal, told)
        }
    };
    term::leave();
    // A terminal that hung up fails what's written to it, and has no one
    // to tell.
    if let Some(e) = result.err().filter(|_| !hangup::caught()) {
        refuse(e.to_string());
    }
}

/// Where pictures' ids start, somewhere another program's are unlikely to.
fn seed() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos()) ^ std::process::id().rotate_left(16)
}

fn run(demo: &mut dyn Demo, terminal: &mut Terminal, whole: Option<(u16, u16)>, told: Told) -> io::Result<()> {
    let mut out = io::stdout();
    let pictures = demo.shows() == Shows::Pictures;
    let mut stage: Option<Stage> = None;
    let mut easel: Option<Easel> = None;
    let mut ids = Ids::new(seed());
    // Laid out again at the start and after the window changes.
    let mut fresh = true;
    let mut shown: Vec<Line> = Vec::new();
    // Whether the glass was washed, to wash it off on the way out.
    let mut washed = false;
    let mut last = Instant::now();
    loop {
        // Hung up or told to end: as on q, for a terminal still there to
        // hear it.
        if hangup::caught() {
            let _ = put_away(easel.as_ref(), stage.as_ref(), washed, terminal);
            return Ok(());
        }
        // What goes to the screen as text (the words, and a picture's
        // cells), and the graphics: a scene goes after the words, a picture
        // before the cells that show it. On Windows the graphics go down
        // Flow's pipe and the text through ConPTY, and a scene's cells,
        // which Flow writes, would be painted over by words sent after them,
        // while a picture's cells, which the program writes, would show
        // nothing until it came.
        let (mut screen, mut graphics, mut glass) = (Vec::new(), Vec::new(), Vec::new());
        let mut size = term::size()?;
        if fresh {
            // ConPTY gives no pixels; Flow's pipe tells the text area's.
            if size.cell.is_none() {
                size.cell = terminal.cell_size();
            }
            if let Some(old) = &easel {
                old.clear(&mut graphics);
            } else if let Some(old) = &stage {
                old.clear(&mut graphics);
            }
            screen.extend(b"\x1b[0m\x1b[2J");
            // A scene goes anew each frame it changes: to Flow packed into
            // whole numbers, half the size, and to other terminals plain.
            stage = Stage::new(size, whole, demo.words()).map(|s| s.made_by("flow-demo").packed(terminal.caps.flow));
            easel = None;
            shown.clear();
            match &stage {
                Some(stage) => {
                    demo.cells(stage.cols, stage.rows);
                    if pictures {
                        easel = Some(Easel::new(stage, terminal.caps.bare, &mut ids));
                    }
                }
                None => {
                    let text = "A bigger window, please: q quits";
                    screen.extend(format!("\x1b[{};{}H{text}", size.rows / 2 + 1, (size.cols as usize).saturating_sub(text.len()) / 2 + 1).as_bytes());
                }
            }
            fresh = false;
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        if let Some(stage) = stage.as_mut() {
            let frame = demo.frame(dt, stage.width(), stage.height());
            match (&frame.show, easel.as_mut()) {
                (Show::Scene(scene), _) => stage.show(scene, &mut graphics),
                (Show::Picture(image), Some(easel)) => easel.show(image, &mut graphics, &mut screen),
                (Show::Picture(_), None) => {}
            }
            if frame.words != shown {
                for (i, line) in frame.words.iter().enumerate() {
                    let col = (size.cols as usize).saturating_sub(line.text.chars().count()) / 2;
                    screen.extend(format!("\x1b[{}H\x1b[0m\x1b[2K\x1b[{}G", stage.below() + i as u16 + 1, col + 1).as_bytes());
                    for (text, color, bold) in line.spans() {
                        let [r, g, b] = [color >> 16, color >> 8, color].map(|c| c & 0xff);
                        let bold = if bold { ";1" } else { "" };
                        screen.extend(format!("\x1b[0;38;2;{r};{g};{b}{bold}m{text}").as_bytes());
                    }
                    screen.extend(b"\x1b[0m");
                }
                shown = frame.words;
            }
            if told.effects {
                for fx in &frame.effects {
                    glass.extend(stage.effect(fx).escape().as_bytes());
                }
            }
        }
        // The glass's wash is Flow's, and isn't the liquid's effects.
        if let Some(wash) = demo.wash().filter(|_| told.flow) {
            glass.extend(wash.as_bytes());
            washed = true;
        }
        if pictures {
            terminal.send(&graphics)?;
        }
        out.write_all(&screen)?;
        out.flush()?;
        if !pictures {
            terminal.send(&graphics)?;
        }
        terminal.send_effects(&glass)?;

        // Keys and clicks until the next frame is due.
        while let Some(event) = input::next_before(now + demo.interval())? {
            match event {
                Event::Key(key) if input::is_quit(&key) => return put_away(easel.as_ref(), stage.as_ref(), washed, terminal),
                Event::Key(key) => demo.key(key.code),
                Event::Resize(..) => fresh = true,
                event => {
                    if let Some(at) = input::left_click(&event).and_then(|(col, row)| stage.as_ref()?.point_at(col, row)) {
                        demo.click(at);
                    }
                }
            }
        }
    }
}

/// Takes the demo's picture or scene away as it quits, and the glass's
/// wash if it washed it.
fn put_away(easel: Option<&Easel>, stage: Option<&Stage>, washed: bool, terminal: &mut Terminal) -> io::Result<()> {
    let mut gone = Vec::new();
    if let Some(easel) = easel {
        easel.clear(&mut gone);
    } else if let Some(stage) = stage {
        stage.clear(&mut gone);
    }
    terminal.send(&gone)?;
    // Leaving the alternate screen would wash it off too.
    if washed {
        terminal.send_effects(UNTINT.as_bytes())?;
    }
    Ok(())
}

/// A demo in the screen's own cells: it writes what changed each frame, its
/// models' commands after (their cells are Flow's to write), then its
/// effects and its sounds, and a key or a click brings the next frame at
/// once.
fn run_text(demo: &mut dyn TextDemo, terminal: &mut Terminal, told: Told) -> io::Result<()> {
    let mut out = io::stdout();
    let mut last = Instant::now();
    // A cell's pixels where the pty doesn't tell them (ConPTY), asked again
    // when the window changes: the cells it was asked for, and the answer.
    let (mut asked_for, mut cell) = (None, None);
    loop {
        // Hung up or told to end: as on q, for a terminal still there to
        // hear it; what the demo made goes as it's dropped.
        if hangup::caught() {
            let _ = terminal.send(demo.quit().graphics.as_bytes());
            return Ok(());
        }
        let mut size = term::size()?;
        if size.cell.is_none() {
            if asked_for != Some((size.cols, size.rows)) {
                asked_for = Some((size.cols, size.rows));
                cell = terminal.cell_size();
            }
            size.cell = cell;
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.25);
        last = now;
        let written = demo.frame(dt, size);
        out.write_all(written.text.as_bytes())?;
        out.flush()?;
        terminal.send(written.graphics.as_bytes())?;
        if told.effects {
            terminal.send_effects(written.glass.concat().as_bytes())?;
        }
        if told.flow {
            terminal.send_effects(written.sounds.concat().as_bytes())?;
        }
        while let Some(event) = input::next_before(now + demo.interval())? {
            match event {
                Event::Key(key) if input::is_quit(&key) => return terminal.send(demo.quit().graphics.as_bytes()),
                Event::Key(key) => {
                    demo.key(key.code);
                    break;
                }
                Event::Resize(..) => break,
                event => {
                    if let Some((col, row)) = input::left_click(&event) {
                        demo.click(col, row);
                        break;
                    }
                }
            }
        }
    }
}
