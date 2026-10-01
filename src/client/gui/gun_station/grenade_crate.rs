//! The rifle station's grenade crate on its shelf, used without opening the station: grenades
//! put in and taken out with a right click on it.

use super::table::Table;
use crate::client::Game;
use crate::item::inventory::take;
use crate::item::{FRAG_GRENADE, SMOKE_GRENADE, Stack};
use crate::model::gun_station::{CRATE_MAX, crate_halves};
use crate::util::ray_box;
use crate::world::bench_main;
use glam::IVec3;

impl Game {
    /// The half of a rifle station's grenade crate the crosshair is on (the station's left
    /// block, 0 frag grenades / 1 smoke grenades), within reach.
    pub(in crate::client) fn crate_under_crosshair(&self) -> Option<(IVec3, usize)> {
        let (hit, _) = self.me.aim.target?;
        let w = &self.terrain.world;
        let main = bench_main(hit, w.geti(hit), |q| w.geti(q))?;
        let table = Table::of(main, w.geti(main)).filter(|t| t.rifle())?;
        let (eye, dir) = (self.eye(), self.me.look.dir());
        let mut found: Option<(usize, f32)> = None;
        for (half, lo, hi, m) in crate_halves(main, table.toward) {
            let inv = m.inverse();
            let (o, d) = (inv.transform_point3(eye), inv.transform_vector3(dir));
            // (the ray in model pixels: its length scales with them)
            let reach = 5.0 * d.length();
            if let Some(t) = ray_box(o, d.normalize(), lo, hi, reach) {
                if found.is_none_or(|(_, bt)| t < bt) {
                    found = Some((half, t));
                }
            }
        }
        found.map(|(half, _)| (main, half))
    }

    /// Right click on a rifle station's grenade crate: holding grenades, one goes in (into the
    /// half for its kind; sneaking, as many as there is room for); otherwise one is taken out
    /// of the half clicked (sneaking, all of it). False if the crosshair is not on the crate.
    pub(in crate::client) fn crate_click(&mut self) -> bool {
        let Some((main, half)) = self.crate_under_crosshair() else { return false };
        let kinds = [FRAG_GRENADE, SMOKE_GRENADE];
        let held = self.held();
        let slot = self.me.items.hotbar_slot;
        let creative = self.creative();
        let sneaking = self.sneaking();
        let bench = self.level.block_entities.benches.entry(main).or_default();
        if let Some(i) = kinds.iter().position(|&k| k == held) {
            let count = self.me.items.inventory.slots[slot].map_or(0, |s| s.count);
            let k = (if sneaking { count } else { 1 }).min(CRATE_MAX - bench.grenades[i].min(CRATE_MAX));
            if k == 0 {
                return true;
            }
            bench.grenades[i] += k;
            if !creative {
                take(&mut self.me.items.inventory.slots[slot], k);
            }
        } else if bench.grenades[half] > 0 {
            let n = if sneaking { bench.grenades[half] } else { 1 };
            bench.grenades[half] -= n;
            self.give(Stack::new(kinds[half], n));
        } else {
            return true;
        }
        self.audio.play(crate::audio::Sound::GrenadeBounce, Some(self.eye()), 0.35);
        self.me.hand.swing();
        self.bench_changed(main, None);
        true
    }
}
