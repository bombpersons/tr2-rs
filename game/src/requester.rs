//! The original's requester (`0x800355C8`): a heading over a few lines of a
//! list, one of them chosen, moved through with Up and Down and picked with
//! Cross. The passport's load list draws it in a panel with boxes; the
//! pause's questions draw it bare but for the chosen line's box.
//!
//! Positions are the original's: the list is centred and measured up from
//! the screen's bottom (`y`), its lines `line_height` apart. Evidence:
//! `docs/save-evidence.md`.
use bevy::prelude::*;

use crate::{
    inventory::ring::input,
    text::{Align, BoxStyle, ScreenText, TextBox, symbol},
};

/// A list to choose from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requester {
    pub heading: String,
    pub items: Vec<String>,
    /// The chosen line (`+0x4`) and the first one shown (`+0x8`).
    pub selected: usize,
    pub top: usize,
    /// How many lines show at once (`+0x6`).
    pub visible: usize,
    /// The panel's width (`+0xC`) and the gap between lines (`+0xE`).
    pub width: i32,
    pub line_height: i32,
    /// The list's bottom, up from the screen's (`+0x12`).
    pub y: i32,
    /// Drawn in its panel with a boxed heading (the passport's third
    /// argument), or bare.
    pub boxed: bool,
    /// Triangle leaves it (the passport's second argument).
    pub cancel: bool,
}

/// What a frame of the requester decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Chosen(usize),
    Cancelled,
}

impl Requester {
    /// The passport's lists (`0x8006EE18`, `0x8006F010`): five lines 18
    /// apart in a panel 296 wide, 44 above the bottom.
    pub fn passport(heading: impl Into<String>, items: Vec<String>, selected: usize) -> Self {
        let visible = 5;
        // The chosen line starts in view (`0x8006E6A0`).
        let top = (selected + 1).saturating_sub(visible);
        Self {
            heading: heading.into(),
            items,
            selected,
            top,
            visible,
            width: 296,
            line_height: 18,
            y: -44,
            boxed: true,
            cancel: true,
        }
    }

    /// The pause's questions (`0x80037118`, `0x80037298`): two lines 15
    /// apart, 80 wide, 44 above the bottom.
    pub fn question(heading: impl Into<String>, items: [&str; 2], selected: usize) -> Self {
        Self {
            heading: heading.into(),
            items: items.map(String::from).to_vec(),
            selected,
            top: 0,
            visible: 2,
            width: 80,
            line_height: 15,
            y: -44,
            boxed: false,
            cancel: false,
        }
    }

    /// One frame on the ring's once-only input word: Down, else Up, else
    /// Cross or (when allowed) Triangle.
    pub fn input(&mut self, db: u32) -> Option<Answer> {
        if db & input::BACK != 0 {
            if self.selected + 1 < self.items.len() {
                self.selected += 1;
            }
            if self.selected > self.top + self.visible - 1 {
                self.top += 1;
            }
        } else if db & input::FORWARD != 0 {
            self.selected = self.selected.saturating_sub(1);
            if self.selected < self.top {
                self.top -= 1;
            }
        } else if db & input::SELECT != 0 {
            return Some(Answer::Chosen(self.selected));
        } else if db & input::DESELECT != 0 && self.cancel {
            return Some(Answer::Cancelled);
        }
        None
    }

    /// Its lines: the heading 10 above the list, the panel behind both,
    /// the shown lines with the chosen one boxed, and arrows when more lie
    /// above or below.
    pub fn lines(&self) -> Vec<ScreenText> {
        let line = |text: &str, y: i32| ScreenText::new(text, IVec2::new(0, y), Align::BOTTOM);
        let list_top = self.y - (self.visible as i32 * self.line_height + 10);
        let mut lines = vec![];
        if self.boxed {
            let height = self.line_height + self.visible as i32 * self.line_height + 10 + 12;
            lines.push(line(" ", list_top - self.line_height - 12).with_box(TextBox {
                width: self.width,
                height: Some(height),
                style: BoxStyle::Panel,
            }));
        }
        let mut heading = line(&self.heading, list_top - self.line_height - 10);
        if self.boxed {
            heading = heading.with_box(TextBox { width: self.width - 4, height: None, style: BoxStyle::Heading });
        }
        lines.push(heading);
        for (row, index) in (self.top..self.items.len()).take(self.visible).enumerate() {
            let mut item = line(&self.items[index], list_top + row as i32 * self.line_height);
            if index == self.selected {
                item = item.with_box(TextBox { width: self.width - 12, height: None, style: BoxStyle::Selected });
            }
            lines.push(item);
        }
        let arrow = |code: char, y: i32| ScreenText::new(format!(" {code}"), IVec2::new(-10, y), Align::BOTTOM);
        if self.top != 0 {
            lines.push(arrow(symbol::UP, list_top - self.line_height + 4));
        }
        if self.top + self.visible < self.items.len() {
            lines.push(arrow(symbol::DOWN, self.y - 14));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(n: usize) -> Requester {
        Requester::passport("Load Game", (0..n).map(|i| format!("{i}")).collect(), 0)
    }

    /// Down stops at the last line and scrolls to keep the chosen one in
    /// view; Up likewise at the first.
    #[test]
    fn moving_through_a_long_list_scrolls() {
        let mut r = list(7);
        for _ in 0..6 {
            r.input(input::BACK);
        }
        assert_eq!((r.selected, r.top), (6, 2));
        r.input(input::BACK);
        assert_eq!((r.selected, r.top), (6, 2));
        for _ in 0..5 {
            r.input(input::FORWARD);
        }
        assert_eq!((r.selected, r.top), (1, 1));
        assert_eq!(r.input(input::SELECT), Some(Answer::Chosen(1)));
        assert_eq!(r.input(input::DESELECT), Some(Answer::Cancelled));
    }

    /// The load list of `validation/save/title_load`: the heading's
    /// baseline 68 down the screen, the lines' 96, 114 and 132, the panel
    /// at 51..181 and 300 wide, as the GPU capture draws them.
    #[test]
    fn the_load_list_lies_where_the_original_draws_it() {
        let r = Requester::passport(
            "Load Game",
            vec!["The Great Wall".into(), "Current Position".into(), "Story so far...".into()],
            1,
        );
        let lines = r.lines();
        let baseline = |l: &ScreenText| 240 + l.at.y;
        assert_eq!(lines.iter().map(baseline).collect::<Vec<_>>(), [66, 68, 96, 114, 132]);
        let panel = lines[0].boxed.unwrap().rect(crate::text::width(" "));
        let start = Align::BOTTOM.start(lines[0].at, crate::text::width(" "));
        assert_eq!(panel.min + start, IVec2::new(34, 51));
        assert_eq!(panel.max + start, IVec2::new(334, 181));
        let chosen = &lines[3];
        let rect = chosen.boxed.unwrap().rect(crate::text::width(&chosen.text));
        let start = Align::BOTTOM.start(chosen.at, crate::text::width(&chosen.text));
        assert_eq!((rect.min + start, rect.max + start), (IVec2::new(40, 99), IVec2::new(328, 115)));
    }
}
