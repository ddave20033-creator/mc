//! What the player uses in their hands: guns (the revolver's cylinder on its own), grenades and
//! the fishing rod.

pub(super) mod guns;
pub(super) mod revolver;
pub(super) mod grenades;
pub(super) mod fishing;

/// The state of each tool: shots, bullets and bullet holes; grenades readied and flying;
/// the fishing line. Made anew for every world: nothing of the last one comes along (a grenade
/// thrown just before leaving would blow up in the next).
#[derive(Default)]
pub(in crate::client) struct Tools {
    /// Shooting and the gun station.
    pub(in crate::client) guns: guns::Guns,
    pub(in crate::client) grenades: grenades::Grenades,
    /// The fishing rod's line, bobber and the fish on it.
    pub(in crate::client) fishing: fishing::Fishing,
}
