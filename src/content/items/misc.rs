//! Everything else: buckets and the glass bottle, the guide book, grenades, and what puts a
//! mob into the world (spawn eggs, the target dummy).

use super::*;
use crate::world::{LAVA, WATER};

/// Puts its mob into the world (the mob's `egg` is this item): a row of the mobs' tab.
const fn egg(group: u8, icon: u32) -> ItemDef {
    ItemDef { icon: Icon::Flat(icon), on_use: OnUse::Spawn, creative: Creative::Mobs(group), ..ITEM }
}

/// A grenade: thrown with the right mouse button held.
const fn grenade(kind: GrenadeKind, icon: u32) -> ItemDef {
    ItemDef {
        icon: Icon::Flat(icon),
        stack: 16,
        grenade: Some(kind),
        on_use: OnUse::Throw,
        creative: Creative::Tools(7),
        ..ITEM
    }
}

items! {
    after food::END;

    BUCKET = ItemDef {
        key: "bucket", en: "Bucket", hu: "Vödör", icon: Icon::Flat(tex::BUCKET), stack: 16,
        on_use: OnUse::Scoop, creative: Creative::Tools(4), ..ITEM
    };
    WATER_BUCKET = ItemDef {
        key: "water_bucket", en: "Water Bucket", hu: "Vizesvödör", icon: Icon::Flat(tex::WATER_BUCKET),
        on_use: OnUse::Pour(WATER), creative: Creative::Tools(4), ..SINGLE
    };
    LAVA_BUCKET = ItemDef {
        key: "lava_bucket", en: "Lava Bucket", hu: "Lávás vödör", icon: Icon::Flat(tex::LAVA_BUCKET),
        fuel: Some(1000.0), on_use: OnUse::Pour(LAVA), creative: Creative::Tools(4), ..SINGLE
    };
    /// Filled at a lake (`WATER_BOTTLE`), and given back empty when drunk.
    GLASS_BOTTLE = ItemDef {
        key: "glass_bottle", en: "Glass Bottle", hu: "Üvegpalack", icon: Icon::Flat(tex::GLASS_BOTTLE),
        on_use: OnUse::FillBottle, creative: Creative::Tools(4), ..ITEM
    };
    /// The guide book: held open in both hands, its pages turned with the mouse buttons; it
    /// explains crafting, the furnaces and the guns (see `client::book`).
    GUIDE_BOOK = ItemDef {
        key: "guide_book", en: "Guide Book", hu: "Kézikönyv", icon: Icon::Flat(tex::BOOK),
        creative: Creative::Tools(4), ..SINGLE
    };

    FRAG_GRENADE = ItemDef {
        key: "frag_grenade", en: "Frag Grenade", hu: "Repeszgránát", ..grenade(GrenadeKind::Frag, tex::FRAG_GRENADE)
    };
    SMOKE_GRENADE = ItemDef {
        key: "smoke_grenade", en: "Smoke Grenade", hu: "Füstgránát", ..grenade(GrenadeKind::Smoke, tex::SMOKE_GRENADE)
    };

    PIG_SPAWN_EGG = ItemDef {
        key: "pig_spawn_egg", en: "Pig Spawn Egg", hu: "Disznó idéző tojás", ..egg(0, tex::PIG_SPAWN_EGG)
    };
    SHEEP_SPAWN_EGG = ItemDef {
        key: "sheep_spawn_egg", en: "Sheep Spawn Egg", hu: "Birka idéző tojás", ..egg(0, tex::SHEEP_SPAWN_EGG)
    };
    WOLF_SPAWN_EGG = ItemDef {
        key: "wolf_spawn_egg", en: "Wolf Spawn Egg", hu: "Farkas idéző tojás", ..egg(0, tex::WOLF_SPAWN_EGG)
    };
    /// A wooden target dummy: set up with a right click, it shows the damage it takes above
    /// its head (see `content::mobs::target_dummy`).
    TARGET_DUMMY = ItemDef {
        key: "target_dummy", en: "Target Dummy", hu: "Gyakorlóbábu", stack: 16, ..egg(1, tex::TARGET_DUMMY)
    };
}
