//! Chapter 5, the gun station: the two stations, and laying things on the table, taking guns
//! apart and putting them together, cleaning, attachments and loading magazines there.

use super::super::pictures::Pic;
use super::{El, Writer};
use crate::item::*;
use crate::world::{GUN_STATION, RIFLE_BENCH};

pub(super) fn write(w: &mut Writer) {
    w.chapter(("The Gun Station", "A fegyverasztal"), ("Station", "Asztal"));
    w.push(El::Picture(Pic::Station));
    w.text(
        "The gun station is two blocks wide, its drawer at the front (keep the space in front of it free). Right-click it and the view glides over its table; move the mouse to look along it. Everything happens right there on the table.",
        "A fegyverasztal két blokk széles, elöl van a fiókja (előtte hagyd szabadon a helyet). Kattints rá jobb gombbal, és a nézet az asztal fölé úszik; az egérrel végignézhetsz rajta. Minden közvetlenül az asztalon történik.",
    );
    w.recipe(GUN_STATION as ItemId);
    w.push(El::Head(name(RIFLE_BENCH as ItemId)));
    w.text(
        "The long guns (the AK-47) are put together, taken apart and cleaned at the rifle station: three blocks wide, it works just as the gun station does. The small station is for the handguns only.",
        "A hosszú fegyvereket (az AK-47-et) a puskaasztalon rakod össze, szeded szét és tisztítod: három blokk széles, és ugyanúgy működik, mint a fegyverasztal. A kis asztal csak a kézifegyvereké.",
    );
    w.recipe(RIFLE_BENCH as ItemId);
    w.head("On the table", "Az asztalon");
    w.text(
        "Lay anything from your inventory on the table where you click: a gun, its parts, attachments. Click it again to pick it up, or drag it somewhere else.",
        "Bármit rárakhatsz az asztalra a tárgylistádból oda, ahová kattintasz: fegyvert, alkatrészeket, kiegészítőket. Újra rákattintva felveszed, vagy odébb húzhatod.",
    );
    w.head("Assemble and take apart", "Összerakás és szétszedés");
    w.text(
        "Right-click a gun on the table and it comes apart there; its magazine goes back into your inventory. Right-click one of the parts and the parts on the table (frame, barrel, recoil spring and slide) fly to the middle and go together.",
        "Jobb klikk egy fegyverre az asztalon, és ott szétszedi; a tára visszakerül a tárgylistádba. Jobb klikk valamelyik alkatrészre, és az asztalon lévő alkatrészek (váz, cső, helyretoló rugó és szán) középre repülnek és összeállnak.",
    );
    w.head("Clean", "Tisztítás");
    w.text(
        "Every shot makes a gun dirtier, and you can see it on the gun. A gun that is too dirty jams and will not fire. Take the brush out of the drawer and hold the left mouse button on a part to scrub it clean; a whole gun cleans too, but slowly. Right-click puts the brush back.",
        "Minden lövéstől koszosabb lesz a fegyver, és ez látszik is rajta. A túl koszos beakad, nem lő. Vedd ki a kefét a fiókból, és a bal egérgombot nyomva tartva sikáld tisztára az alkatrészeket; egészben is lehet, de lassan. Jobb klikkel visszateszed a kefét.",
    );
    w.head("Attachments", "Kiegészítők");
    w.text(
        "Drag an attachment onto a gun lying on the table and it goes on. Click one on the gun to take it off: it is laid beside the gun.",
        "Húzz egy kiegészítőt az asztalon fekvő fegyverre, és felmegy rá. A fegyveren lévőre kattintva leveszed: a fegyver mellé kerül.",
    );
    w.head("Loading magazines", "Tárak töltése");
    w.text(
        "Move the mouse down to the drawer: on its right are three boxes of rounds, 128 in each, the count written on them. Drop rounds from your inventory into a box, click a box to take a round out (right-click: a magazine's worth), and drag rounds onto a magazine lying on the table: they are pushed in one by one. The witness holes on its side show how many are in it.",
        "Vidd le az egeret a fiókhoz: a jobb oldalán három doboz töltény van, mindegyikben 128 fér el, a szám rá van írva. A tárgylistádból dobj töltényt a dobozba, kattints egy dobozra, hogy kivegyél egy töltényt (jobb klikk: egy tárnyit), és húzd a töltényeket az asztalon fekvő tárra: egyenként belenyomja őket. Az oldalán lévő lyukakban látszik, mennyi van benne.",
    );
}
