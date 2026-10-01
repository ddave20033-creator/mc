//! Chapter 7, ammunition and attachments: the rounds, the speedloader, each attachment and
//! the target dummy.

use super::super::pictures::Pic;
use super::{El, Writer};
use crate::item::*;

/// What an attachment does.
fn about(item: ItemId, w: &Writer) -> String {
    let hu = w.hu;
    match (item, hu) {
        (SCOPE, false) => "Zooms in far when you aim.".into(),
        (SCOPE, true) => "Célzáskor nagyon ránagyít.".into(),
        (SILENCER, false) => "No muzzle flash gives you away.".into(),
        (SILENCER, true) => "Nem árul el a torkolattűz.".into(),
        (EXTENDED_MAGAZINE, _) => {
            let list: Vec<String> =
                GUN_KINDS.iter().filter_map(|k| Some(format!("{} {}", name(k.item()), k.magazine()?.extended?.1))).collect();
            format!("{}: {}.", w.tr("More rounds before you reload", "Több töltény újratöltés előtt"), list.join(", "))
        }
        (FLASHLIGHT, false) => "A bright light on the rail (instead of the laser sight): it lights up where the gun points. Switched on and off with its key (L). Only found in creative.".into(),
        (FLASHLIGHT, true) => "Erős lámpa a sínen (a lézer helyett): megvilágítja, amerre a fegyver néz. A gombjával (L) kapcsolod ki-be. Csak kreatívban van.".into(),
        (_, false) => "Much steadier from the hip, and a red dot shows where you point.".into(),
        (_, true) => "Csípőből sokkal pontosabb, és egy piros pont mutatja, hová célzol.".into(),
    }
}

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Ammo and Attachments", "Lőszer és kiegészítők"), ("Ammo", "Lőszer"));
    w.text(
        "The pistol fires bullets made from iron and coal (or charcoal); the revolver longer magnum rounds, with a copper jacket; the AK big 7.62 rifle rounds, with a steel core. One goes per shot, and each fits only its own gun. Magazines are loaded at the gun station: drag rounds onto one lying on the table.",
        "A pisztoly vasból és szénből (vagy faszénből) készült töltényt lő, a revolver hosszabb, rézköpenyes magnum töltényt, az AK nagy, acélmagvas 7,62-es puskatöltényt. Lövésenként egy fogy, és mindegyik csak a saját fegyverébe jó. A tárakat a fegyverasztalon töltöd meg: húzz töltényeket az asztalon fekvő tárra.",
    );
    let mut ammo: Vec<ItemId> = GUN_KINDS.iter().map(|k| k.ammo()).collect();
    ammo.dedup();
    for item in ammo {
        w.recipe(item);
    }
    w.push(El::Head(name(SPEEDLOADER)));
    w.text(
        "Holds six magnum rounds for the revolver, to load it all at once. Load it at the gun station: drag rounds onto it lying on the table.",
        "Hat magnum töltényt tart a revolverhez, hogy egyszerre töltsd meg. A fegyverasztalon töltöd meg: húzz rá töltényeket, amíg az asztalon fekszik.",
    );
    w.recipe(SPEEDLOADER);
    w.head("Attachments", "Kiegészítők");
    w.text(
        "Fitted on the pistol at the gun station: drag one onto the gun lying on its table.",
        "A fegyverasztalon szerelheted fel őket a pisztolyra: húzd rá az asztalon fekvő fegyverre.",
    );
    w.push(El::Picture(Pic::Items(ATTACHMENTS.iter().map(|a| a.1).collect())));
    for (_, item) in ATTACHMENTS {
        w.push(El::Head(name(item)));
        let text = about(item, w);
        w.push(El::Text(text));
        // (the weapon light is not made: only found in creative)
        if recipe_view(item).is_some() {
            w.recipe(item);
        }
    }
    w.push(El::Head(name(TARGET_DUMMY)));
    w.text(
        "Set it up with a right click, then shoot or hit it: above its head it shows all the damage it has taken and the last hit. After a few quiet seconds it starts counting again. Sneak and hit it to take it down.",
        "Jobb klikkel állítod fel, aztán lőj vagy üss bele: a feje fölött mutatja, mennyi sebzést kapott összesen, és mennyit az utolsó találat. Néhány nyugodt másodperc után újrakezdi a számolást. Guggolva ütve leszeded.",
    );
    w.recipe(TARGET_DUMMY);
}
