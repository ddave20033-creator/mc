//! UI translations (English / Hungarian). Strings are looked up by key; `{}` placeholders
//! are filled in order by `tf`.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static HUNGARIAN: AtomicBool = AtomicBool::new(false);

pub fn set_hungarian(hu: bool) {
    HUNGARIAN.store(hu, Ordering::Relaxed);
}

pub fn is_hungarian() -> bool {
    HUNGARIAN.load(Ordering::Relaxed)
}

/// (key, English, Hungarian)
const TABLE: &[(&str, &str, &str)] = &[
    // Main menu
    ("menu.singleplayer", "Singleplayer", "Egyjátékos"),
    ("menu.multiplayer", "Multiplayer", "Többjátékos"),
    ("menu.skin", "Skin", "Skin"),
    ("skin.classic", "Classic", "Klasszikus"),
    ("skin.forest", "Forest", "Erdő"),
    ("skin.red", "Red", "Piros"),
    ("skin.night", "Night", "Éjszaka"),
    ("skin.custom", "Custom", "Saját"),
    ("skin.upload", "Upload PNG", "PNG feltöltése"),
    ("menu.credits", "Credits", "Készítők"),
    ("menu.options", "Options...", "Beállítások..."),
    ("menu.quit", "Quit Game", "Kilépés"),
    ("menu.edition", "VULKAN EDITION", "VULKAN KIADÁS"),
    (
        "menu.madewith",
        "Made with Rust + Vulkan",
        "Rust + Vulkan alapokon",
    ),
    // Options
    ("opt.title", "Options", "Beállítások"),
    ("opt.tab.graphics", "Graphics", "Grafika"),
    ("opt.tab.controls", "Controls", "Irányítás"),
    ("opt.tab.sound", "Sound", "Hang"),
    ("opt.tab.interface", "Interface", "Felület"),
    ("opt.l.volume", "Master Volume", "Fő hangerő"),
    ("opt.l.volume_weapons", "Weapons", "Fegyverek"),
    ("opt.l.volume_other", "Other Sounds", "Egyéb hangok"),
    ("opt.l.render", "Render Distance", "Látótávolság"),
    ("opt.l.fov", "Field of View", "Látószög"),
    ("opt.l.shadows", "Shadows", "Árnyékok"),
    ("opt.l.clouds", "Clouds", "Felhők"),
    ("opt.l.fullscreen", "Fullscreen", "Teljes képernyő"),
    ("opt.l.fps_limit", "Max FPS", "FPS-korlát"),
    ("opt.v.unlimited", "Unlimited", "Korlátlan"),
    ("opt.l.aa", "Anti-aliasing", "Élsimítás"),
    ("opt.l.sens", "Mouse Sensitivity", "Egér érzékenység"),
    ("opt.l.bobbing", "View Bobbing", "Fejmozgás"),
    ("opt.l.fp_body", "First Person Body", "Test belső nézetben"),
    ("opt.l.language", "Language", "Nyelv"),
    ("opt.l.gui", "GUI Scale", "Felület mérete"),
    ("opt.l.dark_ui", "Dark Mode", "Sötét mód"),
    ("opt.l.fps", "Show FPS", "FPS kijelzése"),
    ("opt.v.normal", "Normal", "Normál"),
    ("opt.v.auto", "Auto", "Automatikus"),
    ("opt.v.language", "English", "Magyar"),
    (
        "opt.d.render",
        "How far you can see. Lower is faster.",
        "Milyen messzire látsz. Kisebb = gyorsabb.",
    ),
    (
        "opt.d.fov",
        "How wide the view is.",
        "Mennyire széles a látómező.",
    ),
    (
        "opt.d.shadows",
        "Sun and moon shadows. Off is faster.",
        "Nap- és holdárnyékok. Kikapcsolva gyorsabb.",
    ),
    ("opt.d.clouds", "Clouds in the sky.", "Felhők az égen."),
    (
        "opt.d.fullscreen",
        "Fill the whole screen (F11).",
        "Az egész képernyőt kitölti (F11).",
    ),
    (
        "opt.d.fps_limit",
        "Frames per second at most. Less heat and noise.",
        "Legfeljebb ennyi képkocka/mp. Kevesebb melegedés, zaj.",
    ),
    (
        "opt.d.aa",
        "Smooths jagged block edges (MSAA). Higher is slower.",
        "Kisimítja a recés blokkéleket (MSAA). Több = lassabb.",
    ),
    (
        "opt.d.sens",
        "How fast the camera turns with the mouse.",
        "Milyen gyorsan fordul a kamera az egérrel.",
    ),
    (
        "opt.d.bobbing",
        "The view bobs while walking.",
        "Járás közben billeg a kép.",
    ),
    (
        "opt.d.volume",
        "How loud the game is overall.",
        "A játék összes hangjának hangereje.",
    ),
    (
        "opt.d.volume_weapons",
        "Guns, grenades and explosions.",
        "Fegyverek, gránátok és robbanások.",
    ),
    (
        "opt.d.volume_other",
        "Fire, furnaces, armor and the rest.",
        "Tűz, kemencék, páncél és a többi.",
    ),
    (
        "opt.d.fp_body",
        "See your own body when looking down.",
        "Lefelé nézve látod a saját tested.",
    ),
    (
        "opt.d.language",
        "Menu and chat language.",
        "A menük és a chat nyelve.",
    ),
    (
        "opt.d.gui",
        "Size of menus and the HUD.",
        "A menük és a kijelzők mérete.",
    ),
    (
        "opt.d.dark_ui",
        "Dark theme for the inventory screens.",
        "Sötét téma az inventory ablakokhoz.",
    ),
    (
        "opt.d.fps",
        "Frames per second in the top left corner.",
        "Képkocka/másodperc a bal felső sarokban.",
    ),
    ("opt.l.packs", "Resource Packs", "Textúracsomagok"),
    ("opt.v.press_key", "Press a key", "Nyomj egy gombot"),
    ("opt.l.keys", "Key Binds", "Billentyűk"),
    ("opt.v.keys", "Change...", "Beállítás..."),
    (
        "opt.d.keys",
        "Which key does what.",
        "Melyik gomb mit csinál.",
    ),
    ("keys.title", "Key Binds", "Billentyűk"),
    ("keys.cat.movement", "Movement", "Mozgás"),
    ("keys.cat.inventory", "Inventory", "Tárgyak"),
    ("keys.cat.multiplayer", "Chat and Multiplayer", "Chat és többjátékos"),
    ("keys.cat.view", "View", "Nézet"),
    ("keys.reset", "Reset", "Alap"),
    ("keys.reset_all", "Reset All Keys", "Mind alapra"),
    ("keys.conflict", "Also used by:", "Ezt használja még:"),
    (
        "opt.d.key",
        "Click, then press the new key. Esc: cancel.",
        "Kattints, majd nyomd meg az új gombot. Esc: mégse.",
    ),
    ("key.forward", "Walk Forwards", "Előre"),
    ("key.back", "Walk Backwards", "Hátra"),
    ("key.left", "Strafe Left", "Balra"),
    ("key.right", "Strafe Right", "Jobbra"),
    ("key.jump", "Jump", "Ugrás"),
    ("key.sneak", "Sneak", "Lopakodás"),
    ("key.sprint", "Sprint", "Futás"),
    ("key.zoom", "Zoom", "Nagyítás"),
    ("key.inventory", "Inventory", "Inventory"),
    ("key.drop", "Drop Item", "Tárgy eldobása"),
    ("key.reload", "Reload Gun", "Fegyver újratöltése"),
    ("key.inspect", "Inspect Gun", "Fegyver megnézése"),
    ("key.gunlight", "Weapon Light", "Fegyverlámpa ki/be"),
    ("key.chat", "Open Chat", "Chat"),
    ("key.command", "Open Command", "Parancs"),
    ("key.playerlist", "List Players", "Játékoslista"),
    ("key.fly", "Fly (Creative)", "Repülés (kreatív)"),
    ("key.perspective", "Toggle Perspective", "Nézetváltás"),
    ("key.hidehud", "Hide HUD", "HUD elrejtése"),
    ("key.debug", "Debug Screen", "Debug képernyő"),
    ("key.fullscreen", "Fullscreen", "Teljes képernyő"),
    ("key.hotbar1", "Hotbar Slot 1", "Gyorsmenü 1"),
    ("key.hotbar2", "Hotbar Slot 2", "Gyorsmenü 2"),
    ("key.hotbar3", "Hotbar Slot 3", "Gyorsmenü 3"),
    ("key.hotbar4", "Hotbar Slot 4", "Gyorsmenü 4"),
    ("key.hotbar5", "Hotbar Slot 5", "Gyorsmenü 5"),
    ("key.hotbar6", "Hotbar Slot 6", "Gyorsmenü 6"),
    ("key.hotbar7", "Hotbar Slot 7", "Gyorsmenü 7"),
    ("key.hotbar8", "Hotbar Slot 8", "Gyorsmenü 8"),
    ("key.hotbar9", "Hotbar Slot 9", "Gyorsmenü 9"),
    ("opt.v.packs", "Open...", "Megnyitás..."),
    (
        "opt.d.packs",
        "Minecraft resource packs over the built-in textures.",
        "Minecraft textúracsomagok a beépített textúrák fölé.",
    ),
    ("packs.title", "Select Resource Packs", "Textúracsomagok kiválasztása"),
    (
        "packs.hint",
        "Put Minecraft resource packs (.zip) in the folder",
        "Tedd a Minecraft textúracsomagokat (.zip) a mappába",
    ),
    ("packs.available", "Available", "Elérhető"),
    ("packs.selected", "Selected", "Kiválasztott"),
    (
        "packs.empty",
        "No resource packs yet. Open the folder and put some in.",
        "Még nincs textúracsomag. Nyisd meg a mappát, és tegyél bele.",
    ),
    ("packs.folder", "Open Pack Folder", "Mappa megnyitása"),
    ("packs.add", "Select", "Kiválasztás"),
    ("packs.remove", "Deselect", "Eltávolítás"),
    ("packs.up", "Move up", "Feljebb"),
    ("packs.down", "Move down", "Lejjebb"),
    ("packs.builtin", "Built-in, always on", "Beépített, mindig aktív"),
    (
        "packs.builtin_tip",
        "The packs above it override its textures",
        "A fölötte lévő csomagok felülírják a textúráit",
    ),
    ("credits.textures", "Textures", "Textúrák"),
    ("credits.3_pack", "Procedural terrain", "Generált terep"),
    (
        "credits.open_link",
        "Open in browser",
        "Megnyitás böngészőben",
    ),
    ("opt.on", "ON", "BE"),
    ("opt.off", "OFF", "KI"),
    ("gui.done", "Done", "Kész"),
    ("gui.cancel", "Cancel", "Mégse"),
    ("gui.back", "Back", "Vissza"),
    // Pause / death / loading
    ("pause.title", "Game Menu", "Játékmenü"),
    ("pause.resume", "Back to Game", "Vissza a játékba"),
    ("pause.lan", "Open to LAN", "Megnyitás LAN-ra"),
    ("pause.lan_open", "LAN:", "LAN:"),
    ("pause.disconnect", "Disconnect", "Lecsatlakozás"),
    // LAN multiplayer
    ("mp.title", "Play Multiplayer", "Többjátékos"),
    ("mp.name", "Player name:", "Játékosnév:"),
    (
        "mp.lan_games",
        "Games on your network",
        "Játékok a hálózaton",
    ),
    (
        "mp.searching",
        "Searching for LAN games",
        "LAN játékok keresése",
    ),
    (
        "mp.search_failed",
        "LAN search is not available here, use Direct Connect",
        "A LAN keresés itt nem megy, használd a közvetlen kapcsolódást",
    ),
    (
        "mp.other_version",
        "Different game version",
        "Más játékverzió",
    ),
    ("mp.direct", "Direct Connect:", "Közvetlen IP:"),
    ("mp.connect", "Connect", "Kapcsolódás"),
    ("mp.join", "Join Game", "Csatlakozás"),
    ("mp.connecting", "Connecting...", "Csatlakozás..."),
    ("mp.connecting_to", "Joining {}", "Kapcsolódás ide: {}"),
    (
        "mp.connect_failed",
        "Could not connect: {}",
        "Nem sikerült kapcsolódni: {}",
    ),
    ("mp.disconnected", "Disconnected", "Kapcsolat bontva"),
    (
        "mp.lost",
        "Lost connection to the host",
        "Megszakadt a kapcsolat a gazdával",
    ),
    (
        "mp.back_to_menu",
        "Back to Title Screen",
        "Vissza a főmenübe",
    ),
    (
        "lan.opened",
        "Local game hosted at {} - others can join from Multiplayer",
        "Helyi játék megnyitva: {} - a többiek a Többjátékos menüben csatlakozhatnak",
    ),
    (
        "lan.failed",
        "Could not open to LAN: {}",
        "Nem sikerült megnyitni LAN-ra: {}",
    ),
    (
        "lan.host_left",
        "The host closed the world",
        "A gazda bezárta a világot",
    ),
    (
        "lan.bad_version",
        "The host runs a different game version",
        "A gazdánál más játékverzió fut",
    ),
    (
        "lan.bad_name",
        "Choose a player name first",
        "Előbb adj meg egy játékosnevet",
    ),
    (
        "lan.name_taken",
        "Someone with this name is already playing",
        "Ezzel a névvel már játszik valaki",
    ),
    (
        "lan.joined",
        "{} joined the game",
        "{} csatlakozott a játékhoz",
    ),
    ("lan.players", "Players online: {}", "Játékosok: {}"),
    ("lan.host_tag", "(host)", "(gazda)"),
    ("lan.left", "{} left the game", "{} kilépett a játékból"),
    (
        "death.player",
        "Player was slain by another player",
        "Játékost megölte egy másik játékos",
    ),
    (
        "death.explosion",
        "Player blew up",
        "Játékos felrobbant",
    ),
    (
        "death.wolf",
        "Player was mauled by a wolf",
        "Játékost széttépte egy farkas",
    ),
    (
        "pause.quit",
        "Save and Quit to Title",
        "Mentés és kilépés a főmenübe",
    ),
    ("bed.spawn_set", "Respawn point set", "Újraéledési pont beállítva"),
    (
        "bed.no_sleep",
        "You can sleep only at night",
        "Csak éjszaka lehet aludni",
    ),
    ("bed.occupied", "This bed is occupied", "Ez az ágy foglalt"),
    (
        "bed.missing",
        "You have no home bed, or it was obstructed",
        "Nincs otthoni ágyad, vagy el van torlaszolva",
    ),
    ("bed.leave", "Sneak to leave the bed", "Guggolj a felkeléshez"),
    // A felled trunk lying on the ground, aimed at with an axe
    ("log.trunk", "Trunk", "Törzs"),
    ("log.blocks", "blocks long", "blokk hosszú"),
    ("log.cut", "cut", "vágás"),
    // Fishing
    ("fish.caught", "You caught a {} kg {}!", "Fogtál egy {} kg-os {}!"),
    ("fish.bite", "A bite! Reel in!", "Kapás! Tekerj!"),
    ("fish.missed", "Too late, the fish is gone.", "Elkésett a bevágás, a hal elúszott."),
    ("fish.escaped", "The fish got off the hook.", "Leakadt a hal."),
    ("fish.snap", "The line snapped!", "Elszakadt a damil!"),
    ("fish.rod_broke", "Your fishing rod broke.", "Eltört a horgászbotod."),
    (
        "bed.waiting",
        "{} of {} players sleeping",
        "{}/{} játékos alszik",
    ),
    ("death.title", "You died!", "Meghaltál!"),
    ("death.respawn", "Respawn", "Újraéledés"),
    ("death.title_screen", "Title Screen", "Főmenü"),
    ("loading.generating", "Generating world", "Világ generálása"),
    ("loading.saving", "Saving world", "Világ mentése"),
    (
        "tip.1",
        "Tip: /gamemode creative lets you fly (needs cheats)",
        "Tipp: /gamemode creative módban repülhetsz (csalás kell)",
    ),
    (
        "tip.2",
        "Tip: press T to chat, Tab completes commands",
        "Tipp: T a chat, Tab kiegészíti a parancsokat",
    ),
    (
        "tip.3",
        "Tip: punch a tree, craft planks, then a crafting table",
        "Tipp: üss ki egy fát, barkácsolj deszkát, majd asztalt",
    ),
    (
        "tip.4",
        "Tip: iron ore needs at least a copper pickaxe",
        "Tipp: a vasérchez legalább rézcsákány kell",
    ),
    (
        "tip.5",
        "Tip: F5 switches to third person view",
        "Tipp: F5-tel harmadik személyű nézetre válthatsz",
    ),
    (
        "tip.6",
        "Tip: smelt iron ore in a furnace to get iron ingots",
        "Tipp: a vasércet kemencében égesd vasrúddá",
    ),
    // Credits
    (
        "credits.1",
        "A voxel sandbox made from scratch",
        "Egy nulláról megírt kockajáték",
    ),
    (
        "credits.2",
        "Custom Vulkan engine in Rust",
        "Saját Vulkan motor Rustban",
    ),
    (
        "credits.3",
        "Procedural terrain and textures",
        "Generált terep és textúrák",
    ),
    (
        "credits.4",
        "Inspired by Minecraft.",
        "A Minecraft ihlette.",
    ),
    (
        "credits.5",
        "Not affiliated with Mojang or Microsoft.",
        "Nem kapcsolódik a Mojanghoz vagy a Microsofthoz.",
    ),
    // World selection / creation
    ("worlds.title", "Select World", "Világ kiválasztása"),
    ("worlds.play", "Play Selected World", "Világ indítása"),
    ("worlds.create", "Create New World", "Új világ létrehozása"),
    ("worlds.delete", "Delete", "Törlés"),
    ("worlds.play_short", "Play", "Játék"),
    ("worlds.create_short", "Create", "Létrehozás"),
    (
        "worlds.empty",
        "No worlds yet - create one!",
        "Még nincs világod - hozz létre egyet!",
    ),
    ("worlds.cheats", "Cheats", "Csalások"),
    (
        "worlds.delete_q",
        "Are you sure you want to delete this world?",
        "Biztosan törlöd ezt a világot?",
    ),
    (
        "worlds.delete_warn",
        "'{}' will be lost forever! (A long time!)",
        "'{}' örökre elvész! (Az hosszú idő!)",
    ),
    ("create.name", "World Name", "Világ neve"),
    ("create.default_name", "New World", "Új világ"),
    (
        "create.seed",
        "Seed for the World Generator",
        "Seed a világgenerátorhoz",
    ),
    (
        "create.seed_hint",
        "Leave blank for random",
        "Üresen hagyva véletlen",
    ),
    ("create.mode", "Game Mode: {}", "Játékmód: {}"),
    (
        "create.cheats",
        "Allow Cheats: {}",
        "Csalások engedélyezése: {}",
    ),
    (
        "create.survival_desc",
        "Search for resources, craft tools, survive",
        "Gyűjts nyersanyagot, barkácsolj, maradj életben",
    ),
    (
        "create.creative_desc",
        "Unlimited resources, free flying, instant breaking",
        "Végtelen nyersanyag, repülés, azonnali bontás",
    ),
    ("mode.survival", "Survival", "Túlélő"),
    ("mode.creative", "Creative", "Kreatív"),
    ("mode.survival_long", "Survival Mode", "Túlélő mód"),
    ("mode.creative_long", "Creative Mode", "Kreatív mód"),
    ("mode.spectator", "Spectator", "Néző"),
    ("mode.spectator_long", "Spectator Mode", "Néző mód"),
    // Spectator mode
    ("spectate.title", "Spectate a player", "Játékos nézése"),
    (
        "spectate.none",
        "No other players to watch",
        "Nincs más játékos, akit nézhetnél",
    ),
    (
        "spectate.single",
        "Open the world to LAN to watch others",
        "Nyisd meg a világot LAN-ra, hogy másokat nézhess",
    ),
    ("spectate.current", "watching", "nézed"),
    ("spectate.stop", "Stop watching", "Nézés abbahagyása"),
    ("spectate.now", "Now watching {}", "Most őt nézed: {}"),
    ("spectate.stopped", "Stopped watching", "Már nem nézel senkit"),
    ("spectate.lost", "The player you watched is gone", "A nézett játékos eltűnt"),
    ("spectate.watching", "Watching {}", "{} nézése"),
    (
        "spectate.hint",
        "E: players to watch  |  Space/Shift: up/down",
        "E: játékosok nézése  |  Szóköz/Shift: fel/le",
    ),
    (
        "spectate.hint_watching",
        "Shift: stop watching  |  E: someone else",
        "Shift: kilépés  |  E: másik játékos",
    ),
    (
        "spectate.no_player",
        "No player to watch named {}",
        "Nincs ilyen nevű játékos: {}",
    ),
    (
        "spectate.not_spectator",
        "Only in spectator mode (/gamemode spectator)",
        "Csak néző módban (/gamemode spectator)",
    ),
    // Containers
    ("gui.inventory", "Inventory", "Tárgylista"),
    ("gui.crafting", "Crafting", "Barkácsolás"),
    ("gui.furnace", "Furnace", "Kemence"),
    ("gui.chest", "Chest", "Láda"),
    ("gui.large_chest", "Large Chest", "Nagy láda"),
    ("gui.creative", "Creative Items", "Kreatív tárgyak"),
    ("gui.trash", "Destroy Item", "Tárgy megsemmisítése"),
    ("gui.search", "Search...", "Keresés..."),
    ("gui.no_results", "No items found", "Nincs találat"),
    ("gui.tab.blocks", "Blocks", "Blokkok"),
    ("gui.tab.functional", "Functional Blocks", "Használati blokkok"),
    ("gui.tab.tools", "Tools & Weapons", "Eszközök és fegyverek"),
    ("gui.tab.armor", "Armor", "Ruházat"),
    ("gui.tab.all", "All Items", "Minden"),
    ("jei.none", "Found in the world, not crafted.", "A világban található, nem barkácsolható."),
    ("jei.furnace", "Furnace", "Kemence"),
    ("jei.blast", "Blast Furnace", "Kohó"),
    ("jei.advanced", "Advanced Furnace", "Fejlett kohó"),
    ("jei.hint_survival", "Click: how it is made", "Kattints: hogyan készül"),
    ("jei.hint_creative", "Click: take it | Right click: recipe", "Kattintás: elveszed | Jobb klikk: recept"),
    ("gui.tab.food", "Food & Drinks", "Ételek és italok"),
    ("gui.tab.mobs", "Mobs", "Mobok"),
    ("gui.tab.materials", "Materials", "Alapanyagok"),
    ("gui.tab.search", "Search results", "Keresési találatok"),
    (
        "gui.durability",
        "Durability: {} / {}",
        "Tartósság: {} / {}",
    ),
    // Gun station and the pistol
    ("gui.gun_station", "Gun Station", "Fegyverasztal"),
    ("gun.assemble_1", "Assemble", "Fegyver"),
    ("gun.assemble_2", "Gun", "összerakás"),
    ("gun.clean_1", "Clean", "Fegyver"),
    ("gun.clean_2", "Gun", "tisztítás"),
    ("gun.parts", "Parts", "Alkatrészek"),
    ("gun.part.frame", "Frame", "Váz"),
    ("gun.part.barrel", "Barrel", "Cső"),
    ("gun.part.spring", "Spring", "Rugó"),
    ("gun.part.slide", "Slide", "Szán"),
    ("gun.part.magazine", "Magazine", "Tár"),
    ("gun.gun", "Gun", "Fegyver"),
    ("gun.next", "Next: {} - click it!", "Következő: {} - kattints rá!"),
    (
        "gun.missing",
        "Missing: {} (what it takes is on the right)",
        "Hiányzik hozzá: {} (jobb oldalt látod, mi kell)",
    ),
    (
        "gun.done",
        "Done! The gun is in your inventory.",
        "Kész! A fegyver a tárgylistádba került.",
    ),
    ("gun.again", "Click to build another one.", "Kattints egy újabbhoz."),
    (
        "gun.put_gun",
        "Hold a gun, or put one on the table (shift-click it in the inventory).",
        "Vegyél kézbe egy fegyvert, vagy tedd az asztalra (shift+kattintás a tárgylistában)!",
    ),
    (
        "gun.scrub",
        "Hold the left mouse button on the parts to scrub them.",
        "Tartsd lenyomva a bal egérgombot az alkatrészeken a sikáláshoz!",
    ),
    ("gun.clean", "The gun is clean!", "A fegyver tiszta!"),
    ("gun.cleanliness", "Clean: {}%", "Tisztaság: {}%"),
    ("gun.rotate", "Right drag: turn", "Jobb gomb: forgatás"),
    (
        "gun.jammed",
        "The gun jammed! Clean it at a gun station.",
        "Beakadt a fegyver! Tisztítsd meg a fegyverasztalon.",
    ),
    ("gun.no_ammo", "No ammunition for it!", "Nincs hozzá töltény!"),
    ("gun.empty_reload", "Empty! Reload: {}", "Kiürült! Újratöltés: {}"),
    (
        "gun.loader_rifle_station",
        "The magazine loader goes into the rifle station's drawer.",
        "A tárazógép a puskaasztal fiókjába való.",
    ),
    (
        "gun.rifle_station_only",
        "The long guns are worked on at the rifle station.",
        "A hosszú fegyvereket a puskaasztalon rakhatod össze.",
    ),
    (
        "gun.controls_auto",
        "LMB (hold): shoot  |  RMB: aim  |  {}: reload",
        "Bal (nyomva): sorozat  |  Jobb: célzás  |  {}: újratöltés",
    ),
    ("gun.tuning_1", "Tune", "Fegyver"),
    ("gun.tuning_2", "Gun", "tuning"),
    ("gun.attachments", "Attachments", "Kiegészítők"),
    (
        "gun.tune_hint",
        "Click an attachment beside the gun: it goes on. Click one on the gun to take it off.",
        "Kattints egy kiegészítőre a fegyver mellett, és felkerül. A fegyveren lévőt rákattintva leveszed.",
    ),
    (
        "gun.no_attachments",
        "You have no attachments for it yet: craft some.",
        "Még nincs hozzá kiegészítőd: barkácsolj egyet!",
    ),
    (
        "gun.tune_ready",
        "Take an attachment off by clicking it.",
        "Egy kiegészítőt rákattintva szedhetsz le.",
    ),
    ("gun.mod.scope", "Scope: zooms in far when aiming", "Távcső: nagy nagyítás célzáskor"),
    ("gun.mod.silencer", "Silencer: no muzzle flash", "Hangtompító: nincs torkolattűz"),
    (
        "gun.mod.extended",
        "Extended magazine: 20 rounds instead of 12",
        "Bővített tár: 12 helyett 20 töltény",
    ),
    (
        "gun.mod.laser",
        "Laser sight: steadier from the hip",
        "Lézeres célzó: pontosabb csípőből",
    ),
    ("gun.reloading", "Reloading...", "Újratöltés..."),
    ("gun.empty", "Empty! Press {} to reload.", "Üres a tár! {} = újratöltés"),
    ("gun.no_mag", "No magazine! {} = put one in", "Nincs tár a fegyverben! {} = tár be"),
    (
        "gun.no_mags",
        "No loaded magazine! Right-click with a magazine to fill it.",
        "Nincs töltött tárad! Tárral a kezedben jobb klikk: megtöltöd.",
    ),
    ("gun.mag_full", "The magazine is full.", "A tár tele van."),
    ("gun.mag_hint", "load it at the gun station", "a fegyverasztalon töltheted meg"),
    (
        "gun.controls",
        "LMB: shoot  |  RMB: aim  |  {}: reload",
        "Bal: lövés  |  Jobb: célzás  |  {}: újratöltés",
    ),
    ("gun.magazine", "Magazine: {}/{}", "Tár: {}/{}"),
    // HUD
    (
        "hud.hint",
        "WASD move | LMB mine | RMB use/place | E inventory | T chat | F5 view",
        "WASD mozgás | Bal: bányászás | Jobb: használat | E tárgylista | T chat | F5 nézet",
    ),
    // Chat / commands
    (
        "chat.welcome",
        "Welcome! Press T to chat, /help for commands.",
        "Üdv! T-vel chatelhetsz, /help a parancsokhoz.",
    ),
    (
        "cmd.unknown",
        "Unknown command. Type /help for help.",
        "Ismeretlen parancs. Írd be: /help",
    ),
    (
        "cmd.no_cheats",
        "Cheats are not enabled in this world.",
        "Ebben a világban nincsenek engedélyezve a csalások.",
    ),
    ("cmd.help", "--- Commands ---", "--- Parancsok ---"),
    ("cmd.time_set", "Set the time to {}", "Idő beállítva: {}"),
    ("cmd.time_add", "Added {} to the time", "Idő hozzáadva: {}"),
    ("cmd.time_query", "The time is {}", "Az idő: {}"),
    ("cmd.bad_time", "Invalid time: {}", "Érvénytelen idő: {}"),
    (
        "cmd.bad_number",
        "Invalid number: {}",
        "Érvénytelen szám: {}",
    ),
    (
        "cmd.gamemode",
        "Set own game mode to {}",
        "Saját játékmód beállítva: {}",
    ),
    (
        "cmd.bad_mode",
        "Unknown game mode: {}",
        "Ismeretlen játékmód: {}",
    ),
    (
        "cmd.tp",
        "Teleported Player to {}, {}, {}",
        "Játékos teleportálva ide: {}, {}, {}",
    ),
    (
        "cmd.bad_coords",
        "Invalid coordinates",
        "Érvénytelen koordináták",
    ),
    (
        "cmd.spawn",
        "Teleported to spawn",
        "Teleportálva a kezdőpontra",
    ),
    ("cmd.seed", "Seed: [{}]", "Seed: [{}]"),
    (
        "cmd.give",
        "Gave {} [{}] to Player",
        "{} db [{}] átadva a játékosnak",
    ),
    ("cmd.bad_item", "Unknown item: {}", "Ismeretlen tárgy: {}"),
    ("cmd.summon", "Summoned new {}", "Megidézve: {}"),
    (
        "cmd.bad_entity",
        "Unknown entity: {}",
        "Ismeretlen entitás: {}",
    ),
    ("cmd.effect", "Applied effect: {}", "Hatás megadva: {}"),
    (
        "cmd.effect_clear",
        "Removed all effects",
        "Minden hatás törölve",
    ),
    (
        "cmd.bad_effect",
        "Unknown effect: {}",
        "Ismeretlen hatás: {}",
    ),
    ("cmd.saved", "Saved the game", "Játék elmentve"),
    // Death messages
    (
        "death.fall",
        "Player hit the ground too hard",
        "Játékos túl nagyot esett",
    ),
    (
        "death.lava",
        "Player tried to swim in lava",
        "Játékos megpróbált úszni a lávában",
    ),
    (
        "death.fire",
        "Player burned to death",
        "Játékos halálra égett",
    ),
    (
        "death.cactus",
        "Player was pricked to death",
        "Játékost halálra szurkálta egy kaktusz",
    ),
    (
        "death.suffocate",
        "Player suffocated in a wall",
        "Játékos megfulladt a falban",
    ),
    (
        "death.void",
        "Player fell out of the world",
        "Játékos kiesett a világból",
    ),
    ("death.drown", "Player drowned", "Játékos vízbe fulladt"),
    ("death.kill", "Player was killed", "Játékost megölték"),
    (
        "death.starve",
        "Player starved to death",
        "Játékos éhen halt",
    ),
    (
        "death.thirst",
        "Player died of thirst",
        "Játékos szomjan halt",
    ),
    (
        "death.poison",
        "Player was poisoned",
        "Játékos megmérgeződött",
    ),
    ("effect.poison", "Poison", "Mérgezés"),
    ("effect.nausea", "Nausea", "Hányinger"),
];

fn map() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    static MAP: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();
    MAP.get_or_init(|| TABLE.iter().map(|&(k, en, hu)| (k, (en, hu))).collect())
}

/// Translated string for `key` (the key itself if missing).
pub fn t(key: &'static str) -> &'static str {
    match map().get(key) {
        Some(&(en, hu)) => {
            if is_hungarian() {
                hu
            } else {
                en
            }
        }
        None => key,
    }
}

/// Translated string with `{}` placeholders filled in order.
pub fn tf(key: &'static str, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let mut rest = t(key);
    for a in args {
        match rest.find("{}") {
            Some(i) => {
                out.push_str(&rest[..i]);
                out.push_str(&a.to_string());
                rest = &rest[i + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

pub fn on_off(b: bool) -> &'static str {
    if b {
        t("opt.on")
    } else {
        t("opt.off")
    }
}
