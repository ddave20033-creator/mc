//! An axe's cut in a trunk. The cuts are kept by the world (`World::notches`), apart from
//! the blocks: a cut trunk is still the same log block.

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
