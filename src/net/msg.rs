//! The messages host and players send each other, and the data in them.

use crate::entity::{GunBench, Grilled};
use crate::item::{Slot, Stack};
use crate::sim::felling::{FallingTree, LyingLog};
use crate::world::block::Block;
use crate::world::mesh::Notch;
use glam::{IVec3, Vec3};

/// "No block" in `Pose::open`.
pub const NO_BLOCK: IVec3 = IVec3::new(0, i32::MIN, 0);

/// A player's look, sent 20 times a second (animation values included, so the others
/// see exactly what the player sees of their own model).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Pose {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub body_yaw: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    /// Attack swing progress 0..1.
    pub attack: f32,
    pub crouch: f32,
    pub held: u16,
    pub skin: u8,
    pub flags: u8,
    /// The block being mined and how far (0 = not mining), for the crack overlay.
    pub mining: IVec3,
    pub mine_progress: f32,
    /// Block entity this player has open (chest lids open for everyone), `NO_BLOCK` if none.
    pub open: IVec3,
    /// What the player is busy with (`status`), shown in a bubble above their head.
    pub status: u8,
    /// The attachments on the held gun (`gun_mod` bits), and what it is doing
    /// (`model::pistol_view::GunAnim::pack`: its slide and magazine, for the others to see).
    pub gun_mods: u8,
    pub gun_state: u16,
    /// What they wear (`item::armor_code`).
    pub armor: u16,
    /// Holding the guide book open (`book` bits): open, and the page turns so far.
    pub book: u8,
    /// The spread it is open at (its left page / 2), and `book::HUNGARIAN`.
    pub book_page: u8,
    /// In spectator mode: flies through blocks, and only other spectators see them.
    pub spectator: bool,
    /// Running (0..1, eased in and out: their gun is carried across the chest).
    pub sprint: f32,
    /// How dirty the held gun is (`model::pistol_view::dirt_level`), for its look.
    pub gun_dirt: u8,
    /// At a gun station: the cleaning brush in the hand, where it is; looking into its
    /// drawer (it is out).
    pub brush: Option<Vec3>,
    pub drawer: bool,
    /// The held stack's `data` (a magazine's rounds, a gun's state: the rounds in its
    /// magazine, a revolver's cylinder).
    pub held_data: u16,
    /// More of what the held gun is doing (`model::pistol_view::GunAnim::pack_extra`): the
    /// magazine a reload brings, a revolver's round being loaded, its cases thrown out, the
    /// chambers its speedloader fills.
    pub gun_extra: u32,
    /// At a gun station: what is held on the mouse over its table or drawer, and where it
    /// shows (world).
    pub bench_hold: Option<(Stack, Vec3)>,
    /// Readying the held grenade: hundredths of a second since the button went down, plus
    /// one (0: not).
    pub grenade: u16,
    /// Holding a fishing rod: what it is doing, and where its bobber is.
    pub rod: Option<crate::model::angler::RodAnim>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobNet {
    pub id: u32,
    pub kind: u8,
    pub pos: Vec3,
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    pub hurt: bool,
    /// Seconds since dying, or negative while alive.
    pub death: f32,
    /// A sheep without its wool.
    pub sheared: bool,
    /// A target dummy: the damage it has taken, and the last hit.
    pub taken: f32,
    pub last_hit: f32,
    /// A wolf: `mob::wolf_flags`, and its collar's colour.
    pub flags: u8,
    pub collar: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemNet {
    pub id: u32,
    pub pos: Vec3,
    pub stack: Stack,
    pub age: f32,
}

/// Everything about a player that the host keeps between visits.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub needs: [f32; 7],
    /// Game mode (`mode`): survival, creative or spectator.
    pub mode: u8,
    pub flying: bool,
    pub slot: u8,
    pub inventory: Vec<Slot>,
    /// The head of the bed the player last used (where they come back to life).
    pub bed: Option<IVec3>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    // Player -> host
    Hello {
        proto: u16,
        name: String,
        /// The player's render distance in chunks: block changes farther away than that
        /// are not sent one by one (the chunk comes whole when they get near).
        view: u8,
    },
    Pose(Pose),
    /// A block the player placed or removed with an item (buckets): the host applies it
    /// with the world's rules.
    Place {
        p: IVec3,
        b: Block,
    },
    /// A block the player mined: the host drops what it drops.
    Break {
        p: IVec3,
        held: u16,
        creative: bool,
    },
    AttackMob {
        id: u32,
        dmg: f32,
        knock: f32,
    },
    AttackPlayer {
        id: u8,
        dmg: f32,
        knock: f32,
        kind: u8,
    },
    SpawnMob {
        kind: u8,
        pos: Vec3,
    },
    /// A shot from a gun, for the others to see: the bullets (their speed and direction)
    /// leaving `eye`, the muzzle flash and the case. From a player the host sends it on to
    /// the rest with `id` set to the shooter.
    Shot {
        id: u8,
        kind: u8,
        mods: u8,
        eye: Vec3,
        seed: f32,
        bullets: Vec<Vec3>,
    },
    /// A grenade thrown (`kind`: 0 frag, 1 smoke), the same way as `Shot`; `seed` names it,
    /// `fuse`: seconds until it goes off (less the longer it was held).
    Grenade {
        id: u8,
        kind: u8,
        pos: Vec3,
        vel: Vec3,
        seed: u32,
        fuse: f32,
    },
    /// Host -> players: that grenade went off here.
    Blast {
        pos: Vec3,
        seed: u32,
    },
    /// Used shears on a mob (the host drops its wool).
    Shear {
        id: u32,
    },
    /// Took down a target dummy (the host removes it and drops it as an item).
    BreakDummy {
        id: u32,
    },
    /// Right clicked a mob holding `item` (a wolf: given a bone, or told to sit or stand).
    UseOnMob {
        id: u32,
        item: u16,
    },
    DropItem {
        pos: Vec3,
        vel: Vec3,
        stack: Stack,
        delay: f32,
    },
    /// Opened the block entity at `p` (the host answers with its contents).
    Open {
        p: IVec3,
    },
    Command(String),
    Save(PlayerState),
    /// The world's owner paused the game (or goes on): alone in it, the world stands still.
    Pause(bool),
    /// An axe's stroke into the trunk at `p` left this cut (deep enough, the tree falls).
    /// From the server: the cut there now (none: gone).
    Notch {
        p: IVec3,
        notch: Option<Notch>,
    },
    /// Blocks set as they are, without the world's rules (the world's owner only: the
    /// testbed's scripts).
    Edit(Vec<(IVec3, Block)>),
    /// The stump at `p` struck with an axe came apart.
    Stump {
        p: IVec3,
    },
    /// The lying trunk `id` struck with an axe: a piece comes off the end `from_base` or not.
    CutLog {
        id: u32,
        from_base: bool,
    },
    /// Used a part of a furnace (`block_entity::part`): a left click (`take`) takes out
    /// what is there, a right click puts in or turns meat over. The player already took
    /// `offered` from their hand; the host gives back what did not go in, and what came out.
    FurnaceUse {
        p: IVec3,
        part: u8,
        take: bool,
        offered: Slot,
    },

    // Host -> player
    Welcome {
        id: u8,
        seed: u32,
        world: String,
        time: f32,
        spawn: (i32, i32),
        creative: bool,
        cheats: bool,
        state: Option<PlayerState>,
    },
    Refuse(String),
    Chunk {
        pos: (i32, i32),
        rle: Vec<u8>,
    },
    /// All chunks are sent: the player can start loading.
    Ready,
    Blocks(Vec<(IVec3, Block)>),
    Time(f32),
    Join {
        id: u8,
        name: String,
    },
    Leave {
        id: u8,
    },
    Poses(Vec<(u8, Pose)>),
    /// Mobs, dropped items and falling blocks near the player, as changes (`net::delta`):
    /// the mobs and items that are new or changed, the ids of those gone (or out of range),
    /// and all the falling blocks. `full`: everything near is listed, the rest goes.
    Entities {
        full: bool,
        mobs: Vec<MobNet>,
        items: Vec<ItemNet>,
        gone_mobs: Vec<u32>,
        gone_items: Vec<u32>,
        falling: Vec<(Vec3, Block)>,
    },
    Give(Stack),
    /// Someone broke a block here: debris flies (the block change comes separately).
    BreakFx {
        p: IVec3,
        block: Block,
    },
    /// Hit by another player (or their grenade).
    /// A tree felled: it falls over (every game shows it going; `TreeLands` says when it is
    /// down).
    TreeFalls(Box<FallingTree>),
    TreeLands {
        id: u32,
    },
    /// Player `by` picked up the dropped item `item`: it flies to them.
    Collect {
        item: u32,
        by: u8,
    },
    /// The trunks lying on the ground (all of them, when they change).
    Logs(Vec<LyingLog>),
    /// Something to see (and hear) at `pos` (`fx`).
    Fx {
        kind: u8,
        pos: Vec3,
    },
    Hurt {
        dmg: f32,
        from: Vec3,
        knock: f32,
        kind: u8,
    },

    // Both ways
    /// What lies on a gun station (at its left half) and the last change there.
    Bench {
        p: IVec3,
        bench: GunBench,
    },
    /// Contents of a chest or crafting table.
    Container {
        p: IVec3,
        kind: u8,
        slots: Vec<Slot>,
    },
    /// What everyone sees of a furnace: its fire, the meat on top (by corner) and what was
    /// put into the front.
    Furnace {
        p: IVec3,
        burn: f32,
        /// Seconds spent smelting the item in its mouth.
        cook: f32,
        input: Slot,
        fuel: Slot,
        /// What is smelted (it stays in the mouth).
        output: Slot,
        grill: Vec<(u8, Grilled)>,
    },
    Chat {
        text: String,
        color: [u8; 4],
    },
    /// One player's custom Minecraft skin PNG; id is assigned by the host.
    Skin {
        id: u8,
        png: Vec<u8>,
    },
}
