//! Soccer.

mod card;

use super::Minigame;
use crate::ui::rgba;

pub const INFO: Minigame = Minigame {
    name: "minigames.soccer",
    accent: rgba(120, 230, 120, 255),
    card: card::draw,
};
