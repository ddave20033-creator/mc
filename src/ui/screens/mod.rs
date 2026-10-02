//! The menu screens drawn with `Ui`: the title screen, options, key binds, resource packs,
//! credits, pause, death and loading. Each returns the `Action` the player picked.

mod common;
mod menu;
mod options;
mod packs;

pub use common::{action_bar, backdrop, card_title, death, loading, pause, screen_header, PauseLan};
pub use menu::{credits, main_menu, minigames, skin_menu, PreviewRotation};
pub use options::{key_binds, options, OptionsState};
pub use packs::{resource_packs, PackScreen};

pub enum Action {
    None,
    Singleplayer,
    Options,
    Credits,
    Quit,
    Resume,
    ToTitle,
    Back,
    ToggleFullscreen,
    /// Options: the anti-aliasing setting changed.
    AntialiasingChanged,
    Respawn,
    Language,
    Multiplayer,
    SkinMenu,
    Minigames,
    /// Pause menu: open this world to the LAN.
    OpenLan,
    /// Open a web page in the browser (clicked link).
    OpenLink(&'static str),
    /// Options: the resource pack screen.
    ResourcePacks,
    /// Options: the key binds screen.
    KeyBinds,
    /// Resource pack screen: show `resourcepacks/` in the file explorer.
    OpenPackFolder,
}
