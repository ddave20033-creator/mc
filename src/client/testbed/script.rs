//! The testbed's scripts: one command a line, `#` starts a comment. See `testbed/README.md`
//! for every command.

/// Where a position is measured from.
#[derive(Clone, Copy, Debug)]
pub enum Origin {
    /// The test's origin (the lane's start, or the spawn).
    Rel,
    /// World coordinates.
    Abs,
}

#[derive(Clone, Debug)]
pub enum Cmd {
    /// `world bench|new <seed>|menu`: which world (the first line; a copy in the temp
    /// folder, never a real save).
    World(WorldKind),
    /// `window <w> <h>`: the window's size in pixels (the pictures are this big).
    Window(u32, u32),
    /// `lane`: a flat stone lane from the origin east, cleared above, a post at its end.
    Lane,
    /// `clear <radius> <height>`: air around the origin.
    Clear(i32, i32),
    /// `time <0..1>`: the time of day (held there).
    Time(f32),
    /// `pos [abs] <x> <y> <z>`, `look <yaw°> <pitch°>`, `fly on|off`.
    Pos(Origin, [f32; 3]),
    Look(f32, f32),
    Fly(bool),
    /// `camera fp|back|front|side|side_left|fixed`.
    Camera(u8),
    /// `hold <item> [count] [loaded]`: the item in the selected slot (a gun loaded full).
    Hold(String, u32, bool),
    /// `slot <0..8>`: the selected hotbar slot; `empty`: the inventory emptied, slot 0.
    Slot(usize),
    Empty,
    /// `give <item> [count]`, `cmd <chat command>`.
    Command(String),
    /// `place [abs] <x> <y> <z> <block>`, `fill <x0 y0 z0> <x1 y1 z1> <block>` (relative).
    Place(Origin, [i32; 3], String),
    Fill([i32; 3], [i32; 3], String),
    /// `tree <oak|birch|spruce> <x> <z> [seed]`: a tree standing on the ground there.
    Tree(String, i32, i32, u32),
    /// `drop <item> <x> <y> <z>`: an item lying there.
    Drop(String, [f32; 3]),
    /// `key <name> <secs>`: held down (a key name like `W`, `Space`, `ShiftLeft`, or a bind:
    /// `forward`, `jump`, `sneak`, `sprint`, `reload`, `inspect`...); `press <name>`: once.
    Key(String, f32),
    Press(String),
    /// `click left|right [secs]`: a mouse button (held that long).
    Click(bool, f32),
    /// `mouse <x> <y>`: the mouse over the UI, as fractions of the window.
    Mouse(f32, f32),
    /// `uikey <name>`: a key for the menus (W/S, Space, Q/E... see `Ui::nav_key`).
    UiKey(String),
    /// `screen <name>`: a screen opened (main, worlds, create, delete, pause, options,
    /// options_game, keys, packs, credits, multi, dead, skin, inventory, creative, playing).
    Screen(String),
    /// `turn <deg/s> <secs>`: the view turning.
    Turn(f32, f32),
    /// `set <name> <value>`: body on|off, blur on|off, hud on|off, fov <deg>, gui <scale>.
    Set(String, String),
    /// `wait <secs>`, `frames <n>`.
    Wait(f32),
    Frames(u32),
    /// `shot <name>`: a picture of the frame; `shots <name> <count> <every secs>`.
    Shot(String),
    /// `check zfight [chunks]`, `check models`, `check textures`.
    Check(String, i32),
    /// `pickmap <name>`: the open gun station's click map.
    PickMap(String),
    /// `echo <text>`: a line in the report.
    Echo(String),
    /// `stats`: frame rate, chunks, meshes and memory, a line in the report.
    Stats,
    /// `lan open`, `lan join <addr> <name>`, `lan report`, `lan block <x> <y> <z>`: a LAN
    /// game (two windows, each with its script).
    Lan(String),
    // Steps the others are made of: `flicker <name>` is two pictures of the view a hair
    // apart (`Jitter`), compared (`Compare`).
    Jitter(bool),
    Compare(String, String, String),
    Quit,
}

#[derive(Clone, Copy, Debug)]
pub enum WorldKind {
    Bench,
    New(u32),
    Menu,
}

/// The commands of a script, and what could not be read (line, why).
pub fn parse(text: &str) -> (Vec<Cmd>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(cmds) => out.extend(cmds),
            Err(e) => errors.push(format!("line {}: `{line}`: {e}", n + 1)),
        }
    }
    (out, errors)
}

fn parse_line(line: &str) -> Result<Vec<Cmd>, String> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let f = |i: usize| -> Result<f32, String> {
        w.get(i).ok_or(format!("missing number {i}"))?.parse::<f32>().map_err(|_| format!("`{}` is not a number", w[i]))
    };
    let int = |i: usize| f(i).map(|v| v as i32);
    let on = |i: usize| -> Result<bool, String> {
        match w.get(i).copied() {
            Some("on" | "true" | "1") => Ok(true),
            Some("off" | "false" | "0") => Ok(false),
            _ => Err("expected on or off".into()),
        }
    };
    let word = |i: usize| -> Result<String, String> { w.get(i).map(|s| s.to_string()).ok_or(format!("missing word {i}")) };
    let rest = |i: usize| w.get(i..).map(|r| r.join(" ")).unwrap_or_default();
    // `abs` after the command: world coordinates.
    let (origin, a) = if w.get(1) == Some(&"abs") { (Origin::Abs, 2) } else { (Origin::Rel, 1) };
    let one = |c: Cmd| Ok(vec![c]);
    match w[0] {
        "world" => one(Cmd::World(match w.get(1).copied() {
            Some("bench") | None => WorldKind::Bench,
            Some("new") => WorldKind::New(f(2).unwrap_or(12345.0) as u32),
            Some("menu") => WorldKind::Menu,
            Some(o) => return Err(format!("unknown world `{o}`")),
        })),
        "window" => one(Cmd::Window(f(1)? as u32, f(2)? as u32)),
        "lane" => one(Cmd::Lane),
        "clear" => one(Cmd::Clear(int(1)?, int(2).unwrap_or(8))),
        "time" => one(Cmd::Time(f(1)?)),
        "pos" => one(Cmd::Pos(origin, [f(a)?, f(a + 1)?, f(a + 2)?])),
        "look" => one(Cmd::Look(f(1)?, f(2).unwrap_or(0.0))),
        "fly" => one(Cmd::Fly(on(1)?)),
        "camera" => one(Cmd::Camera(match w.get(1).copied() {
            Some("fp") => 0,
            Some("back") => 1,
            Some("front") => 2,
            Some("side") => crate::client::player::camera::SIDE_VIEW,
            Some("fixed") => crate::client::player::camera::FIXED_FRONT,
            Some("side_left") => crate::client::player::camera::SIDE_LEFT,
            o => return Err(format!("unknown camera {o:?}")),
        })),
        "hold" => one(Cmd::Hold(word(1)?, f(2).map(|v| v as u32).unwrap_or(1), w.contains(&"loaded"))),
        "slot" => one(Cmd::Slot(int(1)?.clamp(0, 8) as usize)),
        "empty" => one(Cmd::Empty),
        "give" => one(Cmd::Command(format!("give {}", rest(1)))),
        "cmd" => one(Cmd::Command(rest(1).trim_start_matches('/').to_string())),
        "place" => one(Cmd::Place(origin, [int(a)?, int(a + 1)?, int(a + 2)?], word(a + 3)?)),
        "fill" => one(Cmd::Fill([int(1)?, int(2)?, int(3)?], [int(4)?, int(5)?, int(6)?], word(7)?)),
        "tree" => one(Cmd::Tree(word(1)?, int(2)?, int(3)?, f(4).map(|v| v as u32).unwrap_or(1))),
        "drop" => one(Cmd::Drop(word(1)?, [f(2)?, f(3)?, f(4)?])),
        "key" => one(Cmd::Key(word(1)?, f(2)?)),
        "press" => one(Cmd::Press(word(1)?)),
        "click" => one(Cmd::Click(w.get(1) != Some(&"right"), f(2).unwrap_or(0.0))),
        "mouse" => one(Cmd::Mouse(f(1)?, f(2)?)),
        "uikey" => one(Cmd::UiKey(word(1)?)),
        "screen" => one(Cmd::Screen(word(1)?)),
        "turn" => one(Cmd::Turn(f(1)?, f(2)?)),
        "set" => one(Cmd::Set(word(1)?, rest(2))),
        "wait" => one(Cmd::Wait(f(1)?)),
        "frames" => one(Cmd::Frames(int(1)?.max(1) as u32)),
        "shot" => one(Cmd::Shot(word(1)?)),
        "shots" => {
            let (name, n, every) = (word(1)?, int(2)?.max(1), f(3)?);
            let mut v = Vec::new();
            for i in 0..n {
                if i > 0 {
                    v.push(Cmd::Wait(every));
                }
                v.push(Cmd::Shot(format!("{name}_{i:02}")));
            }
            Ok(v)
        }
        "flicker" => {
            let name = word(1)?;
            let (a, b) = (format!("{name}_a"), format!("{name}_b"));
            Ok(vec![
                Cmd::Shot(a.clone()),
                Cmd::Jitter(true),
                Cmd::Shot(b.clone()),
                Cmd::Jitter(false),
                Cmd::Frames(2),
                Cmd::Compare(a, b, format!("{name}_diff")),
            ])
        }
        "check" => one(Cmd::Check(word(1)?, int(2).unwrap_or(2))),
        "pickmap" => one(Cmd::PickMap(word(1)?)),
        "echo" => one(Cmd::Echo(rest(1))),
        "stats" => one(Cmd::Stats),
        "lan" => one(Cmd::Lan(rest(1))),
        "quit" => one(Cmd::Quit),
        other => Err(format!("unknown command `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_script_reads() {
        for (name, text) in super::super::BUILT_IN {
            let (cmds, errors) = parse(text);
            assert!(errors.is_empty(), "{name}: {errors:?}");
            assert!(!cmds.is_empty(), "{name}");
        }
    }

    #[test]
    fn bursts_and_flicker_expand() {
        let (cmds, errors) = parse("shots walk 3 0.1 # three\nflicker grass");
        assert!(errors.is_empty());
        assert_eq!(cmds.iter().filter(|c| matches!(c, Cmd::Shot(_))).count(), 5);
        assert!(matches!(cmds.last(), Some(Cmd::Compare(..))));
        assert!(!parse("jump high").1.is_empty());
    }
}
