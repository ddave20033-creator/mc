//! Chapter 8, more recipes: everything else there is to craft (all the recipes the earlier
//! chapters did not show, but the tools of the other materials).

use super::{El, Writer};
use crate::item::*;

pub(super) fn write(w: &mut Writer) {
    w.chapter(("More Recipes", "További receptek"), ("Recipes", "Receptek"));
    w.text(
        "Everything else there is to craft. Storage blocks turn back into nine of what they are made of.",
        "Minden más, amit barkácsolni lehet. A tárolóblokkokból visszakapod a kilenc darabot, amiből készültek.",
    );
    for id in recipe_results() {
        if !w.shown.contains(&id) && tool_of(id).is_none() {
            w.push(El::Recipe(id));
        }
    }
}
