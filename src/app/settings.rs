//! The options, kept in `options.txt` next to the game: the view, the controls (with the key
//! binds, `keys`), the language, the resource packs and the rest of the options screens.

use crate::app::keys::{KeyMap, BINDS};

const PATH: &str = "options.txt";

#[derive(Clone)]
pub struct Settings {
    pub fov: f32,
    /// Percent, 100 = default.
    pub sensitivity: f32,
    /// In chunks.
    pub render_distance: f32,
    /// 0 = auto.
    pub gui_scale: u32,
    pub fullscreen: bool,
    /// Frames per second at most (0 = no limit).
    pub fps_limit: u32,
    /// Anti-aliasing: samples per pixel (2, 4, 8; lowered to what the GPU supports).
    pub msaa: u32,
    pub show_fps: bool,
    pub shadows: bool,
    pub clouds: bool,
    pub view_bobbing: bool,
    /// Show the player's own body when looking down in first person.
    pub first_person_body: bool,
    pub hungarian: bool,
    /// Dark theme for the item screens.
    pub dark_ui: bool,
    /// Enabled packs from `resourcepacks/`, highest priority first. The built-in pack is
    /// always under them (see `resource_pack::Packs`).
    pub resource_packs: Vec<String>,
    /// Player name shown to others in LAN games.
    pub name: String,
    pub keys: KeyMap,
    /// Volumes in percent: overall, weapons (guns, grenades), everything else.
    pub volume: f32,
    pub volume_weapons: f32,
    pub volume_other: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fov: 70.0,
            sensitivity: 100.0,
            render_distance: 12.0,
            gui_scale: 0,
            fullscreen: false,
            fps_limit: 144,
            msaa: 2,
            show_fps: false,
            shadows: true,
            clouds: true,
            view_bobbing: true,
            first_person_body: true,
            hungarian: true,
            dark_ui: false,
            resource_packs: Vec::new(),
            name: default_name(),
            keys: KeyMap::default(),
            volume: 80.0,
            volume_weapons: 100.0,
            volume_other: 100.0,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        if let Ok(text) = std::fs::read_to_string(PATH) {
            for line in text.lines() {
                let Some((k, v)) = line.split_once(':') else {
                    continue;
                };
                let v = v.trim();
                let b = v == "true";
                match k.trim() {
                    "fov" => s.fov = v.parse().unwrap_or(s.fov),
                    "sensitivity" => s.sensitivity = v.parse().unwrap_or(s.sensitivity),
                    "render_distance" => s.render_distance = v.parse().unwrap_or(s.render_distance),
                    "gui_scale" => s.gui_scale = v.parse().unwrap_or(s.gui_scale),
                    "fullscreen" => s.fullscreen = b,
                    "fps_limit" => s.fps_limit = v.parse().unwrap_or(s.fps_limit),
                    "antialiasing" => s.msaa = v.parse().unwrap_or(s.msaa),
                    "show_fps" => s.show_fps = b,
                    "shadows" => s.shadows = b,
                    "clouds" => s.clouds = b,
                    "view_bobbing" => s.view_bobbing = b,
                    "first_person_body" => s.first_person_body = b,
                    "language" => s.hungarian = v == "hu",
                    "dark_ui" => s.dark_ui = b,
                    "name" if !v.is_empty() => s.name = v.chars().take(16).collect(),
                    "volume" => s.volume = v.parse().unwrap_or(s.volume),
                    "volume_weapons" => s.volume_weapons = v.parse().unwrap_or(s.volume_weapons),
                    "volume_other" => s.volume_other = v.parse().unwrap_or(s.volume_other),
                    // `|` cannot be in a Windows file name.
                    "resource_packs" => {
                        s.resource_packs = v
                            .split('|')
                            .filter(|n| !n.is_empty())
                            .map(String::from)
                            .collect()
                    }
                    k => {
                        let bind = k.strip_prefix("key_").and_then(|n| {
                            BINDS.iter().position(|(_, name, _)| *name == n)
                        });
                        if let (Some(i), Some(code)) = (bind, crate::app::keys::parse(v)) {
                            s.keys.0[i] = code;
                        }
                    }
                }
            }
        }
        // Packs removed from the folder since.
        let available = crate::textures::resource_pack::list();
        s.resource_packs.retain(|n| available.contains(n));
        s.fov = s.fov.clamp(30.0, 110.0);
        s.sensitivity = s.sensitivity.clamp(10.0, 200.0);
        s.render_distance = s.render_distance.clamp(4.0, 64.0);
        s.gui_scale = s.gui_scale.min(6);
        for v in [&mut s.volume, &mut s.volume_weapons, &mut s.volume_other] {
            *v = v.clamp(0.0, 100.0);
        }
        if s.fps_limit != 0 {
            s.fps_limit = s.fps_limit.clamp(30, 250);
        }
        // Always on: 2x at least.
        s.msaa = [2, 4, 8].into_iter().rfind(|&n| n <= s.msaa).unwrap_or(2);
        crate::app::lang::set_hungarian(s.hungarian);
        s
    }

    pub fn save(&self) {
        let mut text = format!(
            "fov:{}\nsensitivity:{}\nrender_distance:{}\ngui_scale:{}\nfullscreen:{}\nfps_limit:{}\nantialiasing:{}\nshow_fps:{}\n\
             shadows:{}\nclouds:{}\nview_bobbing:{}\nfirst_person_body:{}\nlanguage:{}\ndark_ui:{}\nresource_packs:{}\nname:{}\n\
             volume:{}\nvolume_weapons:{}\nvolume_other:{}\n",
            self.fov,
            self.sensitivity,
            self.render_distance,
            self.gui_scale,
            self.fullscreen,
            self.fps_limit,
            self.msaa,
            self.show_fps,
            self.shadows,
            self.clouds,
            self.view_bobbing,
            self.first_person_body,
            if self.hungarian { "hu" } else { "en" },
            self.dark_ui,
            self.resource_packs.join("|"),
            self.name,
            self.volume,
            self.volume_weapons,
            self.volume_other
        );
        for (i, (_, name, _)) in BINDS.iter().enumerate() {
            text += &format!("key_{name}:{}\n", crate::app::keys::code_name(self.keys.0[i]));
        }
        let _ = std::fs::write(PATH, text);
    }

    /// Largest scale that still fits a 320x240 GUI, like Minecraft's "Auto".
    pub fn max_gui_scale(w: f32, h: f32) -> u32 {
        ((w / 320.0).min(h / 240.0).floor() as u32).max(1)
    }

    pub fn effective_gui_scale(&self, w: f32, h: f32) -> f32 {
        let max = Self::max_gui_scale(w, h);
        (if self.gui_scale == 0 {
            max
        } else {
            self.gui_scale.min(max)
        }) as f32
    }
}

/// The Windows user name (or "Player"), cleaned up for a player name.
fn default_name() -> String {
    let name: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(16)
        .collect();
    if name.is_empty() {
        "Player".into()
    } else {
        name
    }
}
