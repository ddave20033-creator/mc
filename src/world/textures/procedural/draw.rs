//! Drawing helpers shared by the procedural textures: colours, design units, distances,
//! bevels and outlined shapes.

use super::*;

/// Pixels per design unit.
pub(super) const K: f32 = TILE as f32 / 32.0;

pub(super) const GRAY: [f32; 3] = [255.0, 255.0, 255.0];

pub(super) fn col(rgb: [f32; 3], v: f32, a: u8) -> [u8; 4] {
    [
        (rgb[0] * v).clamp(0.0, 255.0) as u8,
        (rgb[1] * v).clamp(0.0, 255.0) as u8,
        (rgb[2] * v).clamp(0.0, 255.0) as u8,
        a,
    ]
}

pub(super) fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Pixel center in design units (0..32).
pub(super) fn d(v: i32) -> f32 {
    (v as f32 + 0.5) / K
}

/// Distance from point to segment.
pub(super) fn seg_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
}

/// Soft bevel for a rectangle (in pixels): > 1 near the top/left edge, < 1 near bottom/right.
pub(super) fn bevel(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, w: f32) -> f32 {
    let tl = (x - x0).min(y - y0);
    let br = (x1 - x).min(y1 - y);
    if tl < w && tl <= br {
        1.0 + 0.16 * (1.0 - tl / w)
    } else if br < w {
        1.0 - 0.2 * (1.0 - br / w)
    } else {
        1.0
    }
}

pub(super) fn in_polygon(x: f32, y: f32, points: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Filled shape given by `inside(x, y)` in design units, with a dark outline, a highlight
/// toward the top-left and a smooth shading gradient.
pub(super) fn shape(x: i32, y: i32, color: [f32; 3], inside: impl Fn(f32, f32) -> bool) -> Option<[u8; 4]> {
    let (fx, fy) = (d(x), d(y));
    if !inside(fx, fy) {
        return None;
    }
    let e = 0.8;
    let edge =
        !inside(fx - e, fy) || !inside(fx + e, fy) || !inside(fx, fy - e) || !inside(fx, fy + e);
    let light = !inside(fx - 1.6, fy - 1.6);
    let dark = !inside(fx + 1.6, fy + 1.6);
    let v = if edge {
        0.45
    } else if light {
        1.22
    } else if dark {
        0.78
    } else {
        1.0 - (fx + fy) * 0.005
    };
    Some(col(color, v, 255))
}
