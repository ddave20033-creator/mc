//! Chapter 1, getting started: how to read the book, the first tools and crafting.

use super::{El, Writer};
use crate::item::*;
use crate::world::{CHEST, CRAFTING_TABLE, FURNACE, PLANKS, TORCH};

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Getting Started", "Első lépések"), ("Start", "Kezdés"));
    w.text(
        "Welcome! This book tells how things work in this world: crafting, the furnaces and the guns. Hold it and look down to bring it up to read. Right click turns the page, left click turns back (the arrow keys too). The numbers on the tabs along the top open the chapters: press 1 to 8 while reading.",
        "Üdv! Ez a könyv elmondja, hogyan működnek a dolgok ebben a világban: a barkácsolás, a kemencék és a fegyverek. Vedd a kezedbe, és nézz le: magad elé emeled és olvashatod. Jobb klikkel lapozol előre, bal klikkel vissza (a nyilakkal is). A fenti füleken lévő számok a fejezetek: olvasás közben az 1-8 gombbal odaugrasz.",
    );
    w.head("Your first tools", "Az első eszközök");
    w.text(
        "Punch a tree to get logs. Logs make planks, planks make sticks and a crafting table. At the table you can make a wooden pickaxe: with it, go for stone.",
        "Üss meg egy fát, hogy rönköt kapj. A rönkből deszka lesz, a deszkából bot és barkácsasztal. Az asztalon fa csákányt készíthetsz, azzal pedig jöhet a kő.",
    );
    for id in [PLANKS as ItemId, STICK, CRAFTING_TABLE as ItemId, tool_id(ToolKind::Pickaxe, Tier::Wood)] {
        w.recipe(id);
    }
    w.head("Crafting", "Barkácsolás");
    let inv = &w.keys.0;
    let text = if w.hu {
        format!("Az {inv} gombbal nyílik a tárgylistád, benne egy 2x2-es barkácsráccsal. A nagyobb receptekhez a barkácsasztal 3x3-as rácsa kell: kattints rá jobb gombbal, és a nézet az asztal fölé úszik. Rakd a tárgyakat az asztal rácsára.")
    } else {
        format!("Press {inv} to open your inventory: it has a 2x2 crafting grid. Bigger recipes need the 3x3 grid of a crafting table: right-click it and the view glides over the table. Lay the items onto its grid.")
    };
    w.push(El::Text(text));
    w.bullet(
        "At the table, click anywhere off the slots with an empty hand: it crafts a whole stack into the middle of the table.",
        "Az asztalnál üres kézzel kattints bárhová a mezőkön kívül: egy egész köteget elkészít az asztal közepére.",
    );
    w.bullet("A recipe can sit anywhere on the grid, and works mirrored too.", "A recept bárhol lehet a rácson, és tükrözve is működik.");
    w.bullet(
        "Where a cell in this book keeps changing, any of those items will do (any log, coal or charcoal).",
        "Ahol ebben a könyvben egy mező váltakozik, ott bármelyik tárgy jó (bármilyen rönk, szén vagy faszén).",
    );
    for id in [TORCH as ItemId, CHEST as ItemId, FURNACE as ItemId, GUIDE_BOOK] {
        w.recipe(id);
    }
}
