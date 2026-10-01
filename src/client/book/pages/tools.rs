//! Chapter 2, tools and ores: the pickaxes of each tier (how fast, how long they last, the
//! hardest they mine), and the tool shapes.

use super::super::pictures::Pic;
use super::{El, Writer};
use crate::item::*;

/// The hardest a pickaxe of this harvest level mines, by name ("Gold Ore and Diamond Ore").
fn hardest(w: &Writer, level: u8) -> String {
    let names: Vec<String> = hardest_mined(level).into_iter().map(|b| name(b as ItemId)).collect();
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} {} {last}", rest.join(", "), w.tr("and", "és")),
        None => String::new(),
    }
}

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Tools and Ores", "Eszközök és ércek"), ("Tools", "Eszközök"));
    w.text(
        "Tools come in six materials. The better ones dig faster, last longer and mine harder ores. A pickaxe that is too weak breaks an ore without dropping anything.",
        "Az eszközök hatféle anyagból készülhetnek. A jobbak gyorsabbak, tovább bírják, és keményebb érceket is kibányásznak. A túl gyenge csákány úgy töri szét az ércet, hogy semmi sem esik ki belőle.",
    );
    w.push(El::Picture(Pic::Tiers));
    for tier in TIER_ORDER {
        let id = tool_id(ToolKind::Pickaxe, tier);
        let (speed, uses, ores) = (w.num(tier.speed()), tier.durability(), hardest(w, tier.level()));
        let line = if w.hu {
            format!("{}: {speed}x gyors, {uses} használat. Legfeljebb: {ores}.", name(id))
        } else {
            format!("{}: speed {speed}, {uses} uses. Mines up to {ores}.", name(id))
        };
        w.push(El::Row(id, line));
    }
    w.head("Tool shapes", "Eszközformák");
    w.text(
        "The same shapes with any material: planks, cobblestone, copper, iron, gold or diamond. Here with copper, smelted from copper ore.",
        "Ugyanezek a formák bármilyen anyaggal: deszka, zúzottkő, réz, vas, arany vagy gyémánt. Itt rézzel, amit rézércből olvasztasz.",
    );
    for kind in TOOL_KINDS {
        w.recipe(tool_id(kind, Tier::Copper));
    }
}
