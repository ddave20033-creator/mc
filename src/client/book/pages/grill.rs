//! Chapter 4, grilling and water: meat on top of a furnace, what each doneness does to you,
//! and drinking water boiled safe.

use super::super::pictures::Pic;
use super::{El, Writer};
use crate::entity::block_entity::{BURN_TIME, FLIP_TIME, GRILL_TIME};
use crate::item::*;

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Grilling and Water", "Grillezés és ivóvíz"), ("Grill", "Grillezés"));
    w.text(
        "Meat does not go into the mouth: it is grilled on top of a burning furnace, one piece on each corner of the top.",
        "A hús nem a kemence szájába kerül: az égő kemence tetején grillezed, a tető minden sarkára egy darab fér.",
    );
    let labels = [w.tr("raw", "nyers"), w.tr("half", "félig"), w.tr("cooked", "sült"), w.tr("burnt", "szenes")];
    w.push(El::Picture(Pic::Grill(labels)));
    w.bullet(
        "Right-click a corner with raw meat to lay it there. Right-click it again to turn it over.",
        "Jobb klikk egy sarokra nyers hússal: odateszed. Újabb jobb klikk: megfordítod.",
    );
    let (grill, burn, flip) = (w.num(GRILL_TIME), w.num(BURN_TIME), w.num(FLIP_TIME));
    let text = if w.hu {
        format!("Csak a tűz felé néző oldal sül: {grill} mp után kész, {burn} mp után megég. A fordítás {flip} mp.")
    } else {
        format!("Only the side facing the fire cooks: it is done after {grill} s and burns after {burn} s. Turning takes {flip} s.")
    };
    w.push(El::Bullet(text));
    w.bullet("Left-click takes the piece off. Its name tells how its two sides came out.", "Bal klikk leveszi. A neve megmondja, milyen lett a két oldala.");
    w.bullet(
        "Watch the smoke: light steam while it cooks, more when the side is done, thick and dark when it burns.",
        "Figyeld a füstöt: sütés közben halvány gőz, ha kész az oldala, több füst, ha ég, sűrű sötét füst.",
    );
    w.head("What it does to you", "Mit tesz veled");
    for id in meat(PORKCHOP).unwrap_or_default() {
        let Some(c) = consumable(id) else { continue };
        let line = format!("{}: {} {}, {}", name(id), w.num(c.food), w.tr("food", "étel"), w.sickness(&c));
        w.push(El::Row(id, line));
    }
    w.text(
        "Mutton works the same way. A burnt side spoils the taste, so you get hungry again sooner.",
        "A birkahús ugyanígy működik. Egy szenes oldal elrontja az ízét, ezért hamarabb megéhezel.",
    );
    w.head("Drinking water", "Ivóvíz");
    w.text(
        "Right-click water with a glass bottle to fill it. Lake water quenches thirst but can make you sick. Put it in a furnace's mouth to boil it: boiled water is safe.",
        "Üvegpalackkal kattints jobb gombbal a vízre, és megtelik. A tóvíz oltja a szomjat, de megbetegíthet. Tedd a kemence szájába, hogy felforrjon: a forralt víz biztonságos.",
    );
    w.recipe(GLASS_BOTTLE);
    w.smelt(WATER_BOTTLE);
    for id in [WATER_BOTTLE, PURIFIED_WATER] {
        if let Some(c) = consumable(id) {
            let line = format!("{}: {} {}, {}", name(id), w.num(c.thirst), w.tr("thirst", "szomjúság"), w.sickness(&c));
            w.push(El::Row(id, line));
        }
    }
}
