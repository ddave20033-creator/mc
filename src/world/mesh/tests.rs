//! The mesher's tests.

use super::shapes::bed_local;
use super::*;

/// No cuts in any trunk (see `World::notches`).
const NO_NOTCHES: &[(glam::IVec3, Notch)] = &[];

/// A 3x3 neighbourhood of chunks with a stone floor at y 0, the center chunk from `edit`.
fn hood(edit: impl Fn(&mut ChunkData)) -> [Arc<ChunkData>; 9] {
    let floor = || {
        let mut c = ChunkData::new();
        for z in 0..16 {
            for x in 0..16 {
                c.set(x, 0, z, STONE);
            }
        }
        c
    };
    let mut center = floor();
    edit(&mut center);
    std::array::from_fn(|i| Arc::new(if i == 4 { center.clone() } else { floor() }))
}

/// Prints where meshing time goes (`cargo test --release mesh_speed -- --nocapture`).
#[test]
fn mesh_speed() {
    let gen = Generator::new(1201871768);
    let chunks: Vec<Arc<ChunkData>> = (0..25)
        .map(|i| Arc::new(gen.generate_chunk(i % 5, i / 5)))
        .collect();
    let at = |x: i32, z: i32| chunks[(z * 5 + x) as usize].clone();
    let hoods: Vec<[Arc<ChunkData>; 9]> = (1..4)
        .flat_map(|z| (1..4).map(move |x| (x, z)))
        .map(|(x, z)| std::array::from_fn(|i| at(x + i as i32 % 3 - 1, z + i as i32 / 3 - 1)))
        .collect();
    let t = std::time::Instant::now();
    let mut vertices = 0;
    for (i, nb) in hoods.iter().enumerate() {
        let m = mesh_chunk(((i % 3) as i32 + 1, (i / 3) as i32 + 1), nb, &[], NO_NOTCHES, &gen);
        vertices += m.vertices.len();
    }
    println!("9 chunks meshed in {:?}, {vertices} vertices", t.elapsed());
}

#[test]
fn beds_turn_their_head_north_for_the_textures() {
    for f in 0..4 {
        let head = facing_dir(f);
        assert_eq!(bed_local(head, f), glam::IVec3::NEG_Z);
        // The bed's right (its east when facing north) stays on its right.
        assert_eq!(bed_local(facing_dir(f + 1), f), glam::IVec3::X);
    }
}

#[test]
fn glass_wall_faces_join_their_neighbours() {
    // A glass wall three blocks wide and one high along x, at z 8.
    let nb = hood(|c| {
        for x in 7..10 {
            c.set(x, 1, 8, GLASS);
        }
    });
    let r = Region::new(&nb);
    // Face -Z of the middle pane: joined left and right (u runs along -x), not up or down.
    let mask = glass_mask(&r, 16 + 8, 1, 16 + 8, 5);
    assert_eq!(mask & 0b11, 0b11, "{mask:08b}");
    assert_eq!(mask & 0b1100, 0, "{mask:08b}");
    // The end pane joins only toward the middle.
    let end = glass_mask(&r, 16 + 7, 1, 16 + 8, 5);
    assert_eq!((end & 0b11).count_ones(), 1, "{end:08b}");
}

#[test]
fn a_just_broken_block_takes_the_light_around_it() {
    // A stone block on the floor, under the open sky.
    let nb = hood(|c| c.set(8, 1, 8, STONE));
    let m = mesh_chunk((0, 0), &nb, &[], NO_NOTCHES, &Generator::new(1));
    let mut world = World::new();
    world.chunks.insert((0, 0), nb[4].clone());
    world.light.insert((0, 0), m.light.clone());
    // Broken: its cell still has the solid block's light until the chunk is lit again.
    world.set(8, 1, 8, AIR);
    let at = glam::IVec3::new(8, 1, 8);
    assert!(world.light_estimate(at.as_vec3() + glam::Vec3::splat(0.5)).0 < 15);
    assert_eq!(world.light_around(at).0, 15);
}

#[test]
fn chests_are_listed_for_their_lids() {
    let nb = hood(|c| {
        c.set(3, 1, 4, crate::world::CHEST);
        c.set(10, 5, 12, crate::world::CHEST + 2);
    });
    let m = mesh_chunk((0, 0), &nb, &[], NO_NOTCHES, &Generator::new(1));
    let mut chests = m.chests.clone();
    chests.sort_by_key(|p| p.x);
    assert_eq!(chests, [glam::IVec3::new(3, 1, 4), glam::IVec3::new(10, 5, 12)]);
}

#[test]
fn things_under_a_lintel_get_the_light_around_them() {
    // A door in a wall with a beam over it: the door's cell is lit from the open sides,
    // not dark as if it were under a roof.
    let nb = hood(|c| {
        for x in 6..11 {
            for y in 1..4 {
                c.set(x, y, 8, PLANKS);
            }
        }
        c.set(8, 1, 8, door_id(0, false, false, false));
        c.set(8, 2, 8, door_id(0, false, true, false));
    });
    let gen = Generator::new(1);
    let m = mesh_chunk((0, 0), &nb, &[], NO_NOTCHES, &gen);
    let mut world = World::new();
    world.chunks.insert((0, 0), nb[4].clone());
    world.light.insert((0, 0), m.light.clone());
    assert_eq!(m.doors.len(), 2);
    for y in [1.5, 2.5] {
        let (sky, _) = world.light_estimate(glam::Vec3::new(8.5, y, 8.5));
        assert!(sky >= 14, "door at y {y}: sky {sky}");
    }
    // Inside the wall: the light of its open sides.
    assert_eq!(world.light_estimate(glam::Vec3::new(6.5, 1.5, 8.5)).0, 15);
}

/// FNV-1a over bytes: a fingerprint of generated terrain and meshes.
struct Fnv(u64);
impl Fnv {
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn mesh(&mut self, m: &MeshData) {
        for v in &m.vertices {
            v.pos.iter().chain(&v.uv).chain([&v.layer]).for_each(|&c| self.f32(c));
            self.bytes(&v.light);
            self.bytes(&v.tint);
        }
        m.indices.iter().for_each(|&i| self.u32(i));
        for c in [m.opaque_count, m.solid_count, m.leaf_inner_count] {
            self.u32(c);
        }
        m.dir_counts.iter().chain(&m.cut_dir_counts).for_each(|&c| self.u32(c));
        self.f32(m.min_y);
        self.f32(m.max_y);
        for p in m.doors.iter().chain(&m.chests).chain(&m.gun_stations).chain(&m.torches).chain(&m.stump_marks) {
            p.to_array().iter().for_each(|&c| self.u32(c as u32));
        }
        self.u32(m.light.h as u32);
        self.bytes(&m.light.data);
    }
}

/// Generated terrain, its meshes and the block properties stay exactly the same (a
/// guard for refactoring: the fingerprint changes only when generation or meshing does).
#[test]
fn terrain_and_meshes_are_unchanged() {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for b in 0..BLOCK_IDS as Block {
        h.bytes(&[is_opaque(b) as u8, is_solid(b) as u8, attenuates_sky(b) as u8, emission(b)]);
        (0..6).for_each(|f| h.u32(face_texture(b, f)));
    }
    let fingerprint = |h: &Fnv| format!("{:016x}", h.0);
    let mut chunks = 0;
    for (seed, cx, cz) in [(1201871768u32, 0, 0), (12345, 40, -7), (7, -300, 120), (99991, 5, 900)] {
        let gen = Generator::new(seed);
        let nb: [Arc<ChunkData>; 9] =
            std::array::from_fn(|i| Arc::new(gen.generate_chunk(cx + i as i32 % 3 - 1, cz + i as i32 / 3 - 1)));
        for c in &nb {
            for y in 0..HEIGHT {
                for z in 0..16 {
                    c.row(y, z).iter().for_each(|b| h.bytes(&b.to_le_bytes()));
                }
            }
            h.bytes(&c.heightmap);
            h.bytes(&[c.max_y]);
        }
        // A fluid that just changed, to animate.
        let anim = [(glam::IVec3::new(cx * 16 + 3, SEA_TEST, cz * 16 + 4), AIR, 0.5)];
        h.mesh(&mesh_chunk((cx, cz), &nb, &anim, NO_NOTCHES, &gen));
        chunks += 1;
    }
    let (px, pz) = (1000, -1000);
    let (nb, notches) = all_blocks(px, pz);
    let m = mesh_chunk((px, pz), &nb, &[], &notches, &Generator::new(3));
    h.mesh(&m);
    println!("{chunks} generated chunks + all blocks: {}", fingerprint(&h));
    assert_eq!(fingerprint(&h), "52a1fe0493e8785e");
}

/// Every block id on a floor, spaced out, and a few next to each other (stairs bending,
/// double chests, glass joining, logs meeting), for a chunk at `(px, pz)`, and cuts in two
/// trunks.
fn all_blocks(px: i32, pz: i32) -> ([Arc<ChunkData>; 9], [(glam::IVec3, Notch); 2]) {
    let nb = hood(|c| {
        for b in 1..BLOCK_IDS as Block {
            let i = b as usize;
            c.set_raw(i % 8 * 2, 1 + i / 64 * 2, i / 8 % 8 * 2, b);
        }
        for x in 0..16 {
            c.set_raw(x, 12, 15, stairs_id((x % 4) as u8, x % 3 == 0));
            c.set_raw(x, 14, 15, if x % 2 == 0 { GLASS } else { OAK_LOG_X });
            c.set_raw(x, 15, 15, [OAK_LOG, BIRCH_BRANCH_X, SPRUCE_BRANCH_Z, OAK_LEAVES][x % 4]);
        }
        c.set_raw(0, 12, 14, chest_id(0, 1));
        c.set_raw(1, 12, 14, chest_id(0, -1));
        for y in 16..20 {
            c.set_raw(3, y, 3, OAK_LOG);
            c.set_raw(6, y, 3, BIRCH_LOG);
        }
        c.recompute();
    });
    let (n1, n2) = (glam::IVec3::new(px * 16 + 3, 17, pz * 16 + 3), glam::IVec3::new(px * 16 + 6, 18, pz * 16 + 3));
    let notches = [
        (n1, Notch { angle: 0.7, height: 0.5, depth: 0.6, felled: false }),
        (n2, Notch { angle: 2.1, height: 0.4, depth: 0.8, felled: true }),
    ];
    (nb, notches)
}

/// The whole-block faces drawn without alpha testing (the first solid indices, see
/// `MeshData`) have no see-through texel at any mip level, in every block and in generated
/// terrain.
#[test]
fn plain_faces_have_no_see_through_texels() {
    let levels = crate::textures::generate(&crate::textures::resource_pack::Packs::none());
    let layers = levels[0].len() / (crate::textures::TILE * crate::textures::TILE * 4);
    let opaque_layer = |l: usize| {
        levels.iter().all(|lv| {
            let n = lv.len() / layers;
            lv[l * n..(l + 1) * n].chunks_exact(4).all(|p| p[3] >= 128)
        })
    };
    let (nb, notches) = all_blocks(1000, -1000);
    let mut meshes = vec![mesh_chunk((1000, -1000), &nb, &[], &notches, &Generator::new(3))];
    for (seed, cx, cz) in [(1201871768u32, 0, 0), (12345, 40, -7)] {
        let gen = Generator::new(seed);
        let nb: [Arc<ChunkData>; 9] =
            std::array::from_fn(|i| Arc::new(gen.generate_chunk(cx + i as i32 % 3 - 1, cz + i as i32 / 3 - 1)));
        meshes.push(mesh_chunk((cx, cz), &nb, &[], NO_NOTCHES, &gen));
    }
    let mut checked = std::collections::HashSet::new();
    for m in &meshes {
        let plain: u32 = m.dir_counts.iter().sum();
        assert!(plain > 0);
        for &i in &m.indices[..plain as usize] {
            let v = m.vertices[i as usize];
            assert_eq!(v.tint[3] & (flags::LEAVES | flags::PLANT | flags::OVERLAY), 0);
            let l = v.layer as usize;
            if checked.insert(l) {
                assert!(opaque_layer(l), "layer {l} has see-through texels");
            }
        }
    }
}

const SEA_TEST: i32 = crate::world::gen::SEA;

#[test]
fn torches_and_stump_marks_are_listed() {
    let nb = hood(|c| {
        c.set(3, 1, 4, TORCH);
        c.set(5, 0, 5, stump_mark(GRASS, 0));
    });
    let m = mesh_chunk((0, 0), &nb, &[], NO_NOTCHES, &Generator::new(1));
    assert_eq!(m.torches, vec![glam::IVec3::new(3, 1, 4)]);
    assert_eq!(m.stump_marks, vec![glam::IVec3::new(5, 0, 5)]);
}


/// Every vertex of every block and of generated terrain packs into the GPU's chunk vertex
/// (`render::chunks::ChunkVertex`) and reads back as it was: its position and uv within their
/// steps, its layer, light and tint the same.
#[test]
fn chunk_vertices_pack_without_loss() {
    use crate::render::chunks::pack_vertex;
    let check = |m: &MeshData| {
        let (x0, z0) = ((m.pos.0 * 16) as f32, (m.pos.1 * 16) as f32);
        for v in &m.vertices {
            let p = pack_vertex(v, x0, z0, 0.0);
            let back = [p.pos[0] as f32 / 2048.0 - 8.0 + x0, p.pos[1] as f32 / 128.0 - 32.0, p.pos[2] as f32 / 2048.0 - 8.0 + z0];
            for k in 0..3 {
                assert!((back[k] - v.pos[k]).abs() <= 1.0 / 256.0, "{v:?} -> {back:?}");
            }
            if v.tint[3] & flags::FLUID == 0 {
                for k in 0..2 {
                    assert!((p.uv[k] as i16 as f32 / 4096.0 - v.uv[k]).abs() <= 1.0 / 8192.0, "{v:?}");
                }
            }
            let layer = (p.layer & 0x7fff) as f32 + if p.layer & 0x8000 != 0 { 0.25 } else { 0.0 };
            assert_eq!(layer, v.layer);
            assert_eq!((p.light, p.tint), (v.light, v.tint));
        }
    };
    let (px, pz) = (1000, -1000);
    let (nb, notches) = all_blocks(px, pz);
    check(&mesh_chunk((px, pz), &nb, &[], &notches, &Generator::new(3)));
    let gen = Generator::new(12345);
    for c in 0..8 {
        let nb: [Arc<ChunkData>; 9] = std::array::from_fn(|i| Arc::new(gen.generate_chunk(c * 5 + i as i32 % 3 - 1, i as i32 / 3 - 1)));
        // (a fluid changing, to animate)
        let anim = [(glam::IVec3::new(c * 80 + 3, SEA_TEST, 4), AIR, 0.5)];
        check(&mesh_chunk((c * 5, 0), &nb, &anim, NO_NOTCHES, &gen));
    }
}
