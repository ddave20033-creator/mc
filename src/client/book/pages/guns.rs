//! Chapter 6, guns: shooting, and a page for each gun (what it is like, its numbers next to
//! the other guns', its rounds and its parts).

use super::super::pictures::Pic;
use super::{El, Writer};
use crate::item::*;

/// What a gun is like.
fn about(kind: GunKind, hu: bool) -> &'static str {
    match (kind, hu) {
        (GunKind::Pistol, false) => "Light and quick, a good first gun. Its five parts are crafted from iron.",
        (GunKind::Pistol, true) => "Könnyű és gyors, jó első fegyver. Az öt alkatrészét vasból barkácsolod.",
        (GunKind::Revolver, false) => "Six heavy rounds in a swing-out cylinder: slower, kicks harder, hits much harder. Each pull turns the next chamber under the hammer: a fired case stays in its chamber, and an empty one only clicks. The reload key swings the cylinder out, throws out the cases (the live rounds fall to the ground too) and loads it a round at a time from the bullets you carry (shoot to stop), or all at once from a loaded speedloader.",
        (GunKind::Revolver, true) => "Hat nehéz töltény a kibillenő forgótárban: lassabb, jobban rúg, de sokkal nagyobbat üt. Minden ravaszhúzás a következő kamrát fordítja a kakas alá: a kilőtt hüvely a kamrában marad, az üres kamrán csak kattan. Az újratöltés gomb kibillenti a forgótárat, kiveti a hüvelyeket (az éles töltények is a földre esnek), és egyesével tölti a nálad lévő töltényekből (lövéssel megállíthatod), vagy egyszerre egy megtöltött gyorstöltőből.",
        (GunKind::Ak, false) => "A big-calibre automatic rifle: it fires as long as you hold the button, ten rounds a second, from a curved 30-round magazine. Its 7.62 rounds hit hard and fly far and flat, but every shot kicks the barrel up: aim, and fire short bursts. It is changed like the pistol's magazine, with the reload key, and the bolt carrier's handle is pulled when the chamber is empty. Its magazines are loaded at the gun station.",
        (GunKind::Ak, true) => "Nagy kaliberű gépkarabély: addig lő, amíg nyomva tartod a gombot, másodpercenként tízet, egy 30 töltényes ívelt tárból. A 7,62-es töltény nagyot üt, messzire és laposan repül, de minden lövés feljebb rúgja a csövet: célozz, és lőj rövid sorozatokat. A tárat a pisztolyéhoz hasonlóan az újratöltés gombbal cseréled, üres csőnél a zárkeret fogantyúját is meghúzza. A tárait a fegyverasztalon töltöd meg.",
    }
}

/// How a gun is put together from its parts.
fn parts(kind: GunKind) -> (&'static str, &'static str) {
    match kind {
        GunKind::Pistol => (
            "Craft the frame, barrel, recoil spring and slide, lay them on the gun station's table and right-click one of them. The gun comes out without a magazine: craft that too, and put it in with the reload key.",
            "Barkácsold meg a vázat, a csövet, a helyretoló rugót és a szánt, tedd őket a fegyverasztalra, és kattints jobb gombbal valamelyikre. A fegyver tár nélkül készül el: azt is barkácsold meg, és az újratöltés gombbal tedd bele.",
        ),
        GunKind::Ak => (
            "Craft the receiver (the barrel, sights, grip and stock on it), the gas tube, the bolt carrier and the dust cover, lay them on the gun station's table and right-click one of them. It comes out without a magazine: craft one, load it with rounds at the table, and put it in with the reload key.",
            "Barkácsold meg a tokot (rajta a cső, az irányzék, a markolat és a tus), a gázcsövet, a zárkeretet és a tokfedelet, tedd őket a fegyverasztalra, és kattints jobb gombbal valamelyikre. Tár nélkül készül el: barkácsolj egyet, töltsd meg az asztalon, és az újratöltés gombbal tedd bele.",
        ),
        GunKind::Revolver => (
            "Craft the frame, barrel, mainspring, cylinder and hammer, lay them on the gun station's table and right-click one of them. It comes out empty: load it with the reload key. A right click on it there takes it apart again.",
            "Barkácsold meg a vázat, a csövet, a kakasrugót, a forgótárat és a kakast, tedd őket a fegyverasztalra, és kattints jobb gombbal valamelyikre. Üresen készül el: az újratöltés gombbal töltöd meg. Ott jobb kattintással újra szétszedheted.",
        ),
    }
}

pub(super) fn write(w: &mut Writer) {
    w.chapter(("Guns", "Fegyverek"), ("Guns", "Fegyverek"));
    let reload = &w.keys.1;
    let text = if w.hu {
        format!("Bal egérgomb: lövés. Jobb egérgomb: célzás, pontosabb és ránagyít. {reload}: tárcsere: a régi tár kiesik, a legtelibb töltött tárad bekerül. Ha nincs másik, csak kiesik a régi. A tárat a fegyverasztalon töltöd meg: húzz töltényt az asztalon fekvő tárra.")
    } else {
        format!("Left mouse button: shoot. Right mouse button: aim down the sights, steadier and zoomed in. {reload}: change magazines: the old one drops out and your fullest loaded one goes in. Without another, the old one only drops out. Magazines are loaded at the gun station: drag rounds onto a magazine lying on its table.")
    };
    w.push(El::Text(text));
    w.bullet(
        "Bullets really fly: they take time to get there and drop with distance. Aim a little higher at far targets.",
        "A lövedék valóban repül: idő kell, amíg odaér, és a távolsággal esik. Távoli célnál célozz kicsit feljebb.",
    );
    w.bullet("From the hip the shots spread; aimed they go where the sights point.", "Csípőből szórnak a lövések, célozva oda mennek, ahová az irányzék mutat.");
    // Each number as a bar, as big as it is next to the biggest of the guns'.
    let all = GUN_KINDS.map(|k| k.stats());
    let max = |f: &dyn Fn(&Stats) -> f32| all.iter().map(|s| f(s)).fold(0.0f32, f32::max);
    let per_shot = |st: &Stats| st.damage * st.pellets as f32;
    let rate = |st: &Stats| 1.0 / st.fire_delay;
    for kind in GUN_KINDS {
        let st = kind.stats();
        w.push(El::Break);
        w.push(El::Head(name(kind.item())));
        w.push(El::Picture(Pic::Gun(kind)));
        w.push(El::Text(about(kind, w.hu).into()));
        let damage = if st.pellets > 1 { format!("{}\u{d7}{}", w.num(st.damage), st.pellets) } else { w.num(st.damage) };
        let magazine = match kind.magazine().and_then(|m| m.extended) {
            Some((_, extended)) => format!("{} ({})", st.magazine, extended),
            None => st.magazine.to_string(),
        };
        let stats = [
            (w.tr("Damage", "Sebzés"), per_shot(st) / max(&per_shot), damage),
            (w.tr("Fire rate", "Tűzgyorsaság"), rate(st) / max(&rate), format!("{}/{}", w.num((rate(st) * 10.0).round() / 10.0), w.tr("s", "mp"))),
            (w.tr("Magazine", "Tár"), st.magazine as f32 / max(&|s: &Stats| s.magazine as f32), magazine),
            (w.tr("Range", "Lőtáv"), st.range / max(&|s: &Stats| s.range), w.num(st.range)),
            (w.tr("Accuracy", "Pontosság"), 1.0 - st.spread_hip / max(&|s: &Stats| s.spread_hip) * 0.85, format!("{}\u{b0}", w.num(st.spread_hip))),
            (w.tr("Reload", "Töltés"), st.reload / max(&|s: &Stats| s.reload), format!("{} {}", w.num(st.reload), w.tr("s", "mp"))),
            (w.tr("Clean for", "Tiszta"), st.dirt_max as f32 / max(&|s: &Stats| s.dirt_max as f32), format!("{} {}", st.dirt_max, w.tr("shots", "lövés"))),
        ];
        for (label, k, value) in stats {
            w.push(El::Stat(label, k, value));
        }
        w.text(
            "Magazine in brackets: with the extended magazine. Accuracy: the spread from the hip.",
            "A tárnál zárójelben: bővített tárral. Pontosság: a szórás csípőből.",
        );
        let fires = format!("{}: {}", w.tr("Fires", "Lőszere"), name(kind.ammo()));
        w.push(El::Row(kind.ammo(), fires));
        w.head("Parts", "Alkatrészek");
        let (en, hun) = parts(kind);
        w.text(en, hun);
        for &item in kind.parts() {
            w.recipe(item);
        }
    }
}
