//! The axe's cuts in the trunks (few at a time), kept apart from the blocks: a cut
//! trunk is still the same log block.

use glam::IVec3;

/// An axe's cut in an upright trunk: the way its face looks (radians round the trunk, 0 =
/// +X, toward +Z), the height of its middle in the block (0..1) and how deep it goes (0..1
/// of the trunk's width); after the tree fell, the stump left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    pub angle: f32,
    pub height: f32,
    pub depth: f32,
    /// The tree above has been felled: only the stump is left, up to the cut's middle.
    pub felled: bool,
}

/// The cuts in the trunks (few at a time), read by the mesher.
static NOTCHES: std::sync::RwLock<Vec<(IVec3, Notch)>> = std::sync::RwLock::new(Vec::new());

pub fn notch_at(p: IVec3) -> Option<Notch> {
    let list = NOTCHES.read().ok()?;
    list.iter().find(|(q, _)| *q == p).map(|(_, n)| *n)
}

/// Every cut there is (for saving).
pub fn all_notches() -> Vec<(IVec3, Notch)> {
    NOTCHES.read().map(|l| l.clone()).unwrap_or_default()
}

/// No cuts any more (another world is loaded).
pub fn clear_notches() {
    if let Ok(mut list) = NOTCHES.write() {
        list.clear();
    }
}

/// Puts (or with None, takes away) the cut at `p`; the chunk has to be meshed again.
pub fn set_notch(p: IVec3, notch: Option<Notch>) {
    let Ok(mut list) = NOTCHES.write() else { return };
    list.retain(|(q, _)| *q != p);
    if let Some(n) = notch {
        list.push((p, n));
    }
}
