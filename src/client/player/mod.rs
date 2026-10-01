//! The local player: their body and mode, and the parts of their state (`Me`): health and
//! damage (`health`), the look and the camera (`camera`), what they aim at and do with their
//! hands (`aim`), what they carry (`items`); and sleeping and watching other players.

pub(super) mod aim;
pub(super) mod camera;
pub(super) mod health;
pub(super) mod items;
pub(super) mod sleep;
pub(super) mod spectate;

use crate::entity::player::Player;
use crate::model::hand::HandAnim;

/// The player's game mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Survival,
    Creative,
    /// Flies through blocks, touches nothing, and can watch another player.
    Spectator,
}

/// This game's player in the world played: their body, mode and cheats, and the parts of
/// their state. Made anew for every world (`forget_world`), so nothing of the last one comes
/// along.
pub(in crate::client) struct Me {
    /// Where they are and how they move (the server is told, `net::Pose`).
    pub(in crate::client) body: Player,
    pub(in crate::client) mode: GameMode,
    pub(in crate::client) cheats: bool,
    /// The first-person hands and what they hold, animating.
    pub(in crate::client) hand: HandAnim,
    pub(in crate::client) vitals: health::Vitals,
    pub(in crate::client) look: camera::Look,
    pub(in crate::client) aim: aim::Aim,
    pub(in crate::client) items: items::Items,
}

impl Me {
    /// Out of a world: survival, full health, nothing carried, looking north; `fov`: the
    /// field of view setting (the view eases from it).
    pub(in crate::client) fn new(fov: f32) -> Self {
        Self {
            body: Player::default(),
            mode: GameMode::Survival,
            cheats: false,
            hand: HandAnim::new(),
            vitals: health::Vitals::new(),
            look: camera::Look::new(fov),
            aim: aim::Aim::default(),
            items: items::Items::new(),
        }
    }

    pub(in crate::client) fn creative(&self) -> bool {
        self.mode == GameMode::Creative
    }

    pub(in crate::client) fn spectator(&self) -> bool {
        self.mode == GameMode::Spectator
    }
}
