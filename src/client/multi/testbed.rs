//! The testbed's LAN commands (`lan open`, `lan join <addr> <name>`, `lan report`,
//! `lan block <x> <y> <z>`): two game windows, one hosting and one joining, each with its
//! script, and what each sees in its report.

use super::*;

impl Game {
    /// Runs `lan <args>`; the lines for the report.
    pub(in crate::client) fn testbed_lan(&mut self, args: &str) -> Vec<String> {
        let w: Vec<&str> = args.split_whitespace().collect();
        match w.as_slice() {
            ["open"] => {
                self.open_to_lan();
                match &self.session.lan_address {
                    Some(address) => vec![format!("LAN open at {address}")],
                    None => vec!["LAN could not open".into()],
                }
            }
            ["join", addr, name] => {
                // (a name of its own: the host's window has the same settings)
                self.settings.name = name.to_string();
                self.connect_to(addr.to_string());
                vec![format!("joining {addr} as {name}")]
            }
            ["block", x, y, z] => {
                let p = [x, y, z].map(|v| v.parse::<i32>().unwrap_or(0));
                let p = IVec3::from(p);
                let cp = World::chunk_pos(p.x, p.z);
                let w = &self.terrain.world;
                // (by key and state: `oak_door:9`)
                let named = |b: Block| format!("{}:{}", def(b).key, b - base(b));
                let block = if w.chunks.contains_key(&cp) {
                    named(w.geti(p))
                } else {
                    let saved = w.saved.get(&cp).map(|c| named(c.get(p.x.rem_euclid(16) as usize, p.y as usize, p.z.rem_euclid(16) as usize)));
                    format!("not loaded (saved copy: {saved:?})")
                };
                vec![format!("block {} {} {}: {block}", p.x, p.y, p.z)]
            }
            ["report"] => self.lan_report(),
            // The block this player aims at (and the one in front of it, where one goes).
            ["target"] => match self.me.aim.target {
                Some((hit, prev)) => vec![format!(
                    "target {} {} {}: {}, before it {} {} {}",
                    hit.x,
                    hit.y,
                    hit.z,
                    def(self.terrain.world.geti(hit)).key,
                    prev.x,
                    prev.y,
                    prev.z
                )],
                None => vec!["target: none".into()],
            },
            _ => vec![format!("unknown lan command `{args}`")],
        }
    }

    fn lan_report(&self) -> Vec<String> {
        let mut out = Vec::new();
        let (mobs, items) = (self.level.mobs.len(), self.level.items.len());
        match &self.session.net {
            Some(_) => {
                let w = &self.terrain.world;
                let pending: usize = w.pending.values().map(|v| v.len()).sum();
                out.push(format!(
                    "player: {} other player(s), {mobs} mobs, {items} items, {} chunks loaded, {} saved, {pending} changes waiting in {} chunks, screen {:?}",
                    self.session.remotes.len(),
                    w.chunks.len(),
                    w.saved.len(),
                    w.pending.len(),
                    self.screen
                ));
            }
            None => out.push("not in a LAN game".into()),
        }
        out
    }
}
