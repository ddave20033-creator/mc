//! Chapter 1, getting started: how to read the book, the first tools and crafting.

use super::{El, Writer};
use crate::item::*;
use crate::world::{CHEST, CRAFTING_TABLE, FURNACE, TORCH};

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Getting Started", "Első lépések"), ("Start", "Kezdés"));
    w.text(
        "Welcome! This book tells how things work in this world: crafting, the furnaces and the guns. Hold it and look down to bring it up to read. Right click turns the page, left click turns back (the arrow keys too). The numbers on the tabs along the top open the chapters: press 1 to 8 while reading.",
        "Üdv! Ez a könyv elmondja, hogyan működnek a dolgok ebben a világban: a barkácsolás, a kemencék és a fegyverek. Vedd a kezedbe, és nézz le: magad elé emeled és olvashatod. Jobb klikkel lapozol előre, bal klikkel vissza (a nyilakkal is). A fenti füleken lévő számok a fejezetek: olvasás közben az 1-8 gombbal odaugrasz.",
    );
    w.head("Your first tools", "Az első eszközök");
    w.text(
        "Chop a tree to get logs. Planks are not crafted: saw the logs into planks at a sawbench (you make one by hand from logs). Planks make sticks and a crafting table, and at the table you can make a wooden pickaxe: with it, go for stone.",
        "Vágj ki egy fát, hogy rönköt kapj. A deszkát nem barkácsolod: a rönköt fűrészasztalon fűrészeled deszkára (azt kézzel készíted rönkből). A deszkából bot és barkácsasztal lesz, az asztalnál pedig fa csákányt készíthetsz, azzal jöhet a kő.",
    );
    for id in [STICK, CRAFTING_TABLE as ItemId, tool_id(ToolKind::Pickaxe, Tier::Wood)] {
        w.recipe(id);
    }
    w.head("Crafting", "Barkácsolás");
    let inv = &w.keys.0;
    let text = if w.hu {
        format!("Az {inv} gombbal nyílik a tárgylistád; a Barkácsolás fülén az van, amit kézzel elkészíthetsz: csak néhány alapdolog. Minden máshoz barkácsasztal kell: kattints rá jobb gombbal, és a tárgylistád a Barkácsasztal fülön nyílik meg, rajta minden recepttel.")
    } else {
        format!("Press {inv} to open your inventory: its Crafting tab lists what you can make by hand, only a few basic things. Everything else needs a crafting table: right-click one and your inventory opens on its Crafting Table tab, with every recipe.")
    };
    w.push(El::Text(text));
    w.bullet(
        "Pick what to make from the list (what you have enough for comes first): the ingredients show on the right. Craft makes one; shift-click makes as many as you have enough for.",
        "Válaszd ki a listából, mit készítenél (elöl az, amihez elég minden): jobb oldalt látod a hozzávalókat. Az Elkészít gomb egyet készít, shift-kattintással annyit, amennyire elég.",
    );
    w.bullet(
        "The recipes in this book show the ingredients laid out; where a cell keeps changing, any of those items will do (coal or charcoal, say).",
        "A könyv receptjei kirakva mutatják a hozzávalókat; ahol egy mező váltakozik, ott bármelyik tárgy jó (például szén vagy faszén).",
    );
    for id in [TORCH as ItemId, CHEST as ItemId, FURNACE as ItemId, GUIDE_BOOK] {
        w.recipe(id);
    }
}
