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
use crate::model::players::hand::HandAnim;

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
pub(super) struct Me {
    /// Where they are and how they move (the server is told, `net::Pose`).
    pub(super) body: Player,
    pub(super) mode: GameMode,
    pub(super) cheats: bool,
    /// The first-person hands and what they hold, animating.
    pub(super) hand: HandAnim,
    pub(super) vitals: health::Vitals,
    pub(super) look: camera::Look,
    pub(super) aim: aim::Aim,
    pub(super) items: items::Items,
}

impl Me {
    /// Out of a world: survival, full health, nothing carried, looking east; `fov`: the
    /// field of view setting (the view eases from it).
    pub(super) fn new(fov: f32) -> Self {
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

    pub(super) fn creative(&self) -> bool {
        self.mode == GameMode::Creative
    }

    pub(super) fn spectator(&self) -> bool {
        self.mode == GameMode::Spectator
    }
}
