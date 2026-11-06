//! A picture over a stage's cells, sent with kitty's graphics protocol
//! (`flow_sdk::picture`): each frame's picture goes again under the same
//! id, and the terminal shows it in the last one's place, while the cells
//! that show it (Unicode placeholders, which carry the id) are written once,
//! when it's laid out. So the picture scrolls, clears and rides Flow's
//! liquid as text does. Where cells go bare (on Windows, through Flow's
//! pipe) a cell can't say its row, so each row of the picture is an image
//! of its own, all sent each frame.
//!
//! The SDK sends pictures and writes their cells; this keeps a stage's
//! worth of them in step from frame to frame.

use flow_sdk::picture::{self, Ids, Image, PLACEMENT};
use flow_sdk::stage::Stage;

pub struct Easel {
    left: u16,
    top: u16,
    cols: u16,
    rows: u16,
    bare: bool,
    /// The picture's id, or one a row where cells go bare.
    ids: Vec<u32>,
    placed: bool,
    /// The last picture sent, which a still one needn't send again.
    last: Option<Image>,
}

impl Easel {
    /// Over `stage`'s cells, its ids from `ids`; `bare` as the terminal's
    /// [`flow_sdk::Caps::bare`] says.
    pub fn new(stage: &Stage, bare: bool, ids: &mut Ids) -> Self {
        let count = if bare { stage.rows } else { 1 };
        Self { left: stage.left, top: stage.top, cols: stage.cols, rows: stage.rows, bare, ids: (0..count).map(|_| ids.next_id()).collect(), placed: false, last: None }
    }

    /// Sends `image` (`graphics`, which go first) unless it's the last one
    /// again, and the first time writes the cells that show it (`cells`).
    pub fn show(&mut self, image: &Image, graphics: &mut Vec<u8>, cells: &mut Vec<u8>) {
        if self.last.as_ref() == Some(image) {
            return;
        }
        if self.bare {
            for (row, &id) in self.ids.iter().enumerate() {
                if let Some(strip) = image.strip(row as u16, self.rows) {
                    graphics.extend(picture::upload(id, &strip, PLACEMENT, self.cols, 1).as_bytes());
                }
            }
        } else {
            graphics.extend(picture::upload(self.ids[0], image, PLACEMENT, self.cols, self.rows).as_bytes());
        }
        self.last = Some(image.clone());
        if !self.placed {
            self.placed = true;
            if self.bare {
                for (row, &id) in self.ids.iter().enumerate() {
                    cells.extend(format!("\x1b[{};{}H{}", self.top + row as u16 + 1, self.left + 1, picture::bare_row(id, self.cols)).as_bytes());
                }
            } else {
                cells.extend(picture::placeholders(self.ids[0], PLACEMENT, (self.left, self.top), self.cols, self.rows).as_bytes());
            }
        }
    }

    /// Forgets the pictures, their pixels and all.
    pub fn clear(&self, graphics: &mut Vec<u8>) {
        for &id in &self.ids {
            graphics.extend(picture::delete(id).as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_sdk::term::Size;

    fn stage() -> Stage {
        Stage::new(Size { cols: 80, rows: 24, cell: Some((9.5, 19.0)) }, Some((9, 19)), 2).expect("big enough")
    }

    #[test]
    fn a_picture_goes_each_frame_and_its_cells_once() {
        let stage = stage();
        let mut easel = Easel::new(&stage, false, &mut Ids::after(0x20_0000));
        let image = Image::rgba(16, 8, vec![200; 16 * 8 * 4]);
        let (mut graphics, mut cells) = (Vec::new(), Vec::new());
        easel.show(&image, &mut graphics, &mut cells);
        let sent = String::from_utf8(graphics).unwrap();
        assert!(sent.starts_with(&format!("\x1b_Ga=T,U=1,f=32,o=z,s=16,v=8,i={},p=1,c={},r={},q=2,", 0x20_0001, stage.cols, stage.rows)));
        let cells = String::from_utf8(cells).unwrap();
        assert_eq!(cells.matches(picture::PLACEHOLDER).count(), usize::from(stage.cols) * usize::from(stage.rows));
        assert!(cells.starts_with(&format!("\x1b[{};{}H", stage.top + 1, stage.left + 1)));

        let (mut graphics, mut cells) = (Vec::new(), Vec::new());
        easel.show(&image, &mut graphics, &mut cells);
        assert!(graphics.is_empty() && cells.is_empty(), "the same picture isn't sent again");
        let other = Image::rgba(16, 8, vec![100; 16 * 8 * 4]);
        easel.show(&other, &mut graphics, &mut cells);
        assert!(!graphics.is_empty() && cells.is_empty(), "a new one takes the old one's place, under the same cells");
        let mut gone = Vec::new();
        easel.clear(&mut gone);
        assert_eq!(gone, format!("\x1b_Ga=d,d=I,i={},q=2\x1b\\", 0x20_0001).into_bytes());
    }

    #[test]
    fn bare_cells_take_a_picture_a_row() {
        let stage = stage();
        let mut easel = Easel::new(&stage, true, &mut Ids::after(0x30_0000));
        let rows = u32::from(stage.rows);
        let image = Image::rgba(20, rows * 3, vec![50; 20 * rows as usize * 3 * 4]);
        let (mut graphics, mut cells) = (Vec::new(), Vec::new());
        easel.show(&image, &mut graphics, &mut cells);
        let sent = String::from_utf8(graphics).unwrap();
        assert_eq!(sent.matches("a=T,U=1").count(), usize::from(stage.rows));
        assert!(sent.contains(&format!("s=20,v=3,i={},p=1,c={},r=1,", 0x30_0002, stage.cols)), "each row three pixels high, the second's id the next");
        let cells = String::from_utf8(cells).unwrap();
        assert!(cells.contains(&picture::bare_row(0x30_0001, stage.cols)));
        assert!(!cells.contains('\u{0305}'), "no marks");
    }
}
