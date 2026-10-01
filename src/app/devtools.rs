//! Developer tools run from the command line instead of the game:
//! `--map [seed] [blocks per pixel]`, `--stats [seeds]` and `--sizes [seeds]`.

use crate::world;

/// Runs the tool named by `args[1]`; false if there is none (start the game).
pub fn run(args: &[String]) -> bool {
    let num = |i: usize, default: u32| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default);
    match args.get(1).map(String::as_str) {
        Some("--sizes") => write_sizes(num(2, 4)),
        Some("--stats") => write_stats(num(2, 4)),
        Some("--map") => write_map(num(2, 12345), num(3, 4).max(1) as i32),
        _ => return false,
    }
    true
}
/// Dev tool: `rustcraft --map [seed] [blocks per pixel]` writes a biome/height map to
/// map.bmp (biome colors with hill shading lit from the north-west).
fn write_map(seed: u32, step: i32) {
    use world::gen::{Biome, Generator, SEA};
    let gen = Generator::new(seed);
    let size = 768i32;
    let cols: Vec<_> = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size, i / size);
            gen.column((x - size / 2) * step, (y - size / 2) * step)
        })
        .collect();
    let height =
        |x: i32, y: i32| cols[(y.clamp(0, size - 1) * size + x.clamp(0, size - 1)) as usize].height;
    let mut px = Vec::with_capacity((size * size * 3) as usize);
    for y in (0..size).rev() {
        for x in 0..size {
            let c = cols[(y * size + x) as usize];
            let base: [f32; 3] = match c.biome {
                Biome::Ocean => [40.0, 70.0, 170.0],
                Biome::FrozenOcean => [140.0, 170.0, 220.0],
                Biome::River => [60.0, 110.0, 230.0],
                Biome::Beach => [225.0, 212.0, 150.0],
                Biome::StonyShore => [120.0, 116.0, 110.0],
                Biome::Plains => [120.0, 180.0, 80.0],
                Biome::Forest => [50.0, 120.0, 40.0],
                Biome::BirchForest => [90.0, 150.0, 70.0],
                Biome::Desert => [230.0, 200.0, 120.0],
                Biome::Taiga => [60.0, 100.0, 80.0],
                Biome::SnowyTaiga => [150.0, 180.0, 170.0],
                Biome::SnowyPlains => [235.0, 240.0, 250.0],
                Biome::Mountains => {
                    if c.height > 130 {
                        [245.0, 245.0, 250.0]
                    } else {
                        [135.0, 135.0, 135.0]
                    }
                }
            };
            let water = c.height < SEA;
            let shade = if water {
                (0.75 + (c.height - SEA) as f32 * 0.012).clamp(0.45, 1.0)
            } else {
                // Height differences toward the north-west light up slopes facing it.
                let d = (height(x - 1, y + 1) - height(x + 1, y - 1)) as f32 / step as f32;
                (0.92 + d * 0.35 + (c.height - SEA) as f32 * 0.0015).clamp(0.45, 1.3)
            };
            px.extend([
                (base[2] * shade).min(255.0) as u8,
                (base[1] * shade).min(255.0) as u8,
                (base[0] * shade).min(255.0) as u8,
            ]);
        }
    }
    let data_len = px.len() as u32;
    let mut bmp = Vec::new();
    bmp.extend(b"BM");
    bmp.extend((54 + data_len).to_le_bytes());
    bmp.extend([0u8; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(size.to_le_bytes());
    bmp.extend(size.to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(24u16.to_le_bytes());
    bmp.extend([0u8; 24]);
    bmp.extend(px);
    std::fs::write("map.bmp", bmp).expect("write map.bmp");
    println!("wrote map.bmp (seed {seed}, {} blocks across)", size * step);
}

/// Dev tool: `rustcraft --stats [seeds]` prints how much of the world each biome covers
/// (and what borders what), sampled over 16384 x 16384 blocks per seed.
fn write_stats(seeds: u32) {
    use world::gen::Generator;
    let names: Vec<&str> = (0..13).map(|i| biome_by_index(i).name()).collect();
    let mut count = [0u64; 13];
    let mut border = [[0u64; 13]; 13];
    let mut heights = [0u64; 5];
    // Land relief within 16 blocks around a sample: flat (<= 3), rolling (<= 10), hilly.
    let mut relief = [0u64; 3];
    let (n, step) = (512i32, 32i32);
    for seed in 1..=seeds {
        let gen = Generator::new(seed.wrapping_mul(0x9E37_79B9));
        let grid: Vec<_> = (0..n * n)
            .map(|i| gen.column((i % n - n / 2) * step, (i / n - n / 2) * step))
            .collect();
        for y in 0..n {
            for x in 0..n {
                let c = grid[(y * n + x) as usize];
                let b = c.biome as usize;
                count[b] += 1;
                if c.height >= world::gen::SEA {
                    let (wx, wz) = ((x - n / 2) * step, (y - n / 2) * step);
                    let hs = [(16, 0), (-16, 0), (0, 16), (0, -16)]
                        .map(|(dx, dz)| gen.column(wx + dx, wz + dz).height);
                    let span = hs.iter().chain([&c.height]).max().unwrap()
                        - hs.iter().chain([&c.height]).min().unwrap();
                    relief[match span {
                        ..=3 => 0,
                        4..=10 => 1,
                        _ => 2,
                    }] += 1;
                }
                heights[match c.height {
                    ..62 => 0,
                    62..=80 => 1,
                    81..=100 => 2,
                    101..=130 => 3,
                    _ => 4,
                }] += 1;
                if x + 1 < n {
                    let o = grid[(y * n + x + 1) as usize].biome as usize;
                    if o != b {
                        border[b][o] += 1;
                        border[o][b] += 1;
                    }
                }
            }
        }
    }
    let total: u64 = count.iter().sum();
    println!("Biome coverage ({seeds} seeds):");
    for (i, &c) in count.iter().enumerate() {
        let mut nb: Vec<(u64, usize)> = (0..13)
            .map(|j| (border[i][j], j))
            .filter(|e| e.0 > 0)
            .collect();
        nb.sort_by_key(|e| std::cmp::Reverse(e.0));
        let nbs: Vec<String> = nb
            .iter()
            .take(4)
            .map(|&(_, j)| names[j].to_string())
            .collect();
        println!(
            "  {:<13} {:5.1}%   borders: {}",
            names[i],
            c as f64 * 100.0 / total as f64,
            nbs.join(", ")
        );
    }
    let hn = ["water", "62-80", "81-100", "101-130", ">130"];
    let hs: Vec<String> = heights
        .iter()
        .zip(hn)
        .map(|(&c, n)| format!("{n}: {:.1}%", c as f64 * 100.0 / total as f64))
        .collect();
    println!("Heights: {}", hs.join("  "));
    let land: u64 = relief.iter().sum();
    let pct = |i: usize| relief[i] as f64 * 100.0 / land as f64;
    println!(
        "Land relief: flat {:.1}%  rolling {:.1}%  hilly {:.1}%",
        pct(0),
        pct(1),
        pct(2)
    );
    let open = [5usize, 8, 11].iter().map(|&i| count[i]).sum::<u64>();
    let wooded = [6usize, 7, 9, 10].iter().map(|&i| count[i]).sum::<u64>();
    println!(
        "Open land (plains, desert, snowy plains) {:.1}%  vs  forests {:.1}%",
        open as f64 * 100.0 / total as f64,
        wooded as f64 * 100.0 / total as f64
    );
}

/// Dev tool: `rustcraft --sizes [seeds]` prints how big connected patches of each land
/// biome are (rivers and shores crossing a biome do not split it): the size of a patch is
/// the square root of its area, a typical width in blocks.
fn write_sizes(seeds: u32) {
    use world::gen::Generator;
    let (n, step) = (512i32, 16i32);
    let mut sizes: Vec<Vec<f64>> = vec![Vec::new(); 13];
    for seed in 1..=seeds {
        let gen = Generator::new(seed.wrapping_mul(0x9E37_79B9));
        let grid: Vec<usize> = (0..n * n)
            .map(|i| gen.biome_region((i % n - n / 2) * step, (i / n - n / 2) * step) as usize)
            .collect();
        let mut seen = vec![false; grid.len()];
        for start in 0..grid.len() {
            if seen[start] {
                continue;
            }
            let b = grid[start];
            let (mut area, mut edge) = (0u64, false);
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(i) = stack.pop() {
                area += 1;
                let (x, y) = (i as i32 % n, i as i32 / n);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if !(0..n).contains(&nx) || !(0..n).contains(&ny) {
                        edge = true;
                        continue;
                    }
                    let j = (ny * n + nx) as usize;
                    if !seen[j] && grid[j] == b {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            // Patches cut by the sampled area's edge would look smaller than they are.
            if !edge {
                sizes[b].push((area as f64).sqrt() * step as f64);
            }
        }
    }
    println!("Biome patch sizes (sqrt of area, blocks; {seeds} seeds of 8192 x 8192):");
    println!("  (fragments = patches under 100 blocks; percentiles over the larger ones)");
    println!("  (typical = the size of the patch a random spot of that biome lies in)");
    println!(
        "  {:<13} {:>9} {:>7} {:>7} {:>7} {:>7} {:>8}",
        "", "fragments", "patches", "p10", "median", "p90", "typical"
    );
    for (i, v) in sizes.iter_mut().enumerate() {
        v.sort_by(|a, b| a.total_cmp(b));
        let small = v.iter().filter(|&&s| s < 100.0).count();
        let big = &v[small..];
        if big.is_empty() {
            continue;
        }
        let q = |f: f64| big[((big.len() - 1) as f64 * f) as usize];
        // Area-weighted median: half of the biome's area lies in patches at least this big.
        let total: f64 = v.iter().map(|s| s * s).sum();
        let mut acc = 0.0;
        let typical = v
            .iter()
            .find(|&&s| {
                acc += s * s;
                acc >= total * 0.5
            })
            .copied()
            .unwrap_or(0.0);
        println!(
            "  {:<13} {:>9} {:>7} {:>7.0} {:>7.0} {:>7.0} {:>8.0}",
            biome_by_index(i).name(),
            small,
            big.len(),
            q(0.1),
            q(0.5),
            q(0.9),
            typical
        );
    }
}

fn biome_by_index(i: usize) -> world::gen::Biome {
    use world::gen::Biome::*;
    [
        Ocean,
        FrozenOcean,
        River,
        Beach,
        StonyShore,
        Plains,
        Forest,
        BirchForest,
        Desert,
        Taiga,
        SnowyTaiga,
        SnowyPlains,
        Mountains,
    ][i]
}
