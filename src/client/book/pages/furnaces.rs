//! Chapter 3, furnaces: using them, the three kinds, what smelts where and the fuels.

use super::super::pictures::Pic;
use super::{El, Writer, smelt_time};
use crate::item::*;
use crate::world::{COAL_BLOCK, FURNACES, OAK_LOG, furnace_tier};

/// What each furnace is like, by tier (`secs`: how long it takes to smelt one thing).
fn about(tier: u8, hu: bool, secs: &str) -> String {
    match (tier, hu) {
        (1, false) => format!("One block. Smelts food, clay, cobblestone, logs and copper ore, {secs} s each."),
        (1, true) => format!("Egy blokk. Ételt, agyagot, zúzottkövet, rönköt és rézércet olvaszt, darabját {secs} mp alatt."),
        (2, false) => format!("A furnace with a brick chimney on top, so it needs a free block above it. It smelts iron ore and sand too, {secs} s each. Smoke rises from the chimney while it burns."),
        (2, true) => format!("Kemence téglakéménnyel a tetején, ezért fölötte egy szabad blokk kell. Vasércet és homokot is olvaszt, darabját {secs} mp alatt. Égés közben füstöl a kéménye."),
        (_, false) => format!("Two blocks wide and two tall: the furnace, a panel to its right and a hood over both, so it needs that much room. It smelts everything, gold and diamond ore too, {secs} s each. Its hood glows while it burns."),
        (_, true) => format!("Két blokk széles és két blokk magas: a kemence, jobbra mellette egy panel, fölöttük elszívó, ezért ennyi hely kell neki. Mindent kiolvaszt, az arany- és gyémántércet is, darabját {secs} mp alatt. Égés közben izzik az elszívója."),
    }
}

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Furnaces", "Kemencék és kohók"), ("Furnaces", "Kemencék"));
    w.text(
        "Furnaces have no window: you use them by clicking the block itself. There are three kinds, and the better ones smelt more things, faster.",
        "A kemencéknek nincs külön ablakuk: magán a blokkon használod őket. Háromféle van; a jobbak többféle dolgot és gyorsabban olvasztanak.",
    );
    let labels = [w.tr("to smelt", "olvasztani"), w.tr("fuel", "üzemanyag")];
    w.push(El::Picture(Pic::Front(labels)));
    w.bullet(
        "Right-click the upper half of the front: what you hold goes into its mouth to be smelted.",
        "Jobb klikk az előlap felső felére: ami a kezedben van, a szájába kerül olvadni.",
    );
    w.bullet("Right-click the lower half: the fuel you hold goes into the firebox.", "Jobb klikk az alsó felére: a kezedben lévő üzemanyag a tűztérbe kerül.");
    w.bullet(
        "Left-click takes out: above, what is smelted first, then what still waits; below, the fuel. The furnace is not mined while you do it.",
        "Bal klikk kivesz: fent előbb a kész terméket, aztán ami még vár; lent az üzemanyagot. Közben nem bányászod ki a kemencét.",
    );
    w.bullet(
        "What is smelted stays inside until you take it. Fuel only burns while there is something to heat.",
        "A kész termék bent marad, amíg ki nem veszed. Üzemanyagot csak akkor éget, ha van mit melegítenie.",
    );
    w.bullet(
        "A furnace only takes what it can smelt: a plain furnace will not take iron ore.",
        "A kemence csak azt veszi be, amit ki is tud olvasztani: egy sima kemence nem fogad be vasércet.",
    );
    w.push(El::Break);
    for base in FURNACES {
        let tier = furnace_tier(base);
        w.push(El::Head(name(base as ItemId)));
        w.push(El::Picture(Pic::Furnace(base)));
        let text = about(tier, w.hu, &w.num(smelt_time(tier)));
        w.push(El::Text(text));
        if tier > 1 {
            w.bullet(
                "Breaking any of its blocks takes down the whole furnace; what was in it drops.",
                "Ha bármelyik blokkját kiütöd, az egész kemence lebomlik, és kiesik belőle, ami benne volt.",
            );
        }
        w.recipe(base as ItemId);
    }
    w.head("What smelts where", "Mi hol olvad");
    w.text(
        "The first furnace that can smelt a thing, and how long it takes there:",
        "Az első kemence, amelyik kiolvasztja, és mennyi ideig tart benne:",
    );
    let mut smeltable: Vec<ItemId> = all_items().into_iter().filter(|&id| smelt(id).is_some()).collect();
    smeltable.sort_by_key(|&id| smelt_tier(id));
    for id in smeltable {
        w.smelt(id);
    }
    w.head("Fuel", "Üzemanyag");
    w.text(
        "How long one piece burns, and how many items it smelts in the furnace, blast furnace and advanced furnace. The better furnaces get more out of the same fuel.",
        "Meddig ég egy darab, és hány tárgyat olvaszt ki a kemencében, a kohóban és a fejlett kohóban. A jobb kemence ugyanabból az üzemanyagból többet hoz ki.",
    );
    // Each kind of fuel by one of its items (they burn as long as it does).
    let fuels: [(ItemId, String); 6] = [
        (COAL, format!("{}, {}", name(COAL), name(CHARCOAL))),
        (COAL_BLOCK as ItemId, name(COAL_BLOCK as ItemId)),
        (LAVA_BUCKET, w.tr("Lava Bucket (the empty bucket stays in the firebox)", "Lávás vödör (az üres vödör a tűztérben marad)")),
        (OAK_LOG as ItemId, w.tr("Logs, planks, chests, tables, stairs", "Rönk, deszka, láda, asztal, lépcső")),
        (tool_id(ToolKind::Pickaxe, Tier::Wood), w.tr("Wooden tools, doors", "Fa eszközök, ajtó")),
        (STICK, w.tr("Sticks, saplings, wool", "Bot, csemete, gyapjú")),
    ];
    for (id, what) in fuels {
        let t = fuel_time(id).unwrap_or(0.0);
        let items: Vec<String> = FURNACES.iter().map(|&b| w.num((t / smelt_time(furnace_tier(b))).floor())).collect();
        let line = format!("{what}: {} {}, {} {}", w.num(t), w.tr("s", "mp"), items.join(" / "), w.tr("items", "tárgy"));
        w.push(El::Row(id, line));
    }
}
