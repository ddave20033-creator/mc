//! What the guide book says and how it is laid out on its pages: the chapters (with the
//! recipes, furnace times, fuels and gun numbers read from the game itself, so it always
//! tells how things work now), in English or Hungarian, drawn page by page onto a canvas
//! in a light or a dark theme, with small looping animations in the pictures.

use super::canvas::Canvas;
use crate::entity::block_entity::{BURN_TIME, FLIP_TIME, GRILL_TIME};
use crate::entity::Furnace;
use crate::item::*;
use crate::ui::{rgba, Color, Font};
use crate::world::textures::tex;
use crate::world::*;
use glam::Vec2;

/// A page's size and margins in book units.
const PAGE_W: f32 = 168.0;
const PAGE_H: f32 = 216.0;
const MARGIN_X: f32 = 13.0;
const MARGIN_TOP: f32 = 13.0;
const MARGIN_BOTTOM: f32 = 21.0;
/// Pixels per book unit on a page's texture, and the texture's size: 2 x 3 block texture
/// layers (the page fills 255 x 328 of it).
pub const U: f32 = 1.52;
pub const SHEET_W: usize = 256;
pub const SHEET_H: usize = 384;
/// The part of the texture the page fills.
pub const PAGE_PX: (f32, f32) = (PAGE_W * U, PAGE_H * U);

/// The page's colors.
pub struct Theme {
    paper: Color,
    edge: Color,
    ink: Color,
    soft: Color,
    red: Color,
    gold: Color,
    slot_edge: Color,
    slot_fill: Color,
    bar_bg: Color,
    bar: Color,
    hover: Color,
}

pub const LIGHT: Theme = Theme {
    paper: rgba(243, 234, 210, 255),
    edge: rgba(150, 120, 80, 255),
    ink: rgba(56, 42, 32, 255),
    soft: rgba(118, 98, 76, 255),
    red: rgba(150, 44, 32, 255),
    gold: rgba(200, 150, 50, 255),
    slot_edge: rgba(170, 148, 112, 255),
    slot_fill: rgba(228, 214, 184, 255),
    bar_bg: rgba(222, 208, 176, 255),
    bar: rgba(176, 64, 44, 255),
    hover: rgba(150, 44, 32, 40),
};

pub const DARK: Theme = Theme {
    paper: rgba(40, 38, 46, 255),
    edge: rgba(0, 0, 0, 255),
    ink: rgba(222, 216, 204, 255),
    soft: rgba(150, 144, 136, 255),
    red: rgba(240, 132, 104, 255),
    gold: rgba(226, 180, 74, 255),
    slot_edge: rgba(20, 19, 24, 255),
    slot_fill: rgba(64, 62, 72, 255),
    bar_bg: rgba(64, 62, 72, 255),
    bar: rgba(232, 112, 84, 255),
    hover: rgba(240, 132, 104, 40),
};

/// Sizes on a page's texture: pixels per book unit, the body text's font scale and line
/// height, the titles', and the space inside the margins.
pub struct Metrics {
    pub u: f32,
    pub fs: f32,
    pub lh: f32,
    pub tfs: f32,
    pub tlh: f32,
    pub w: f32,
    pub h: f32,
}

pub fn metrics() -> Metrics {
    let u = U;
    let (fs, tfs) = (1.0, 2.0);
    Metrics {
        u,
        fs,
        lh: fs * 10.0,
        tfs,
        tlh: tfs * 10.0,
        w: (PAGE_W - 2.0 * MARGIN_X) * u,
        h: (PAGE_H - MARGIN_TOP - MARGIN_BOTTOM) * u,
    }
}

/// Pictures drawn on the pages.
#[derive(Clone)]
pub enum Pic {
    /// A furnace's blocks as they stand (FURNACE, BLAST_FURNACE or ADV_FURNACE).
    Furnace(Block),
    /// A furnace's front: the upper half takes what to smelt, the lower half the fuel.
    Front,
    /// The top of a furnace with meat on its corners.
    Grill,
    /// A gun, big.
    Gun(GunKind),
    /// The pickaxes from the first to the last, each over the hardest ore it mines.
    Tiers,
    /// The gun station.
    Station,
    /// A row of items.
    Items(Vec<ItemId>),
}

impl Pic {
    fn height(&self) -> f32 {
        match self {
            Pic::Furnace(FURNACE) => 36.0,
            Pic::Furnace(BLAST_FURNACE) => 52.0,
            Pic::Furnace(_) => 58.0,
            Pic::Front => 56.0,
            Pic::Grill => 62.0,
            Pic::Gun(_) => 66.0,
            Pic::Tiers => 44.0,
            Pic::Station => 50.0,
            Pic::Items(_) => 24.0,
        }
    }
}

/// What the book says, before it is laid out on pages.
enum El {
    /// Starts a new page with a big title, and goes in the contents (and on a tab, by its
    /// short name).
    Chapter(String, String),
    Head(String),
    Text(String),
    /// A paragraph after a dash.
    Bullet(String),
    /// The recipe making this item.
    Recipe(ItemId),
    /// What this item smelts into, in which furnace.
    Smelt(ItemId),
    /// An item with a few words beside it.
    Row(ItemId, String),
    Picture(Pic),
    /// A gun's number: its name, how big it is next to the other guns' (0..1), the value.
    Stat(String, f32, String),
    /// The rest goes on the next page.
    Break,
}

/// A piece of a page, laid out.
#[derive(Clone)]
pub enum Piece {
    Cover,
    Contents,
    Break,
    /// A chapter's title, and its short name for its tab.
    Title(Vec<String>, String),
    Head(String),
    /// A line of a paragraph; a bullet's lines are indented, its first after a dash.
    Line { text: String, indent: bool, dash: bool },
    Gap(f32),
    Recipe(ItemId),
    Smelt(ItemId),
    Row(ItemId, Vec<String>),
    Pic(Pic),
    Stat(String, f32, String),
}

impl Piece {
    /// It moves (the pictures, the recipes' and furnaces' arrows).
    fn animated(&self) -> bool {
        matches!(
            self,
            Piece::Cover | Piece::Pic(_) | Piece::Recipe(_) | Piece::Smelt(_)
        )
    }
}

/// Space for a bullet's dash (book units).
const INDENT: f32 = 8.0;

fn height(m: &Metrics, p: &Piece) -> f32 {
    let u = m.u;
    match p {
        Piece::Cover | Piece::Contents => m.h,
        Piece::Break => 0.0,
        Piece::Title(lines, _) => lines.len() as f32 * m.tlh + 9.0 * u,
        Piece::Head(_) => m.lh + 4.0 * u,
        Piece::Line { .. } => m.lh,
        Piece::Gap(g) => *g,
        Piece::Recipe(id) => {
            let rows = recipe_view(*id).map_or(1, |(r, _)| r.len());
            rows as f32 * 18.0 * u + m.lh + 8.0 * u
        }
        Piece::Smelt(_) => (20.0 * u).max(2.0 * m.lh + 2.0 * u),
        Piece::Row(_, lines) => (19.0 * u).max(lines.len() as f32 * m.lh + 4.0 * u),
        Piece::Pic(p) => p.height() * u,
        Piece::Stat(..) => m.lh.max(7.0 * u) + 2.0 * u,
    }
}

/// Text in the book's language.
fn tr(hu: bool, en: &str, hun: &str) -> String {
    (if hu { hun } else { en }).to_string()
}

/// A number as the language writes it (a decimal comma in Hungarian).
fn num(hu: bool, v: f32) -> String {
    let s = if (v - v.round()).abs() < 0.01 {
        format!("{}", v.round())
    } else if (v * 10.0 - (v * 10.0).round()).abs() < 0.01 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    if hu {
        s.replace('.', ",")
    } else {
        s
    }
}

fn smelt_time(tier: u8) -> f32 {
    Furnace {
        tier,
        ..Default::default()
    }
    .smelt_time()
}

fn furnace_of_tier(tier: u8) -> Block {
    match tier {
        3 => ADV_FURNACE,
        2 => BLAST_FURNACE,
        _ => FURNACE,
    }
}

/// Everything the book says.
fn content(hu: bool, keys: &(String, String)) -> Vec<El> {
    use El::*;
    let s = |en: &str, hun: &str| tr(hu, en, hun);
    let n = |v: f32| num(hu, v);
    let (inv, reload) = (keys.0.as_str(), keys.1.as_str());
    let mut shown: Vec<ItemId> = Vec::new();
    let mut v: Vec<El> = Vec::new();
    let mut recipe = |v: &mut Vec<El>, id: ItemId| {
        shown.push(id);
        v.push(Recipe(id));
    };

    // ------------------------------------------------------------ getting started
    v.push(Chapter(s("Getting Started", "Első lépések"), s("Start", "Kezdés")));
    v.push(Text(s(
        "Welcome! This book tells how things work in this world: crafting, the furnaces and the guns. Hold it and look down to bring it up to read. Right click turns the page, left click turns back (the arrow keys too). The numbers on the tabs along the top open the chapters: press 1 to 8 while reading.",
        "Üdv! Ez a könyv elmondja, hogyan működnek a dolgok ebben a világban: a barkácsolás, a kemencék és a fegyverek. Vedd a kezedbe, és nézz le: magad elé emeled és olvashatod. Jobb klikkel lapozol előre, bal klikkel vissza (a nyilakkal is). A fenti füleken lévő számok a fejezetek: olvasás közben az 1-8 gombbal odaugrasz.",
    )));
    v.push(Head(s("Your first tools", "Az első eszközök")));
    v.push(Text(s(
        "Punch a tree to get logs. Logs make planks, planks make sticks and a crafting table. At the table you can make a wooden pickaxe: with it, go for stone.",
        "Üss meg egy fát, hogy rönköt kapj. A rönkből deszka lesz, a deszkából bot és barkácsasztal. Az asztalon fa csákányt készíthetsz, azzal pedig jöhet a kő.",
    )));
    for id in [
        PLANKS as ItemId,
        STICK,
        CRAFTING_TABLE as ItemId,
        tool_id(ToolKind::Pickaxe, Tier::Wood),
    ] {
        recipe(&mut v, id);
    }
    v.push(Head(s("Crafting", "Barkácsolás")));
    v.push(Text(if hu {
        format!("Az {inv} gombbal nyílik a tárgylistád, benne egy 2x2-es barkácsráccsal. A nagyobb receptekhez a barkácsasztal 3x3-as rácsa kell: kattints rá jobb gombbal, és a nézet az asztal fölé úszik. Rakd a tárgyakat az asztal rácsára.")
    } else {
        format!("Press {inv} to open your inventory: it has a 2x2 crafting grid. Bigger recipes need the 3x3 grid of a crafting table: right-click it and the view glides over the table. Lay the items onto its grid.")
    }));
    v.push(Bullet(s(
        "At the table, click anywhere off the slots with an empty hand: it crafts a whole stack into the middle of the table.",
        "Az asztalnál üres kézzel kattints bárhová a mezőkön kívül: egy egész köteget elkészít az asztal közepére.",
    )));
    v.push(Bullet(s(
        "A recipe can sit anywhere on the grid, and works mirrored too.",
        "A recept bárhol lehet a rácson, és tükrözve is működik.",
    )));
    v.push(Bullet(s(
        "Where a cell in this book keeps changing, any of those items will do (any log, coal or charcoal).",
        "Ahol ebben a könyvben egy mező váltakozik, ott bármelyik tárgy jó (bármilyen rönk, szén vagy faszén).",
    )));
    for id in [TORCH as ItemId, CHEST as ItemId, FURNACE as ItemId, GUIDE_BOOK] {
        recipe(&mut v, id);
    }

    // ------------------------------------------------------------ tools and ores
    v.push(Chapter(s("Tools and Ores", "Eszközök és ércek"), s("Tools", "Eszközök")));
    v.push(Text(s(
        "Tools come in six materials. The better ones dig faster, last longer and mine harder ores. A pickaxe that is too weak breaks an ore without dropping anything.",
        "Az eszközök hatféle anyagból készülhetnek. A jobbak gyorsabbak, tovább bírják, és keményebb érceket is kibányásznak. A túl gyenge csákány úgy töri szét az ércet, hogy semmi sem esik ki belőle.",
    )));
    v.push(Picture(Pic::Tiers));
    let ores = |level: u8| -> String {
        let id = match level {
            0 => COAL_ORE,
            1 => COPPER_ORE,
            2 => IRON_ORE,
            3 => DIAMOND_ORE,
            _ => OBSIDIAN,
        };
        let mut t = name(id as ItemId);
        if level == 3 {
            t = format!("{} {} {}", name(GOLD_ORE as ItemId), s("and", "és"), t);
        }
        t
    };
    for tier in TIER_ORDER {
        let id = tool_id(ToolKind::Pickaxe, tier);
        let line = if hu {
            format!(
                "{}: {}x gyors, {} használat. Legfeljebb: {}.",
                name(id),
                n(tier.speed()),
                tier.durability(),
                ores(tier.level())
            )
        } else {
            format!(
                "{}: speed {}, {} uses. Mines up to {}.",
                name(id),
                n(tier.speed()),
                tier.durability(),
                ores(tier.level())
            )
        };
        v.push(Row(id, line));
    }
    v.push(Head(s("Tool shapes", "Eszközformák")));
    v.push(Text(s(
        "The same shapes with any material: planks, cobblestone, copper, iron, gold or diamond. Here with copper, smelted from copper ore.",
        "Ugyanezek a formák bármilyen anyaggal: deszka, zúzottkő, réz, vas, arany vagy gyémánt. Itt rézzel, amit rézércből olvasztasz.",
    )));
    for kind in [ToolKind::Pickaxe, ToolKind::Axe, ToolKind::Shovel, ToolKind::Sword] {
        recipe(&mut v, tool_id(kind, Tier::Copper));
    }

    // ------------------------------------------------------------ furnaces
    v.push(Chapter(s("Furnaces", "Kemencék és kohók"), s("Furnaces", "Kemencék")));
    v.push(Text(s(
        "Furnaces have no window: you use them by clicking the block itself. There are three kinds, and the better ones smelt more things, faster.",
        "A kemencéknek nincs külön ablakuk: magán a blokkon használod őket. Háromféle van; a jobbak többféle dolgot és gyorsabban olvasztanak.",
    )));
    v.push(Picture(Pic::Front));
    v.push(Bullet(s(
        "Right-click the upper half of the front: what you hold goes into its mouth to be smelted.",
        "Jobb klikk az előlap felső felére: ami a kezedben van, a szájába kerül olvadni.",
    )));
    v.push(Bullet(s(
        "Right-click the lower half: the fuel you hold goes into the firebox.",
        "Jobb klikk az alsó felére: a kezedben lévő üzemanyag a tűztérbe kerül.",
    )));
    v.push(Bullet(s(
        "Left-click takes out: above, what is smelted first, then what still waits; below, the fuel. The furnace is not mined while you do it.",
        "Bal klikk kivesz: fent előbb a kész terméket, aztán ami még vár; lent az üzemanyagot. Közben nem bányászod ki a kemencét.",
    )));
    v.push(Bullet(s(
        "What is smelted stays inside until you take it. Fuel only burns while there is something to heat.",
        "A kész termék bent marad, amíg ki nem veszed. Üzemanyagot csak akkor éget, ha van mit melegítenie.",
    )));
    v.push(Bullet(s(
        "A furnace only takes what it can smelt: a plain furnace will not take iron ore.",
        "A kemence csak azt veszi be, amit ki is tud olvasztani: egy sima kemence nem fogad be vasércet.",
    )));
    v.push(Break);
    for tier in 1..=3u8 {
        let base = furnace_of_tier(tier);
        v.push(Head(name(base as ItemId)));
        v.push(Picture(Pic::Furnace(base)));
        let secs = n(smelt_time(tier));
        v.push(Text(match (tier, hu) {
            (1, false) => format!("One block. Smelts food, clay, cobblestone, logs and copper ore, {secs} s each."),
            (1, true) => format!("Egy blokk. Ételt, agyagot, zúzottkövet, rönköt és rézércet olvaszt, darabját {secs} mp alatt."),
            (2, false) => format!("A furnace with a brick chimney on top, so it needs a free block above it. It smelts iron ore and sand too, {secs} s each. Smoke rises from the chimney while it burns."),
            (2, true) => format!("Kemence téglakéménnyel a tetején, ezért fölötte egy szabad blokk kell. Vasércet és homokot is olvaszt, darabját {secs} mp alatt. Égés közben füstöl a kéménye."),
            (_, false) => format!("Two blocks wide and two tall: the furnace, a panel to its right and a hood over both, so it needs that much room. It smelts everything, gold and diamond ore too, {secs} s each. Its hood glows while it burns."),
            (_, true) => format!("Két blokk széles és két blokk magas: a kemence, jobbra mellette egy panel, fölöttük elszívó, ezért ennyi hely kell neki. Mindent kiolvaszt, az arany- és gyémántércet is, darabját {secs} mp alatt. Égés közben izzik az elszívója."),
        }));
        if tier > 1 {
            v.push(Bullet(s(
                "Breaking any of its blocks takes down the whole furnace; what was in it drops.",
                "Ha bármelyik blokkját kiütöd, az egész kemence lebomlik, és kiesik belőle, ami benne volt.",
            )));
        }
        recipe(&mut v, base as ItemId);
    }
    v.push(Head(s("What smelts where", "Mi hol olvad")));
    v.push(Text(s(
        "The first furnace that can smelt a thing, and how long it takes there:",
        "Az első kemence, amelyik kiolvasztja, és mennyi ideig tart benne:",
    )));
    let mut smeltable: Vec<ItemId> = all_items()
        .into_iter()
        .filter(|&id| smelt(id).is_some())
        .collect();
    smeltable.sort_by_key(|&id| smelt_tier(id));
    for id in smeltable {
        v.push(Smelt(id));
    }
    v.push(Head(s("Fuel", "Üzemanyag")));
    v.push(Text(s(
        "How long one piece burns, and how many items it smelts in the furnace, blast furnace and advanced furnace. The better furnaces get more out of the same fuel.",
        "Meddig ég egy darab, és hány tárgyat olvaszt ki a kemencében, a kohóban és a fejlett kohóban. A jobb kemence ugyanabból az üzemanyagból többet hoz ki.",
    )));
    let fuels: [(ItemId, String); 6] = [
        (COAL, format!("{}, {}", name(COAL), name(CHARCOAL))),
        (COAL_BLOCK as ItemId, name(COAL_BLOCK as ItemId)),
        (
            LAVA_BUCKET,
            s("Lava Bucket (the empty bucket stays in the firebox)", "Lávás vödör (az üres vödör a tűztérben marad)"),
        ),
        (OAK_LOG as ItemId, s("Logs, planks, chests, tables, stairs", "Rönk, deszka, láda, asztal, lépcső")),
        (tool_id(ToolKind::Pickaxe, Tier::Wood), s("Wooden tools, doors", "Fa eszközök, ajtó")),
        (STICK, s("Sticks, saplings, wool", "Bot, csemete, gyapjú")),
    ];
    for (id, what) in fuels {
        let t = fuel_time(id).unwrap_or(0.0);
        let items: Vec<String> = (1..=3)
            .map(|tier| n((t / smelt_time(tier)).floor()))
            .collect();
        v.push(Row(
            id,
            format!(
                "{what}: {} {}, {} {}",
                n(t),
                s("s", "mp"),
                items.join(" / "),
                s("items", "tárgy")
            ),
        ));
    }

    // ------------------------------------------------------------ grilling and water
    v.push(Chapter(s("Grilling and Water", "Grillezés és ivóvíz"), s("Grill", "Grillezés")));
    v.push(Text(s(
        "Meat does not go into the mouth: it is grilled on top of a burning furnace, one piece on each corner of the top.",
        "A hús nem a kemence szájába kerül: az égő kemence tetején grillezed, a tető minden sarkára egy darab fér.",
    )));
    v.push(Picture(Pic::Grill));
    v.push(Bullet(s(
        "Right-click a corner with raw meat to lay it there. Right-click it again to turn it over.",
        "Jobb klikk egy sarokra nyers hússal: odateszed. Újabb jobb klikk: megfordítod.",
    )));
    v.push(Bullet(if hu {
        format!(
            "Csak a tűz felé néző oldal sül: {} mp után kész, {} mp után megég. A fordítás {} mp.",
            n(GRILL_TIME),
            n(BURN_TIME),
            n(FLIP_TIME)
        )
    } else {
        format!(
            "Only the side facing the fire cooks: it is done after {} s and burns after {} s. Turning takes {} s.",
            n(GRILL_TIME),
            n(BURN_TIME),
            n(FLIP_TIME)
        )
    }));
    v.push(Bullet(s(
        "Left-click takes the piece off. Its name tells how its two sides came out.",
        "Bal klikk leveszi. A neve megmondja, milyen lett a két oldala.",
    )));
    v.push(Bullet(s(
        "Watch the smoke: light steam while it cooks, more when the side is done, thick and dark when it burns.",
        "Figyeld a füstöt: sütés közben halvány gőz, ha kész az oldala, több füst, ha ég, sűrű sötét füst.",
    )));
    v.push(Head(s("What it does to you", "Mit tesz veled")));
    for id in meat(PORKCHOP).unwrap_or_default() {
        let Some(c) = consumable(id) else { continue };
        let sick = match c.sick {
            Some(k) => format!(
                "{}% {}",
                (k.chance * 100.0).round(),
                s("chance of getting sick", "eséllyel rosszul leszel")
            ),
            None => s("safe", "biztonságos"),
        };
        v.push(Row(
            id,
            format!("{}: {} {}, {sick}", name(id), n(c.food), s("food", "étel")),
        ));
    }
    v.push(Text(s(
        "Mutton works the same way. A burnt side spoils the taste, so you get hungry again sooner.",
        "A birkahús ugyanígy működik. Egy szenes oldal elrontja az ízét, ezért hamarabb megéhezel.",
    )));
    v.push(Head(s("Drinking water", "Ivóvíz")));
    v.push(Text(s(
        "Right-click water with a glass bottle to fill it. Lake water quenches thirst but can make you sick. Put it in a furnace's mouth to boil it: boiled water is safe.",
        "Üvegpalackkal kattints jobb gombbal a vízre, és megtelik. A tóvíz oltja a szomjat, de megbetegíthet. Tedd a kemence szájába, hogy felforrjon: a forralt víz biztonságos.",
    )));
    recipe(&mut v, GLASS_BOTTLE);
    v.push(Smelt(WATER_BOTTLE));
    for id in [WATER_BOTTLE, PURIFIED_WATER] {
        if let Some(c) = consumable(id) {
            let sick = match c.sick {
                Some(k) => format!(
                    "{}% {}",
                    (k.chance * 100.0).round(),
                    s("chance of getting sick", "eséllyel rosszul leszel")
                ),
                None => s("safe", "biztonságos"),
            };
            v.push(Row(
                id,
                format!("{}: {} {}, {sick}", name(id), n(c.thirst), s("thirst", "szomjúság")),
            ));
        }
    }

    // ------------------------------------------------------------ the gun station
    v.push(Chapter(s("The Gun Station", "A fegyverasztal"), s("Station", "Asztal")));
    v.push(Picture(Pic::Station));
    v.push(Text(s(
        "The gun station is two blocks wide, its drawer at the front (keep the space in front of it free). Right-click it and the view glides over its table; move the mouse to look along it. Everything happens right there on the table.",
        "A fegyverasztal két blokk széles, elöl van a fiókja (előtte hagyd szabadon a helyet). Kattints rá jobb gombbal, és a nézet az asztal fölé úszik; az egérrel végignézhetsz rajta. Minden közvetlenül az asztalon történik.",
    )));
    recipe(&mut v, GUN_STATION as ItemId);
    v.push(Head(name(RIFLE_BENCH as ItemId)));
    v.push(Text(s(
        "The long guns (the AK-47) are put together, taken apart and cleaned at the rifle station: three blocks wide, it works just as the gun station does. The small station is for the handguns only.",
        "A hosszú fegyvereket (az AK-47-et) a puskaasztalon rakod össze, szeded szét és tisztítod: három blokk széles, és ugyanúgy működik, mint a fegyverasztal. A kis asztal csak a kézifegyvereké.",
    )));
    recipe(&mut v, RIFLE_BENCH as ItemId);
    v.push(Head(s("On the table", "Az asztalon")));
    v.push(Text(s(
        "Lay anything from your inventory on the table where you click: a gun, its parts, attachments. Click it again to pick it up, or drag it somewhere else.",
        "Bármit rárakhatsz az asztalra a tárgylistádból oda, ahová kattintasz: fegyvert, alkatrészeket, kiegészítőket. Újra rákattintva felveszed, vagy odébb húzhatod.",
    )));
    v.push(Head(s("Assemble and take apart", "Összerakás és szétszedés")));
    v.push(Text(s(
        "Right-click a gun on the table and it comes apart there; its magazine goes back into your inventory. Right-click one of the parts and the parts on the table (frame, barrel, recoil spring and slide) fly to the middle and go together.",
        "Jobb klikk egy fegyverre az asztalon, és ott szétszedi; a tára visszakerül a tárgylistádba. Jobb klikk valamelyik alkatrészre, és az asztalon lévő alkatrészek (váz, cső, helyretoló rugó és szán) középre repülnek és összeállnak.",
    )));
    v.push(Head(s("Clean", "Tisztítás")));
    v.push(Text(s(
        "Every shot makes a gun dirtier, and you can see it on the gun. A gun that is too dirty jams and will not fire. Take the brush out of the drawer and hold the left mouse button on a part to scrub it clean; a whole gun cleans too, but slowly. Right-click puts the brush back.",
        "Minden lövéstől koszosabb lesz a fegyver, és ez látszik is rajta. A túl koszos beakad, nem lő. Vedd ki a kefét a fiókból, és a bal egérgombot nyomva tartva sikáld tisztára az alkatrészeket; egészben is lehet, de lassan. Jobb klikkel visszateszed a kefét.",
    )));
    v.push(Head(s("Attachments", "Kiegészítők")));
    v.push(Text(s(
        "Drag an attachment onto a gun lying on the table and it goes on. Click one on the gun to take it off: it is laid beside the gun.",
        "Húzz egy kiegészítőt az asztalon fekvő fegyverre, és felmegy rá. A fegyveren lévőre kattintva leveszed: a fegyver mellé kerül.",
    )));
    v.push(Head(s("Loading magazines", "Tárak töltése")));
    v.push(Text(s(
        "Move the mouse down to the drawer: on its right are three boxes of rounds, 128 in each, the count written on them. Drop rounds from your inventory into a box, click a box to take a round out (right-click: a magazine's worth), and drag rounds onto a magazine lying on the table: they are pushed in one by one. The witness holes on its side show how many are in it.",
        "Vidd le az egeret a fiókhoz: a jobb oldalán három doboz töltény van, mindegyikben 128 fér el, a szám rá van írva. A tárgylistádból dobj töltényt a dobozba, kattints egy dobozra, hogy kivegyél egy töltényt (jobb klikk: egy tárnyit), és húzd a töltényeket az asztalon fekvő tárra: egyenként belenyomja őket. Az oldalán lévő lyukakban látszik, mennyi van benne.",
    )));

    // ------------------------------------------------------------ the guns
    v.push(Chapter(s("Guns", "Fegyverek"), s("Guns", "Fegyverek")));
    v.push(Text(if hu {
        format!("Bal egérgomb: lövés. Jobb egérgomb: célzás, pontosabb és ránagyít. {reload}: tárcsere: a régi tár kiesik, a legtelibb töltött tárad bekerül. Ha nincs másik, csak kiesik a régi. A tárat a fegyverasztalon töltöd meg: húzz töltényt az asztalon fekvő tárra.")
    } else {
        format!("Left mouse button: shoot. Right mouse button: aim down the sights, steadier and zoomed in. {reload}: change magazines: the old one drops out and your fullest loaded one goes in. Without another, the old one only drops out. Magazines are loaded at the gun station: drag rounds onto a magazine lying on its table.")
    }));
    v.push(Bullet(s(
        "Bullets really fly: they take time to get there and drop with distance. Aim a little higher at far targets.",
        "A lövedék valóban repül: idő kell, amíg odaér, és a távolsággal esik. Távoli célnál célozz kicsit feljebb.",
    )));
    v.push(Bullet(s(
        "From the hip the shots spread; aimed they go where the sights point.",
        "Csípőből szórnak a lövések, célozva oda mennek, ahová az irányzék mutat.",
    )));
    let all = GUN_KINDS.map(|k| k.stats());
    let max = |f: &dyn Fn(&Stats) -> f32| all.iter().map(|s| f(s)).fold(0.0f32, f32::max);
    let per_shot = |st: &Stats| st.damage * st.pellets as f32;
    let rate = |st: &Stats| 1.0 / st.fire_delay;
    for kind in GUN_KINDS {
        let st = kind.stats();
        let gun = kind.item();
        v.push(Break);
        v.push(Head(name(gun)));
        v.push(Picture(Pic::Gun(kind)));
        v.push(Text(match (kind, hu) {
            (GunKind::Pistol, false) => "Light and quick, a good first gun. Its five parts are crafted from iron.".into(),
            (GunKind::Pistol, true) => "Könnyű és gyors, jó első fegyver. Az öt alkatrészét vasból barkácsolod.".into(),
            (GunKind::Revolver, false) => "Six heavy rounds in a swing-out cylinder: slower, kicks harder, hits much harder. Each pull turns the next chamber under the hammer: a fired case stays in its chamber, and an empty one only clicks. The reload key swings the cylinder out, throws out the cases (the live rounds fall to the ground too) and loads it a round at a time from the bullets you carry (shoot to stop), or all at once from a loaded speedloader.".into(),
            (GunKind::Ak, false) => "A big-calibre automatic rifle: it fires as long as you hold the button, ten rounds a second, from a curved 30-round magazine. Its 7.62 rounds hit hard and fly far and flat, but every shot kicks the barrel up: aim, and fire short bursts. It is changed like the pistol's magazine, with the reload key, and the bolt carrier's handle is pulled when the chamber is empty. Its magazines are loaded at the gun station.".into(),
            (GunKind::Ak, true) => "Nagy kaliberű gépkarabély: addig lő, amíg nyomva tartod a gombot, másodpercenként tízet, egy 30 töltényes ívelt tárból. A 7,62-es töltény nagyot üt, messzire és laposan repül, de minden lövés feljebb rúgja a csövet: célozz, és lőj rövid sorozatokat. A tárat a pisztolyéhoz hasonlóan az újratöltés gombbal cseréled, üres csőnél a zárkeret fogantyúját is meghúzza. A tárait a fegyverasztalon töltöd meg.".into(),
            (GunKind::Revolver, true) => "Hat nehéz töltény a kibillenő forgótárban: lassabb, jobban rúg, de sokkal nagyobbat üt. Minden ravaszhúzás a következő kamrát fordítja a kakas alá: a kilőtt hüvely a kamrában marad, az üres kamrán csak kattan. Az újratöltés gomb kibillenti a forgótárat, kiveti a hüvelyeket (az éles töltények is a földre esnek), és egyesével tölti a nálad lévő töltényekből (lövéssel megállíthatod), vagy egyszerre egy megtöltött gyorstöltőből.".into(),
        }));
        let damage = if st.pellets > 1 {
            format!("{}\u{d7}{}", n(st.damage), st.pellets)
        } else {
            n(st.damage)
        };
        v.push(Stat(s("Damage", "Sebzés"), per_shot(st) / max(&per_shot), damage));
        v.push(Stat(
            s("Fire rate", "Tűzgyorsaság"),
            rate(st) / max(&rate),
            format!("{}/{}", n((rate(st) * 10.0).round() / 10.0), s("s", "mp")),
        ));
        v.push(Stat(
            s("Magazine", "Tár"),
            st.magazine as f32 / max(&|s: &Stats| s.magazine as f32),
            match kind.magazine().and_then(|m| m.extended) {
                Some((_, extended)) => format!("{} ({})", st.magazine, extended),
                None => st.magazine.to_string(),
            },
        ));
        v.push(Stat(
            s("Range", "Lőtáv"),
            st.range / max(&|s: &Stats| s.range),
            n(st.range),
        ));
        v.push(Stat(
            s("Accuracy", "Pontosság"),
            1.0 - st.spread_hip / max(&|s: &Stats| s.spread_hip) * 0.85,
            format!("{}\u{b0}", n(st.spread_hip)),
        ));
        v.push(Stat(
            s("Reload", "Töltés"),
            st.reload / max(&|s: &Stats| s.reload),
            format!("{} {}", n(st.reload), s("s", "mp")),
        ));
        v.push(Stat(
            s("Clean for", "Tiszta"),
            st.dirt_max as f32 / max(&|s: &Stats| s.dirt_max as f32),
            format!("{} {}", st.dirt_max, s("shots", "lövés")),
        ));
        v.push(Text(s(
            "Magazine in brackets: with the extended magazine. Accuracy: the spread from the hip.",
            "A tárnál zárójelben: bővített tárral. Pontosság: a szórás csípőből.",
        )));
        v.push(Row(
            kind.ammo(),
            format!("{}: {}", s("Fires", "Lőszere"), name(kind.ammo())),
        ));
        v.push(Head(s("Parts", "Alkatrészek")));
        v.push(Text(match kind {
            GunKind::Pistol => s(
                "Craft the frame, barrel, recoil spring and slide, lay them on the gun station's table and right-click one of them. The gun comes out without a magazine: craft that too, and put it in with the reload key.",
                "Barkácsold meg a vázat, a csövet, a helyretoló rugót és a szánt, tedd őket a fegyverasztalra, és kattints jobb gombbal valamelyikre. A fegyver tár nélkül készül el: azt is barkácsold meg, és az újratöltés gombbal tedd bele.",
            ),
            GunKind::Ak => s(
                "Craft the receiver (the barrel, sights, grip and stock on it), the gas tube, the bolt carrier and the dust cover, lay them on the gun station's table and right-click one of them. It comes out without a magazine: craft one, load it with rounds at the table, and put it in with the reload key.",
                "Barkácsold meg a tokot (rajta a cső, az irányzék, a markolat és a tus), a gázcsövet, a zárkeretet és a tokfedelet, tedd őket a fegyverasztalra, és kattints jobb gombbal valamelyikre. Tár nélkül készül el: barkácsolj egyet, töltsd meg az asztalon, és az újratöltés gombbal tedd bele.",
            ),
            GunKind::Revolver => s(
                "Craft the frame, barrel, mainspring, cylinder and hammer, lay them on the gun station's table and right-click one of them. It comes out empty: load it with the reload key. A right click on it there takes it apart again.",
                "Barkácsold meg a vázat, a csövet, a kakasrugót, a forgótárat és a kakast, tedd őket a fegyverasztalra, és kattints jobb gombbal valamelyikre. Üresen készül el: az újratöltés gombbal töltöd meg. Ott jobb kattintással újra szétszedheted.",
            ),
        }));
        for &item in kind.parts() {
            recipe(&mut v, item);
        }
    }

    // ------------------------------------------------------------ ammunition and attachments
    v.push(Chapter(s("Ammo and Attachments", "Lőszer és kiegészítők"), s("Ammo", "Lőszer")));
    v.push(Text(s(
        "The pistol fires bullets made from iron and coal (or charcoal); the revolver longer magnum rounds, with a copper jacket; the AK big 7.62 rifle rounds, with a steel core. One goes per shot, and each fits only its own gun. Magazines are loaded at the gun station: drag rounds onto one lying on the table.",
        "A pisztoly vasból és szénből (vagy faszénből) készült töltényt lő, a revolver hosszabb, rézköpenyes magnum töltényt, az AK nagy, acélmagvas 7,62-es puskatöltényt. Lövésenként egy fogy, és mindegyik csak a saját fegyverébe jó. A tárakat a fegyverasztalon töltöd meg: húzz töltényeket az asztalon fekvő tárra.",
    )));
    let mut ammo: Vec<ItemId> = GUN_KINDS.iter().map(|k| k.ammo()).collect();
    ammo.dedup();
    for item in ammo {
        recipe(&mut v, item);
    }
    v.push(Head(name(SPEEDLOADER)));
    v.push(Text(s(
        "Holds six magnum rounds for the revolver, to load it all at once. Load it at the gun station: drag rounds onto it lying on the table.",
        "Hat magnum töltényt tart a revolverhez, hogy egyszerre töltsd meg. A fegyverasztalon töltöd meg: húzz rá töltényeket, amíg az asztalon fekszik.",
    )));
    recipe(&mut v, SPEEDLOADER);
    v.push(Head(s("Attachments", "Kiegészítők")));
    v.push(Text(s(
        "Fitted on the pistol at the gun station: drag one onto the gun lying on its table.",
        "A fegyverasztalon szerelheted fel őket a pisztolyra: húzd rá az asztalon fekvő fegyverre.",
    )));
    v.push(Picture(Pic::Items(ATTACHMENTS.iter().map(|a| a.1).collect())));
    for (_, item) in ATTACHMENTS {
        v.push(Head(name(item)));
        v.push(Text(match (item, hu) {
            (SCOPE, false) => "Zooms in far when you aim.".into(),
            (SCOPE, true) => "Célzáskor nagyon ránagyít.".into(),
            (SILENCER, false) => "No muzzle flash gives you away.".into(),
            (SILENCER, true) => "Nem árul el a torkolattűz.".into(),
            (EXTENDED_MAGAZINE, _) => {
                let list: Vec<String> = GUN_KINDS
                    .iter()
                    .filter_map(|k| Some(format!("{} {}", name(k.item()), k.magazine()?.extended?.1)))
                    .collect();
                format!(
                    "{}: {}.",
                    s("More rounds before you reload", "Több töltény újratöltés előtt"),
                    list.join(", ")
                )
            }
            (FLASHLIGHT, false) => "A bright light on the rail (instead of the laser sight): it lights up where the gun points. Switched on and off with its key (L). Only found in creative.".into(),
            (FLASHLIGHT, true) => "Erős lámpa a sínen (a lézer helyett): megvilágítja, amerre a fegyver néz. A gombjával (L) kapcsolod ki-be. Csak kreatívban van.".into(),
            (_, false) => "Much steadier from the hip, and a red dot shows where you point.".into(),
            (_, true) => "Csípőből sokkal pontosabb, és egy piros pont mutatja, hová célzol.".into(),
        }));
        if item != FLASHLIGHT {
            recipe(&mut v, item);
        }
    }
    v.push(Head(name(TARGET_DUMMY)));
    v.push(Text(s(
        "Set it up with a right click, then shoot or hit it: above its head it shows all the damage it has taken and the last hit. After a few quiet seconds it starts counting again. Sneak and hit it to take it down.",
        "Jobb klikkel állítod fel, aztán lőj vagy üss bele: a feje fölött mutatja, mennyi sebzést kapott összesen, és mennyit az utolsó találat. Néhány nyugodt másodperc után újrakezdi a számolást. Guggolva ütve leszeded.",
    )));
    recipe(&mut v, TARGET_DUMMY);

    // ------------------------------------------------------------ everything else
    v.push(Chapter(s("More Recipes", "További receptek"), s("Recipes", "Receptek")));
    v.push(Text(s(
        "Everything else there is to craft. Storage blocks turn back into nine of what they are made of.",
        "Minden más, amit barkácsolni lehet. A tárolóblokkokból visszakapod a kilenc darabot, amiből készültek.",
    )));
    for id in recipe_results() {
        if !shown.contains(&id) && tool_of(id).is_none() {
            v.push(Recipe(id));
        }
    }
    v
}


fn pieces(font: &Font, m: &Metrics, el: El) -> Vec<Piece> {
    let u = m.u;
    match el {
        El::Chapter(t, short) => vec![Piece::Break, Piece::Title(font.wrap(&t, m.w, m.tfs), short)],
        El::Break => vec![Piece::Break],
        El::Head(t) => vec![Piece::Head(t)],
        El::Text(t) => {
            let mut v: Vec<Piece> = font.wrap(&t, m.w, m.fs)
                .into_iter()
                .map(|text| Piece::Line { text, indent: false, dash: false })
                .collect();
            v.push(Piece::Gap(4.0 * u));
            v
        }
        El::Bullet(t) => {
            let mut v: Vec<Piece> = font.wrap(&t, m.w - INDENT * u, m.fs)
                .into_iter()
                .enumerate()
                .map(|(i, text)| Piece::Line { text, indent: true, dash: i == 0 })
                .collect();
            v.push(Piece::Gap(3.0 * u));
            v
        }
        El::Recipe(id) => vec![Piece::Recipe(id)],
        El::Smelt(id) => vec![Piece::Smelt(id)],
        El::Row(id, t) => vec![Piece::Row(id, font.wrap(&t, m.w - 22.0 * u, m.fs))],
        El::Picture(p) => vec![Piece::Pic(p)],
        El::Stat(a, k, b) => vec![Piece::Stat(a, k, b)],
    }
}

/// The book laid out in one language.
pub struct Layout {
    /// Each page's pieces with their heights on it.
    pub pages: Vec<Vec<(f32, Piece)>>,
    /// Chapter titles and the page each starts on.
    pub chapters: Vec<(String, usize)>,
    /// The chapters' short names, for their tabs.
    pub shorts: Vec<String>,
}

impl Layout {
    /// Something on the page moves.
    pub fn animated(&self, page: usize) -> bool {
        self.pages
            .get(page)
            .is_some_and(|p| p.iter().any(|(_, piece)| piece.animated()))
    }
}

/// Lays the book out on pages: the cover, the contents, then the chapters. `keys` are the
/// names of the inventory and reload keys.
pub fn layout(font: &Font, hu: bool, keys: &(String, String)) -> Layout {
    let m = metrics();
    let flat: Vec<Piece> = content(hu, keys)
        .into_iter()
        .flat_map(|e| pieces(font, &m, e))
        .collect();
    let mut pages: Vec<Vec<(f32, Piece)>> =
        vec![vec![(0.0, Piece::Cover)], vec![(0.0, Piece::Contents)]];
    let mut chapters = Vec::new();
    let mut shorts = Vec::new();
    let mut cur: Vec<(f32, Piece)> = Vec::new();
    let mut y = 0.0;
    for (i, p) in flat.iter().enumerate() {
        let h = height(&m, p);
        let mut need = h;
        match p {
            Piece::Break => {
                if !cur.is_empty() {
                    pages.push(std::mem::take(&mut cur));
                    y = 0.0;
                }
                continue;
            }
            Piece::Gap(_) if cur.is_empty() => continue,
            // A heading stays with what follows it.
            Piece::Head(_) | Piece::Title(..) => {
                if let Some(next) = flat.get(i + 1) {
                    need += height(&m, next);
                }
            }
            _ => {}
        }
        if y + need > m.h && !cur.is_empty() {
            pages.push(std::mem::take(&mut cur));
            y = 0.0;
            if matches!(p, Piece::Gap(_)) {
                continue;
            }
        }
        if let Piece::Title(t, short) = p {
            chapters.push((t.join(" "), pages.len()));
            shorts.push(short.clone());
        }
        cur.push((y, p.clone()));
        y += h;
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    Layout {
        pages,
        chapters,
        shorts,
    }
}

/// Where the contents' entries are on its page: the first one's top, and each one's height
/// (pixels of the page's texture).
fn contents_rows(m: &Metrics) -> (f32, f32) {
    (MARGIN_TOP * m.u + m.tlh + 12.0 * m.u, m.lh + 5.0 * m.u)
}

/// The contents entry at height `y` of the contents page.
pub fn contents_entry(lay: &Layout, y: f32) -> Option<usize> {
    let m = metrics();
    let (top, row) = contents_rows(&m);
    let k = (y - top + 2.0 * m.u) / row;
    (k >= 0.0 && (k as usize) < lay.chapters.len()).then_some(k as usize)
}

/// How a page is drawn.
pub struct Look<'a> {
    pub hu: bool,
    /// Seconds, for the animations.
    pub time: f32,
    pub theme: &'a Theme,
    /// The contents entry under the crosshair.
    pub hover: Option<usize>,
}

/// Page `n` of the book, drawn onto a new `SHEET_W` x `SHEET_H` canvas.
pub fn draw_page<'a>(font: &'a Font, texture: &'a [u8], lay: &Layout, n: usize, look: &Look) -> Canvas<'a> {
    let m = metrics();
    let th = look.theme;
    let mut cv = Canvas::new(SHEET_W, SHEET_H, th.paper, font, texture);
    let (pw, ph) = PAGE_PX;
    // Darker toward the outer edge and at the spine.
    let right = n % 2 == 1;
    let e = 12.0 * m.u;
    for i in 0..12 {
        let f = 1.0 - i as f32 / 12.0;
        let x = if right { pw - (i + 1) as f32 * e / 12.0 } else { i as f32 * e / 12.0 };
        cv.fill(x, 0.0, e / 12.0, ph, with_a(th.edge, 0.16 * f));
        let s = if right { i as f32 * 1.5 } else { pw - (i + 1) as f32 * 1.5 };
        cv.fill(s, 0.0, 1.5, ph, with_a(th.edge, 0.2 * f));
    }
    let mut d = Draw {
        cv: &mut cv,
        m: &m,
        look,
        lay,
    };
    if let Some(page) = lay.pages.get(n) {
        let (ix, iy) = (MARGIN_X * m.u, MARGIN_TOP * m.u);
        for (py, p) in page {
            d.piece(p, ix, iy + py);
        }
    }
    if n > 0 {
        let num = format!("{n}");
        cv.text_centered(&num, pw * 0.5, ph - 13.0 * m.u, m.fs, th.soft);
    }
    cv
}

/// Size of the tabs' texture: `model::book::TAB_LAYERS` layers side by side.
pub const TABS_W: usize = 512;
pub const TABS_H: usize = 128;

/// The chapter tabs, side by side (see `model::book::TABS_PER_HALF`): each a colored tab
/// with its number over its short name; the chapter open now stands out taller with a gold
/// edge, the one aimed at is lighter.
pub fn draw_tabs<'a>(font: &'a Font, texture: &'a [u8], lay: &Layout, open: Option<usize>, hover: Option<usize>, theme: &Theme) -> Canvas<'a> {
    let mut cv = Canvas::new(TABS_W, TABS_H, [0.0; 4], font, texture);
    let colors = [
        rgba(166, 58, 44, 255),
        rgba(190, 120, 40, 255),
        rgba(150, 70, 36, 255),
        rgba(120, 140, 50, 255),
        rgba(60, 120, 120, 255),
        rgba(70, 80, 140, 255),
        rgba(120, 70, 130, 255),
        rgba(110, 96, 80, 255),
    ];
    let bottom = crate::model::book::TAB_PX;
    let w = TABS_W as f32 / 8.0;
    for (i, short) in lay.shorts.iter().enumerate().take(8) {
        let x = i as f32 * w;
        let (open, lit) = (open == Some(i), hover == Some(i));
        let top = if open { 1.0 } else { 7.0 };
        let base = colors[i % colors.len()];
        let c = if lit || open {
            std::array::from_fn(|k| if k == 3 { 1.0 } else { (base[k] * 1.3).min(1.0) })
        } else {
            base
        };
        if open {
            cv.fill(x + 2.0, top - 1.0, w - 4.0, bottom - top + 1.0, theme.gold);
        }
        cv.fill(x + 3.0, top, w - 6.0, bottom - top, c);
        // A lighter band along the top, like a fold.
        cv.fill(x + 3.0, top, w - 6.0, 2.0, with_a([1.0; 4], 0.25));
        let white = rgba(250, 244, 230, 255);
        let n = format!("{}", i + 1);
        cv.text(&n, x + w * 0.5 - font.text_width(&n, 2.0) * 0.5, top + 3.0, 2.0, white, true);
        let tw = font.text_width(short, 1.0);
        cv.text(short, x + (w - tw) * 0.5, top + 23.0, 1.0, white, true);
    }
    cv
}

fn with_a(c: Color, a: f32) -> Color {
    [c[0], c[1], c[2], a]
}

/// Loops 0..1 every `period` seconds.
fn phase(time: f32, period: f32) -> f32 {
    (time / period).fract()
}

struct Draw<'c, 'a> {
    cv: &'c mut Canvas<'a>,
    m: &'c Metrics,
    look: &'c Look<'c>,
    lay: &'c Layout,
}

impl Draw<'_, '_> {
    fn icon(&mut self, id: ItemId, count: u8, x: f32, y: f32, size: f32) {
        self.cv.stack(x, y, size, &Stack::new(id, count.max(1)));
    }

    fn slot(&mut self, x: f32, y: f32, size: f32) {
        let (u, th) = (self.m.u, self.look.theme);
        self.cv.fill(x, y, size, size, th.slot_edge);
        self.cv.fill(x + u, y + u, size - 2.0 * u, size - 2.0 * u, th.slot_fill);
    }

    /// A small arrow pointing right, filled from the left by `k` (0..1).
    fn arrow(&mut self, x: f32, y: f32, u: f32, k: f32) {
        let th = self.look.theme;
        let v = Vec2::new;
        let shaft = |cv: &mut Canvas, c: Color, w: f32| cv.fill(x, y + 2.0 * u, w, 2.0 * u, c);
        let head = [v(x + 5.0 * u, y - 0.5 * u), v(x + 9.0 * u, y + 3.0 * u), v(x + 5.0 * u, y + 6.5 * u)];
        shaft(self.cv, th.bar_bg, 5.0 * u);
        self.cv.poly(&head, th.bar_bg);
        let fill = 9.0 * u * k.clamp(0.0, 1.0);
        if fill > 0.0 {
            shaft(self.cv, th.soft, fill.min(5.0 * u));
            if fill > 5.0 * u {
                let t = (fill - 5.0 * u) / (4.0 * u);
                let top = y - 0.5 * u + 3.5 * u * t;
                let bot = y + 6.5 * u - 3.5 * u * t;
                self.cv.poly(
                    &[v(x + 5.0 * u, y - 0.5 * u), v(x + 5.0 * u + 4.0 * u * t, top), v(x + 5.0 * u + 4.0 * u * t, bot), v(x + 5.0 * u, y + 6.5 * u)],
                    th.soft,
                );
            }
        }
    }

    fn smoke(&mut self, x: f32, y: f32, t: f32, gray: u8, rise: f32) {
        let u = self.m.u;
        let r = (1.5 + 3.0 * t) * u;
        let (px, py) = (x + (t * 7.0).sin() * 2.0 * u, y - t * rise * u);
        let c = rgba(gray, gray, gray, 255);
        self.cv.circle(px, py, r, with_a(c, 0.6 * (1.0 - t)));
    }

    fn piece(&mut self, p: &Piece, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let hu = self.look.hu;
        let time = self.look.time;
        match p {
            Piece::Break | Piece::Gap(_) => {}
            Piece::Cover => self.cover(x, y),
            Piece::Contents => self.contents(x, y),
            Piece::Title(lines, _) => {
                let mut ty = y + 2.0 * u;
                for l in lines {
                    self.cv.text_centered(l, x + m.w * 0.5, ty, m.tfs, th.red);
                    ty += m.tlh;
                }
                // A gold rule with a diamond in the middle.
                let (cx, ry) = (x + m.w * 0.5, ty + u);
                self.cv.fill(cx - 40.0 * u, ry, 80.0 * u, 1.0, th.gold);
                let dd = 2.5 * u;
                let v = Vec2::new;
                self.cv.poly(&[v(cx, ry - dd), v(cx + dd, ry + 0.5), v(cx, ry + dd + 1.0), v(cx - dd, ry + 0.5)], th.gold);
            }
            Piece::Head(t) => {
                self.cv.text(t, x, y + 3.0 * u, m.fs, th.red, false);
            }
            Piece::Line { text, indent, dash } => {
                if *dash {
                    self.cv.fill(x + 2.0 * u, y + 3.0, 3.0 * u, 1.0, th.soft);
                }
                let ix = if *indent { INDENT * u } else { 0.0 };
                self.cv.text(text, x + ix, y, m.fs, th.ink, false);
            }
            Piece::Recipe(id) => self.recipe(*id, x, y),
            Piece::Smelt(id) => {
                let Some(out) = smelt(*id) else { return };
                let s = 16.0 * u;
                let yy = y + 2.0 * u;
                let tier = smelt_tier(*id);
                let secs = smelt_time(tier);
                // The arrow fills as fast as the furnace smelts (twice as fast, to watch).
                let k = phase(time, secs * 0.5);
                self.icon(*id, 1, x, yy, s);
                self.arrow(x + 18.5 * u, yy + 5.0 * u, u, k);
                self.icon(out, 1, x + 30.0 * u, yy, s);
                let where_ = format!(
                    "{}, {} {}",
                    name(furnace_of_tier(tier) as ItemId),
                    num(hu, secs),
                    tr(hu, "s", "mp")
                );
                let tx = x + 50.0 * u;
                self.cv.text(&name(out), tx, y + u, m.fs, th.ink, false);
                self.cv.text(&where_, tx, y + u + m.lh, m.fs, th.soft, false);
            }
            Piece::Row(id, lines) => {
                self.icon(*id, 1, x, y + u, 16.0 * u);
                let h = lines.len() as f32 * m.lh;
                let mut ty = y + ((18.0 * u - h) * 0.5).max(0.0) + u;
                for l in lines {
                    self.cv.text(l, x + 22.0 * u, ty, m.fs, th.ink, false);
                    ty += m.lh;
                }
            }
            Piece::Pic(pic) => self.picture(pic, x, y),
            Piece::Stat(label, k, value) => {
                let ty = y + u;
                self.cv.text(label, x, ty, m.fs, th.ink, false);
                let vw = self.cv.font().text_width(value, m.fs);
                self.cv.text(value, x + m.w - vw, ty, m.fs, th.soft, false);
                let bx = x + m.w * 0.4;
                let bw = m.w * 0.6 - vw - 5.0 * u;
                let bh = (3.0 * u).round().max(2.0);
                let by = (ty + 3.5 * m.fs - bh * 0.5).round();
                self.cv.fill(bx, by, bw, bh, th.bar_bg);
                self.cv.fill(bx, by, (bw * k.clamp(0.04, 1.0)).round(), bh, th.bar);
            }
        }
    }

    /// A crafting grid: the pattern (cells with several items cycle through them), an arrow
    /// filling up and what comes out, under its name.
    fn recipe(&mut self, id: ItemId, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let Some((rows, result)) = recipe_view(id) else { return };
        let title = if result.count > 1 {
            format!("{} \u{d7}{}", name(result.item), result.count)
        } else {
            name(result.item)
        };
        self.cv.text_centered(&title, x + m.w * 0.5, y + u, m.fs, th.soft);
        let cell = (18.0 * u).round();
        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
        let width = cols as f32 * cell + 18.0 * u + cell;
        let gx = (x + (m.w - width) * 0.5).round();
        let gy = (y + m.lh + 3.0 * u).round();
        let cycle = (self.look.time * 0.8) as usize;
        for (r, row) in rows.iter().enumerate() {
            for (c, items) in row.iter().enumerate() {
                let (sx, sy) = (gx + c as f32 * cell, gy + r as f32 * cell);
                self.slot(sx, sy, cell);
                if !items.is_empty() {
                    let item = items[cycle % items.len()];
                    self.icon(item, 1, sx + u, sy + u, cell - 2.0 * u);
                }
            }
        }
        // The arrow fills, then what comes out pops up in its slot.
        let k = phase(self.look.time + id as f32 * 0.37, 2.4);
        let ay = gy + rows.len() as f32 * cell * 0.5 - 3.0 * u;
        self.arrow(gx + cols as f32 * cell + 4.0 * u, ay, u, k / 0.6);
        let rx = gx + cols as f32 * cell + 18.0 * u;
        let ry = (gy + (rows.len() as f32 * cell - cell) * 0.5).round();
        self.slot(rx, ry, cell);
        // When the arrow is full the result pops (grows and settles back).
        let pop = if k > 0.6 { ((k - 0.6) / 0.15 * std::f32::consts::PI).sin().max(0.0) } else { 0.0 };
        let s = (cell - 2.0 * u) * (1.0 + 0.25 * pop);
        let o = (cell - s) * 0.5;
        self.icon(result.item, result.count, rx + o, ry + o, s);
    }

    fn cover(&mut self, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let hu = self.look.hu;
        let t = self.look.time;
        let cx = x + m.w * 0.5;
        let big = m.tfs + m.fs;
        self.cv.text_centered(&tr(hu, "Guide Book", "Kézikönyv"), cx, y + 22.0 * u, big, th.red);
        let sub = tr(hu, "Crafting \u{b7} Furnaces \u{b7} Guns", "Barkácsolás \u{b7} Kemencék \u{b7} Fegyverek");
        self.cv.text_centered(&sub, cx, y + 26.0 * u + big * 10.0, m.fs, th.soft);
        // The book floats, and sparkles twinkle around it.
        let bob = (t * 1.8).sin() * 3.0 * u;
        let iy = y + m.h * 0.55 + bob;
        self.cv.sprite(Vec2::new(cx, iy), 30.0 * u, tex::BOOK, [255; 3], 1.0);
        for i in 0..7 {
            let a = i as f32 * 2.4;
            let k = phase(t + i as f32 * 0.29, 1.6);
            let r = (34.0 + 8.0 * (a * 1.7).sin()) * u;
            let (sx, sy) = (cx + a.cos() * r, iy - bob + a.sin() * r * 0.8);
            let s = (1.0 + 2.0 * (k * std::f32::consts::PI).sin()) * u;
            let v = Vec2::new;
            self.cv.poly(&[v(sx, sy - s), v(sx + s * 0.35, sy), v(sx, sy + s), v(sx - s * 0.35, sy)], th.gold);
            self.cv.poly(&[v(sx - s, sy), v(sx, sy - s * 0.35), v(sx + s, sy), v(sx, sy + s * 0.35)], th.gold);
        }
        // Gold corners.
        for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (px, py) = (x + dx * (m.w - 12.0 * u), y + dy * (m.h - 12.0 * u));
            self.cv.fill(px, py + if dy > 0.5 { 11.0 * u } else { 0.0 }, 12.0 * u, 1.0, th.gold);
            self.cv.fill(px + if dx > 0.5 { 11.0 * u } else { 0.0 }, py, 1.0, 12.0 * u, th.gold);
        }
        let hint = tr(
            hu,
            "Look down to read. Right click: next page, left click: back.",
            "Nézz le az olvasáshoz. Jobb klikk: lapozás, bal klikk: vissza.",
        );
        let mut ty = y + m.h - 26.0 * u;
        for l in self.cv.font().wrap(&hint, m.w, m.fs) {
            self.cv.text_centered(&l, cx, ty, m.fs, th.soft);
            ty += m.lh;
        }
    }

    fn contents(&mut self, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let hu = self.look.hu;
        self.cv
            .text_centered(&tr(hu, "Contents", "Tartalom"), x + m.w * 0.5, y + 2.0 * u, m.tfs, th.red);
        let (top, row) = contents_rows(m);
        let mut ty = top;
        for (i, (title, page)) in self.lay.chapters.iter().enumerate() {
            let label = format!("{}. {}", i + 1, title);
            let num = format!("{page}");
            let hover = self.look.hover == Some(i);
            if hover {
                self.cv.fill(x - 2.0 * u, ty - 2.5 * u, m.w + 4.0 * u, row, th.hover);
            }
            let c = if hover { th.red } else { th.ink };
            let lw = self.cv.text(&label, x, ty, m.fs, c, false);
            let nw = self.cv.font().text_width(&num, m.fs);
            self.cv.text(&num, x + m.w - nw, ty, m.fs, c, false);
            // Dots between the title and the page number.
            let mut dx = x + lw + 3.0 * u;
            while dx < x + m.w - nw - 4.0 * u {
                self.cv.fill(dx, ty + 6.0, 1.0, 1.0, th.soft);
                dx += 3.0;
            }
            ty += row;
        }
        let tip = tr(
            hu,
            "While reading, press a chapter's number (1-8, also on the tabs on top) to open it. Home comes back here.",
            "Olvasás közben a fejezet számával (1-8, a fenti füleken is) odalapozol. A Home ide hoz vissza.",
        );
        ty += 6.0 * u;
        for l in self.cv.font().wrap(&tip, m.w, m.fs) {
            self.cv.text(&l, x, ty, m.fs, th.soft, false);
            ty += m.lh;
        }
    }

    fn picture(&mut self, pic: &Pic, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let hu = self.look.hu;
        let t = self.look.time;
        let cx = x + m.w * 0.5;
        let hgt = pic.height() * u;
        let v = Vec2::new;
        match pic {
            Pic::Furnace(base) => {
                // Iso cubes (as the item icons draw them), the furnace facing the reader's
                // left: (right, up) steps along its front and upward.
                let cells: Vec<(f32, f32, Block)> = match *base {
                    BLAST_FURNACE => vec![(0.0, 0.0, furnace_id(*base, 0, true)), (0.0, 1.0, CHIMNEY)],
                    ADV_FURNACE => vec![
                        (0.0, 0.0, furnace_id(*base, 0, true)),
                        (1.0, 0.0, adv_part_id(1, 0, true)),
                        (0.0, 1.0, adv_part_id(2, 0, true)),
                        (1.0, 1.0, adv_part_id(3, 0, true)),
                    ],
                    _ => vec![(0.0, 0.0, furnace_id(*base, 0, true))],
                };
                let wide = cells.iter().map(|c| c.0).fold(0.0f32, f32::max);
                let tall = cells.iter().map(|c| c.1).fold(0.0f32, f32::max);
                let r = 14.0 * u;
                let k = 0.866 * r;
                let top = -r - r * tall - r * 0.5 * wide;
                let c0 = v(cx - k * wide * 0.5, y + hgt * 0.5 - (top + r) * 0.5 + 2.0 * u);
                for &(ox, oy, b) in &cells {
                    let c = c0 + v(k * ox, r * 0.5 * ox - r * oy);
                    self.cv.cube(c, r, face_texture(b, 2), face_texture(b, 5), face_texture(b, 0), [255; 3], [255; 3]);
                }
                // The fire flickers in its mouth.
                let glow = 0.18 + 0.12 * (t * 9.0).sin() * (t * 5.3).cos();
                let fc = c0 + v(-k * 0.5, r * 0.5);
                self.cv.poly(
                    &[fc + v(-k * 0.3, -r * 0.1), fc + v(k * 0.3, r * 0.2), fc + v(k * 0.3, r * 0.55), fc + v(-k * 0.3, r * 0.25)],
                    rgba(255, 150, 40, (255.0 * glow.max(0.0)) as u8),
                );
                // Smoke out of the chimney or the hood's vents.
                let tops: Vec<Vec2> = match *base {
                    BLAST_FURNACE => vec![c0 + v(0.0, -r * 2.0)],
                    ADV_FURNACE => vec![c0 + v(k * 0.3, -r * 2.2), c0 + v(k * 0.9, -r * 1.9)],
                    _ => Vec::new(),
                };
                for (i, p) in tops.into_iter().enumerate() {
                    for j in 0..3 {
                        let k = phase(t + i as f32 * 0.4 + j as f32 / 3.0 * 1.8, 1.8);
                        self.smoke(p.x, p.y, k, 110, 16.0);
                    }
                }
            }
            Pic::Front => {
                let s = 44.0 * u;
                let (fx, fy) = (x + 6.0 * u, y + (hgt - s) * 0.5);
                let b = furnace_id(FURNACE, 0, true);
                self.cv.tex_quad([v(fx, fy), v(fx + s, fy), v(fx + s, fy + s), v(fx, fy + s)], face_texture(b, 5), 1.0, [255; 3], 1.0);
                // Flames dance in the firebox.
                for i in 0..6 {
                    let k = phase(t + i as f32 * 0.17, 0.7);
                    let fxx = fx + s * (0.3 + 0.07 * i as f32) + (t * 6.0 + i as f32).sin() * u;
                    let fyy = fy + s * 0.82 - k * s * 0.18;
                    let c = if k < 0.4 { rgba(255, 230, 120, 255) } else { rgba(255, 130, 40, 255) };
                    self.cv.fill(fxx, fyy, 2.0 * u, 2.0 * u, with_a(c, 1.0 - k));
                }
                // The line between the halves, dashed.
                let mut dx = fx - 2.0 * u;
                while dx < fx + s + 2.0 * u {
                    self.cv.fill(dx, fy + s * 0.5 - 0.5 * u, 3.0 * u, 1.0, th.gold);
                    dx += 5.0 * u;
                }
                let lx = fx + s + 16.0 * u;
                for (half, item, label) in [
                    (0.25, IRON_ORE as ItemId, tr(hu, "to smelt", "olvasztani")),
                    (0.75, COAL, tr(hu, "fuel", "üzemanyag")),
                ] {
                    let ly = fy + s * half;
                    self.cv.fill(fx + s + u, ly - 0.5 * u, 12.0 * u, 1.0, th.soft);
                    self.icon(item, 1, lx, ly - 7.0 * u, 14.0 * u);
                    self.cv.text(&label, lx + 17.0 * u, ly - 4.0, m.fs, th.ink, false);
                }
            }
            Pic::Grill => {
                let s = 52.0 * u;
                let (gx, gy) = (cx - s * 0.5, y + (hgt - s) * 0.5);
                self.cv.tex_quad([v(gx, gy), v(gx + s, gy), v(gx + s, gy + s), v(gx, gy + s)], face_texture(FURNACE, 2), 1.0, [255; 3], 1.0);
                let meats = meat(PORKCHOP).unwrap_or_default();
                // One piece after the other is turned over (it narrows, and the other side
                // shows).
                let turning = (t / 1.5) as usize % 4;
                let k = phase(t, 1.5);
                for (i, item) in [meats[0], meats[1], meats[2], meats[4]].into_iter().enumerate() {
                    let (qx, qy) = (gx + (i % 2) as f32 * s * 0.5, gy + (i / 2) as f32 * s * 0.5);
                    let size = s * 0.5 - 8.0 * u;
                    let (mut w, mut lift) = (size, 0.0);
                    if i == turning && k < 0.4 {
                        w = size * ((k / 0.4) * std::f32::consts::PI).cos().abs();
                        lift = ((k / 0.4) * std::f32::consts::PI).sin() * 5.0 * u;
                    }
                    let layer = match icon(item) {
                        Icon::Flat(l) => l,
                        Icon::Block(_) => tex::PORKCHOP,
                    };
                    let (ix, iy) = (qx + 4.0 * u + (size - w) * 0.5, qy + 4.0 * u - lift);
                    self.cv.tex_quad([v(ix, iy), v(ix + w, iy), v(ix + w, iy + size), v(ix, iy + size)], layer, 1.0, [255; 3], 1.0);
                    // Smoke, darker the more done.
                    let gray = [230, 200, 150, 60][i];
                    for j in 0..2 {
                        let kk = phase(t * 0.7 + i as f32 * 0.37 + j as f32 * 0.5, 1.0);
                        self.smoke(qx + s * 0.25, qy + 4.0 * u, kk, gray, 14.0);
                    }
                }
                let labels = [
                    (tr(hu, "raw", "nyers"), gx - 4.0 * u, gy + 4.0 * u, true),
                    (tr(hu, "half", "félig"), gx + s + 4.0 * u, gy + 4.0 * u, false),
                    (tr(hu, "cooked", "sült"), gx - 4.0 * u, gy + s * 0.5 + 4.0 * u, true),
                    (tr(hu, "burnt", "szenes"), gx + s + 4.0 * u, gy + s * 0.5 + 4.0 * u, false),
                ];
                for (l, lx, ly, left) in labels {
                    let tw = self.cv.font().text_width(&l, m.fs);
                    let px = if left { lx - tw } else { lx };
                    self.cv.text(&l, px, ly, m.fs, th.soft, false);
                }
            }
            Pic::Gun(kind) => {
                if let Icon::Flat(layer) = icon(kind.item()) {
                    // A shot now and then: the gun kicks back a little and settles.
                    let period = 1.8;
                    let k = phase(t, period);
                    let kick = (1.0 - k / 0.25).max(0.0);
                    let r = hgt * 0.5;
                    let c = v(cx - kick * kick * 3.0 * u, y + r - kick * kick * 1.5 * u);
                    self.cv.sprite(c, r, layer, [255; 3], 1.0);
                }
            }
            Pic::Tiers => {
                let n = TIER_ORDER.len() as f32;
                let col = m.w / n;
                // A gold frame goes along the tools.
                let lit = (t / 0.9) as usize % TIER_ORDER.len();
                for (i, tier) in TIER_ORDER.into_iter().enumerate() {
                    let px = x + col * (i as f32 + 0.5);
                    let ore = match tier.level() {
                        0 => COAL_ORE,
                        1 => COPPER_ORE,
                        2 => IRON_ORE,
                        3 => DIAMOND_ORE,
                        _ => OBSIDIAN,
                    };
                    if i == lit {
                        self.cv.fill(px - 11.0 * u, y, 22.0 * u, 44.0 * u, th.gold);
                        self.cv.fill(px - 10.0 * u, y + u, 20.0 * u, 42.0 * u, th.paper);
                    }
                    let lift = if i == lit { 2.0 * u } else { 0.0 };
                    self.icon(tool_id(ToolKind::Pickaxe, tier), 1, px - 8.0 * u, y + 2.0 * u - lift, 16.0 * u);
                    self.cv.fill(px - 0.5 * u, y + 20.0 * u, 1.0, 4.0 * u, th.soft);
                    self.icon(ore as ItemId, 1, px - 8.0 * u, y + 25.0 * u, 16.0 * u);
                }
            }
            Pic::Station => {
                self.icon(GUN_STATION as ItemId, 1, cx - 38.0 * u, y + 6.0 * u, 42.0 * u);
                if let Icon::Flat(layer) = icon(PISTOL) {
                    // The pistol floats over the table as it is put together there.
                    let bob = (t * 2.0).sin() * 3.0 * u;
                    self.cv.sprite(v(cx + 22.0 * u, y + 22.0 * u + bob), 18.0 * u, layer, [255; 3], 1.0);
                    for i in 0..4 {
                        let k = phase(t + i as f32 * 0.25, 1.0);
                        let a = i as f32 * 1.7 + t;
                        let (sx, sy) = (cx + 22.0 * u + a.cos() * 16.0 * u, y + 22.0 * u + a.sin() * 10.0 * u);
                        self.cv.fill(sx, sy, u * 1.5, u * 1.5, with_a(th.gold, 1.0 - k));
                    }
                }
            }
            Pic::Items(items) => {
                let step = 24.0 * u;
                let x0 = cx - step * items.len() as f32 * 0.5;
                for (i, &id) in items.iter().enumerate() {
                    let bob = ((t * 2.5 + i as f32 * 0.8).sin() * 1.5 * u).round();
                    let sx = x0 + i as f32 * step + 3.0 * u;
                    self.slot(sx - u, y + 2.0 * u, 18.0 * u);
                    self.icon(id, 1, sx, y + 3.0 * u + bob, 16.0 * u);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_book_fits_its_pages_in_both_languages() {
        let font = Font::new();
        let keys = ("E".to_string(), "R".to_string());
        let m = metrics();
        assert!(PAGE_PX.0 <= SHEET_W as f32 && PAGE_PX.1 <= SHEET_H as f32);
        for hu in [false, true] {
            let lay = layout(&font, hu, &keys);
            assert_eq!(lay.chapters.len(), 8, "hu={hu}");
            assert!(lay.chapters.windows(2).all(|c| c[0].1 < c[1].1));
            for (i, page) in lay.pages.iter().enumerate().skip(2) {
                assert!(!page.is_empty(), "empty page {i}");
                for (y, p) in page {
                    assert!(y + height(&m, p) <= m.h + 0.01, "page {i} runs over (hu={hu})");
                    let text = match p {
                        Piece::Line { text, indent, .. } => {
                            Some((text, if *indent { INDENT * m.u } else { 0.0 }))
                        }
                        Piece::Row(_, lines) => lines.first().map(|l| (l, 22.0 * m.u)),
                        _ => None,
                    };
                    if let Some((t, indent)) = text {
                        assert!(font.text_width(t, m.fs) <= m.w - indent + 0.01, "too wide: {t}");
                    }
                    if let Piece::Recipe(id) = p {
                        assert!(recipe_view(*id).is_some(), "no recipe for {id}");
                    }
                }
            }
            // The contents fit on their page, and their rows are found again.
            let (top, row) = contents_rows(&m);
            assert!(top + row * lay.chapters.len() as f32 <= MARGIN_TOP * m.u + m.h);
            assert_eq!(contents_entry(&lay, top + row * 2.5), Some(2));
        }
    }

    #[test]
    fn every_page_draws() {
        let font = Font::new();
        let texture = vec![200u8; crate::world::textures::TILE * crate::world::textures::TILE * 4 * tex::LAYERS];
        let lay = layout(&font, true, &("E".into(), "R".into()));
        for theme in [&LIGHT, &DARK] {
            for n in 0..lay.pages.len() {
                let look = Look { hu: true, time: n as f32 * 0.7, theme, hover: Some(1) };
                let cv = draw_page(&font, &texture, &lay, n, &look);
                assert_eq!(cv.px.len(), SHEET_W * SHEET_H);
            }
        }
    }
}
