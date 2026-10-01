//! The menus' state (`Menus`) and going between the screens: pausing and resuming, going
//! back, and what the buttons of a screen ask for (`apply`).

use super::*;

/// The menus' state: the world list and the new world's settings, the multiplayer screen,
/// the options, the resource packs, and the title screen's player and panorama.
pub(super) struct Menus {
    pub(super) worlds: Vec<WorldMeta>,
    pub(super) selected_world: Option<usize>,
    pub(super) world_scroll: f32,
    pub(super) last_click: (usize, f32),
    pub(super) create_name: String,
    pub(super) create_seed: String,
    pub(super) create_creative: bool,
    pub(super) create_cheats: bool,
    pub(super) finder: Option<crate::net::Finder>,
    pub(super) mp_address: String,
    pub(super) mp_selected: Option<std::net::SocketAddr>,
    /// Connecting to a LAN game (on a thread of its own, the window going on meanwhile): its
    /// address, and the connection when it is made.
    pub(super) joining: Option<(String, std::sync::mpsc::Receiver<std::io::Result<crate::net::Conn>>)>,
    pub(super) net_message: String,
    /// Open tab of the options screen.
    pub(super) options: screens::OptionsState,
    pub(super) pack_screen: screens::PackScreen,
    pub(super) menu_preview: screens::PreviewRotation,
    pub(super) skin_error: String,
    /// Where the title screen's camera is (over the world last played, or a throwaway one).
    pub(super) pano: Vec3,
}

impl Menus {
    pub(super) fn new(pano: Vec3) -> Self {
        Self {
            worlds: Vec::new(),
            selected_world: None,
            world_scroll: 0.0,
            last_click: (usize::MAX, -10.0),
            create_name: String::new(),
            create_seed: String::new(),
            create_creative: false,
            create_cheats: false,
            finder: None,
            mp_address: String::new(),
            mp_selected: None,
            joining: None,
            net_message: String::new(),
            options: Default::default(),
            pack_screen: Default::default(),
            menu_preview: Default::default(),
            skin_error: String::new(),
            pano,
        }
    }
}

/// Opens a web page in the default browser.
fn open_url(url: &str) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}

impl Game {
    pub(super) fn pause(&mut self) {
        self.screen = Screen::Paused;
        self.set_grab(false);
        if self.session.local.is_some() {
            self.send(crate::net::Msg::Pause(true));
        }
    }

    pub(super) fn resume(&mut self) {
        self.screen = Screen::Playing;
        self.set_grab(true);
        if self.session.local.is_some() {
            self.send(crate::net::Msg::Pause(false));
        }
    }

    pub(super) fn go_back(&mut self) {
        match self.screen {
            Screen::KeyBinds { in_game } => {
                self.menus.options.listening = None;
                self.settings.save();
                self.screen = Screen::Options { in_game };
            }
            Screen::Options { in_game } => {
                self.settings.save();
                self.screen = if in_game {
                    Screen::Paused
                } else {
                    Screen::MainMenu
                };
            }
            Screen::ResourcePacks { in_game } => {
                let enabled = self.menus.pack_screen.enabled();
                if enabled != self.settings.resource_packs {
                    self.settings.resource_packs = enabled;
                    self.settings.save();
                    self.reload_packs();
                }
                self.screen = Screen::Options { in_game };
            }
            Screen::Credits => self.screen = Screen::MainMenu,
            Screen::Skin => self.screen = Screen::MainMenu,
            _ => {}
        }
    }

    pub(super) fn apply(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Singleplayer => self.open_world_list(),
            Action::Multiplayer => self.open_multiplayer(),
            Action::SkinMenu => {
                self.menus.skin_error.clear();
                self.screen = Screen::Skin;
            }
            Action::SelectSkin(skin) => {
                self.settings.skin = skin;
                self.settings.save();
                self.screen = Screen::MainMenu;
            }
            Action::UploadSkin => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Minecraft skin PNG", &["png"])
                    .set_title("Skin PNG kiválasztása")
                    .set_parent(&*self.gfx.window)
                    .pick_file()
                {
                    let result = std::fs::read(&path)
                        .map_err(|_| "Nem sikerült beolvasni a fájlt.")
                        .and_then(|data| self.gfx.set_skin_png(0, data.clone()).map(|_| data));
                    match result {
                        Ok(data) => {
                            let _ = std::fs::create_dir_all("skins");
                            if std::fs::write("skins/custom.png", data).is_ok() {
                                self.gfx.skins.local_png = std::fs::read("skins/custom.png").ok();
                                self.settings.skin = 4;
                                self.settings.save();
                                self.screen = Screen::MainMenu;
                            } else {
                                self.menus.skin_error = t("skin.save_failed").into();
                            }
                        }
                        Err(msg) => self.menus.skin_error = msg.into(),
                    }
                }
            }
            Action::OpenLan => self.open_to_lan(),
            Action::Options => {
                self.screen = Screen::Options {
                    in_game: self.screen == Screen::Paused,
                }
            }
            Action::Credits => self.screen = Screen::Credits,
            Action::Quit => self.quit = true,
            Action::Resume => self.resume(),
            Action::ToTitle => self.quit_to_title(),
            Action::Back => self.go_back(),
            Action::ToggleFullscreen => self.toggle_fullscreen(),
            Action::AntialiasingChanged => self.gfx.gpu.set_msaa(self.settings.msaa),
            Action::Respawn => self.respawn(),
            Action::Language => {
                self.settings.hungarian = !self.settings.hungarian;
                crate::lang::set_hungarian(self.settings.hungarian);
            }
            Action::OpenLink(url) => open_url(url),
            Action::ResourcePacks => {
                if let Screen::Options { in_game } = self.screen {
                    self.menus.pack_screen.open(&self.settings.resource_packs);
                    self.screen = Screen::ResourcePacks { in_game };
                }
            }
            Action::KeyBinds => {
                if let Screen::Options { in_game } = self.screen {
                    self.screen = Screen::KeyBinds { in_game };
                }
            }
            Action::OpenPackFolder => {
                let _ = std::fs::create_dir_all(crate::pack::DIR);
                if let Ok(dir) = std::fs::canonicalize(crate::pack::DIR) {
                    // Without the \\?\ prefix, which Explorer does not take.
                    let dir = dir.to_string_lossy().trim_start_matches(r"\\?\").to_string();
                    open_url(&dir);
                }
            }
        }
    }
}
