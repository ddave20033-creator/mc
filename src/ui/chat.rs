//! Chat: message log, text input with history, drawn Minecraft-style in the bottom left.

use crate::ui::{rgba, with_alpha, Color, Ui};
use winit::keyboard::KeyCode;

pub const WHITE: Color = rgba(255, 255, 255, 255);
pub const YELLOW: Color = rgba(255, 255, 85, 255);
pub const GRAY: Color = rgba(170, 170, 170, 255);
pub const RED: Color = rgba(255, 85, 85, 255);

const VISIBLE_SECONDS: f32 = 10.0;
const MAX_MESSAGES: usize = 100;
const MAX_INPUT: usize = 256;

struct Message {
    text: String,
    color: Color,
    time: f32,
}

pub enum ChatInput {
    None,
    Close,
    Submit(String),
}

pub struct Chat {
    pub input: String,
    history: Vec<String>,
    hist_pos: Option<usize>,
    messages: Vec<Message>,
    /// Highlighted suggestion.
    sel: usize,
    /// Input as it was before Tab-completion started (Tab cycles through its options).
    tab_base: Option<String>,
    tab_index: usize,
}

/// Every chat command and its full syntax, shown while typing it and listed by /help.
pub const COMMANDS: &[(&str, &str)] = &[
    ("effect", "/effect give <poison|nausea> [seconds]  |  /effect clear"),
    ("gamemode", "/gamemode <survival|creative|spectator>  (/gm s, /gm c, /gm sp)"),
    ("give", "/give <item> [count]"),
    ("gm", "/gm <survival|creative|spectator>"),
    ("help", "/help"),
    ("kill", "/kill"),
    ("save", "/save"),
    ("seed", "/seed"),
    ("spawn", "/spawn"),
    ("spectate", "/spectate [player]   (spectator mode; no name: stop watching)"),
    ("summon", "/summon <pig|sheep> [x y z]   (~ = your current position)"),
    (
        "time",
        "/time set <day|noon|sunset|night|midnight|sunrise|ticks>  |  /time add <ticks>  |  /time query",
    ),
    ("tp", "/tp <x> <y> <z>   (~ = your current position)"),
];
const TIMES: &[&str] = &["day", "midnight", "night", "noon", "sunrise", "sunset"];

/// Autocomplete information for the text being typed.
struct Suggest {
    /// Byte offset in the input where the current word starts.
    start: usize,
    /// The partial word being typed.
    word: String,
    options: Vec<String>,
    /// Argument placeholder shown when there is nothing to complete, e.g. "<ticks>".
    hint: Option<&'static str>,
    /// Full syntax of the command being typed.
    usage: Option<&'static str>,
}

fn usage(cmd: &str) -> Option<&'static str> {
    COMMANDS.iter().find(|c| c.0 == cmd).map(|c| c.1)
}

fn suggest(input: &str) -> Option<Suggest> {
    let body = input.strip_prefix('/')?;
    let start = input.rfind(' ').map(|i| i + 1).unwrap_or(1);
    let word = input[start..].to_string();
    let words: Vec<&str> = body.split(' ').collect();
    let before = &words[..words.len() - 1];
    let items: Vec<String>;
    let (candidates, hint): (Vec<&str>, Option<&'static str>) = match before {
        [] => (COMMANDS.iter().map(|c| c.0).collect(), None),
        ["give"] => {
            items = crate::item::all_items()
                .into_iter()
                .map(crate::item::key)
                .collect();
            (items.iter().map(|s| s.as_str()).collect(), Some("<item>"))
        }
        ["give", _] => (vec![], Some("[count]")),
        ["time"] => (vec!["add", "query", "set"], None),
        ["time", "set"] => (TIMES.to_vec(), Some("<time>")),
        ["time", "add"] => (vec![], Some("<ticks>")),
        ["gamemode" | "gm"] => (vec!["creative", "spectator", "survival"], None),
        ["summon"] => (vec!["pig", "sheep"], None),
        ["summon", _] => (vec!["~"], Some("[x]")),
        ["summon", _, _] => (vec!["~"], Some("[y]")),
        ["summon", _, _, _] => (vec!["~"], Some("[z]")),
        ["effect"] => (vec!["clear", "give"], None),
        ["effect", "give"] => (vec!["nausea", "poison"], Some("<effect>")),
        ["effect", "give", _] => (vec![], Some("[seconds]")),
        ["tp"] => (vec!["~"], Some("<x>")),
        ["tp", _] => (vec!["~"], Some("<y>")),
        ["tp", _, _] => (vec!["~"], Some("<z>")),
        _ => (vec![], None),
    };
    let options = candidates
        .into_iter()
        .filter(|c| c.starts_with(word.as_str()) && *c != word)
        .map(String::from)
        .collect();
    let usage = words
        .first()
        .filter(|_| !before.is_empty())
        .and_then(|c| usage(c));
    Some(Suggest {
        start,
        word,
        options,
        hint,
        usage,
    })
}

impl Chat {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
            hist_pos: None,
            messages: Vec::new(),
            sel: 0,
            tab_base: None,
            tab_index: 0,
        }
    }

    /// Tab: complete the current word, cycling through the options on repeated presses.
    fn tab_complete(&mut self) {
        let base = self
            .tab_base
            .get_or_insert_with(|| self.input.clone())
            .clone();
        let Some(s) = suggest(&base) else { return };
        if s.options.is_empty() {
            return;
        }
        let pick = s.options[(self.sel + self.tab_index) % s.options.len()].clone();
        self.input = format!("{}{}", &base[..s.start], pick);
        self.tab_index += 1;
    }

    pub fn push(&mut self, text: impl Into<String>, color: Color, now: f32) {
        self.messages.push(Message {
            text: text.into(),
            color,
            time: now,
        });
        if self.messages.len() > MAX_MESSAGES {
            self.messages.remove(0);
        }
    }

    pub fn open(&mut self, prefix: &str) {
        self.input = prefix.to_string();
        self.hist_pos = None;
        self.sel = 0;
        self.tab_base = None;
    }

    /// Handles a key press while the chat is open.
    pub fn key(&mut self, code: KeyCode, text: Option<&str>) -> ChatInput {
        if code == KeyCode::Tab {
            self.tab_complete();
            return ChatInput::None;
        }
        self.tab_base = None;
        self.tab_index = 0;

        // Arrow keys browse suggestions while a list is shown, history otherwise.
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            if let Some(s) = suggest(&self.input).filter(|s| s.options.len() > 1) {
                let n = s.options.len();
                self.sel = if code == KeyCode::ArrowUp {
                    (self.sel + n - 1) % n
                } else {
                    (self.sel + 1) % n
                };
                return ChatInput::None;
            }
        }
        if !matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.sel = 0;
        }
        match code {
            KeyCode::Escape => return ChatInput::Close,
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let line = std::mem::take(&mut self.input);
                let line = line.trim().to_string();
                if line.is_empty() {
                    return ChatInput::Close;
                }
                if self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                }
                return ChatInput::Submit(line);
            }
            KeyCode::Backspace => {
                self.input.pop();
                return ChatInput::None;
            }
            KeyCode::ArrowUp | KeyCode::ArrowDown => {
                if self.history.is_empty() {
                    return ChatInput::None;
                }
                let last = self.history.len() - 1;
                self.hist_pos = match (code, self.hist_pos) {
                    (KeyCode::ArrowUp, None) => Some(last),
                    (KeyCode::ArrowUp, Some(i)) => Some(i.saturating_sub(1)),
                    (_, Some(i)) if i < last => Some(i + 1),
                    _ => None,
                };
                self.input = self
                    .hist_pos
                    .map(|i| self.history[i].clone())
                    .unwrap_or_default();
                return ChatInput::None;
            }
            _ => {}
        }
        if let Some(t) = text {
            for ch in t.chars() {
                if !ch.is_control() && self.input.chars().count() < MAX_INPUT {
                    self.input.push(ch);
                }
            }
        }
        ChatInput::None
    }

    pub fn draw(&self, ui: &mut Ui, now: f32, open: bool) {
        let s = ui.s;
        let fs = (s - 1.0).max(1.0);
        let line_h = 9.0 * fs;
        let width = (ui.w * 0.5).min(320.0 * fs);
        let bottom = ui.h - if open { 18.0 * s } else { 44.0 * s };

        // Newest messages at the bottom.
        let mut rows: Vec<(String, Color, f32)> = Vec::new();
        for m in self.messages.iter().rev() {
            let age = now - m.time;
            let alpha = if open {
                1.0
            } else {
                ((VISIBLE_SECONDS - age) / 1.0).clamp(0.0, 1.0)
            };
            if alpha <= 0.0 {
                continue;
            }
            for l in ui.wrap(&m.text, width - 4.0 * fs, fs).into_iter().rev() {
                rows.push((l, m.color, alpha));
            }
            if rows.len() >= if open { 20 } else { 10 } {
                break;
            }
        }
        for (i, (text, color, alpha)) in rows.iter().enumerate() {
            let y = (bottom - (i + 1) as f32 * line_h).round();
            ui.solid(
                2.0 * s,
                y,
                width,
                line_h,
                rgba(0, 0, 0, (110.0 * alpha) as u8),
            );
            ui.text(
                text,
                2.0 * s + 2.0 * fs,
                y + fs,
                fs,
                with_alpha(*color, *alpha),
                true,
            );
        }

        if open {
            let y = ui.h - 14.0 * s;
            ui.solid(2.0 * s, y, ui.w - 4.0 * s, 12.0 * s, rgba(0, 0, 0, 150));
            let tx = 2.0 * s + 3.0 * fs;
            let ty = (y + (12.0 * s - 7.0 * fs) * 0.5).round();
            let tw = ui.text(&self.input, tx, ty, fs, WHITE, true);
            let end = tx + tw + if self.input.is_empty() { 0.0 } else { fs };

            // Command help: while tab-cycling show the list the cycle came from.
            let (sug, highlighted) = match &self.tab_base {
                Some(base) => match suggest(base) {
                    Some(s) if !s.options.is_empty() => {
                        let i = (self.sel + self.tab_index + s.options.len() - 1) % s.options.len();
                        (Some(s), i)
                    }
                    _ => (None, 0),
                },
                None => (suggest(&self.input), self.sel),
            };
            if let Some(s) = &sug {
                if self.tab_base.is_none() {
                    if let Some(opt) = s.options.get(highlighted % s.options.len().max(1)) {
                        // Faded remainder of the highlighted completion.
                        let rest = &opt[s.word.len().min(opt.len())..];
                        ui.text(rest, end, ty, fs, rgba(120, 120, 120, 255), false);
                    } else if let (Some(h), true) = (s.hint, s.word.is_empty()) {
                        ui.text(h, end, ty, fs, rgba(120, 120, 120, 255), false);
                    }
                }
                if !s.options.is_empty() {
                    let lx = (tx + ui.text_width(&self.input[..s.start.min(self.input.len())], fs))
                        .round();
                    let rows = s.options.len().min(10);
                    let bw = s
                        .options
                        .iter()
                        .map(|o| ui.text_width(o, fs))
                        .fold(0.0, f32::max)
                        + 6.0 * fs;
                    for (i, opt) in s.options.iter().take(rows).enumerate() {
                        let ry = (y - (rows - i) as f32 * line_h - fs).round();
                        ui.solid(lx - 2.0 * fs, ry, bw, line_h, rgba(0, 0, 0, 200));
                        let c = if i == highlighted % s.options.len() {
                            YELLOW
                        } else {
                            GRAY
                        };
                        ui.text(opt, lx, ry + fs, fs, c, true);
                    }
                } else if let Some(u) = s.usage {
                    let ry = (y - line_h - fs).round();
                    let uw = ui.text_width(u, fs) + 6.0 * fs;
                    ui.solid(tx - 2.0 * fs, ry, uw, line_h, rgba(0, 0, 0, 200));
                    ui.text(u, tx, ry + fs, fs, GRAY, true);
                }
            }
            if (now * 2.5) as i32 % 2 == 0 {
                ui.text("_", end, ty, fs, WHITE, true);
            }
        }
    }
}
