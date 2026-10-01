//! What the UI is drawn with: rounded rectangles, gradients, rings, the vignette, block and
//! item textures, and text.

use super::*;

impl Ui {
    pub(super) fn push(&mut self, p: [Vec2; 4], uv: [[f32; 2]; 4], c: [Color; 4], rect: [f32; 4], mode: f32) {
        if self.fade <= 0.001 {
            return;
        }
        for i in [0, 1, 2, 0, 2, 3] {
            self.verts.push(UiVertex {
                pos: (p[i] + self.offset).to_array(),
                uv: uv[i],
                color: with_alpha(c[i], self.fade),
                rect,
                mode,
            });
        }
    }

    /// Rounded rectangle with a vertical gradient and optional soft (blurred) edge.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_full(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        top: Color,
        bottom: Color,
        radius: f32,
        soft: f32,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let m = soft + 1.0;
        let (hw, hh) = (w * 0.5, h * 0.5);
        let c = Vec2::new(x + hw, y + hh);
        let r = radius.min(hw).min(hh).max(0.0);
        let p = [
            Vec2::new(x - m, y - m),
            Vec2::new(x + w + m, y - m),
            Vec2::new(x + w + m, y + h + m),
            Vec2::new(x - m, y + h + m),
        ];
        let uv = p.map(|q| (q - c).to_array());
        self.push(
            p,
            uv,
            [top, top, bottom, bottom],
            [hw, hh, r, soft],
            MODE_RECT,
        );
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color, radius: f32) {
        self.rect_full(x, y, w, h, c, c, radius, 0.0);
    }

    /// Hard-edged gradient quad (no anti-aliasing) - for backgrounds and pixel art.
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let p = [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x + w, y + h),
            Vec2::new(x, y + h),
        ];
        self.push(
            p,
            [[0.0; 2]; 4],
            [top, top, bottom, bottom],
            [1e6, 1e6, 0.0, 0.0],
            MODE_RECT,
        );
    }

    /// Hard-edged gradient quad from left to right.
    pub fn hgradient(&mut self, x: f32, y: f32, w: f32, h: f32, left: Color, right: Color) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let p = [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x + w, y + h),
            Vec2::new(x, y + h),
        ];
        self.push(p, [[0.0; 2]; 4], [left, right, right, left], [1e6, 1e6, 0.0, 0.0], MODE_RECT);
    }

    /// Hard-edged solid quad of any shape (a triangle when two corners are the same).
    pub fn quad(&mut self, p: [Vec2; 4], c: Color) {
        self.push(p, [[0.0; 2]; 4], [c; 4], [1e6, 1e6, 0.0, 0.0], MODE_RECT);
    }

    pub fn solid(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        self.gradient(x, y, w, h, c, c);
    }

    /// Thin circular outline with a transparent center.
    pub fn ring(&mut self, center: Vec2, radius: f32, thickness: f32, color: Color) {
        let extent = radius + thickness * 0.5 + 1.5;
        let p = [
            center + Vec2::new(-extent, -extent),
            center + Vec2::new(extent, -extent),
            center + Vec2::new(extent, extent),
            center + Vec2::new(-extent, extent),
        ];
        let uv = p.map(|point| (point - center).to_array());
        self.push(p, uv, [color; 4], [radius, thickness, 0.0, 0.0], MODE_RING);
    }

    pub fn vignette(&mut self, c: Color) {
        let (w, h) = (self.w, self.h);
        let p = [
            Vec2::new(0.0, 0.0),
            Vec2::new(w, 0.0),
            Vec2::new(w, h),
            Vec2::new(0.0, h),
        ];
        let uv = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        self.push(p, uv, [c; 4], [0.0; 4], MODE_VIGNETTE);
    }

    fn block_face(&mut self, p: [Vec2; 4], layer: u32, shade: f32, tint: [u8; 3]) {
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let c = [
            tint[0] as f32 / 255.0 * shade,
            tint[1] as f32 / 255.0 * shade,
            tint[2] as f32 / 255.0 * shade,
            1.0,
        ];
        self.push(p, uv, [c; 4], [layer as f32, shade, 0.0, 0.0], MODE_BLOCK);
    }

    /// Flat textured sprite (plants).
    pub fn block_sprite(&mut self, c: Vec2, r: f32, layer: u32, tint: [u8; 3]) {
        let p = [
            Vec2::new(c.x - r, c.y - r),
            Vec2::new(c.x + r, c.y - r),
            Vec2::new(c.x + r, c.y + r),
            Vec2::new(c.x - r, c.y + r),
        ];
        self.block_face(p, layer, 1.0, tint);
    }

    /// Isometric icon with different textures on the left (front) and right faces.
    #[allow(clippy::too_many_arguments)]
    pub fn block_icon_faces(
        &mut self,
        c: Vec2,
        r: f32,
        top: u32,
        left: u32,
        right: u32,
        top_tint: [u8; 3],
        side_tint: [u8; 3],
    ) {
        let k = 0.866 * r;
        let top_face = [
            Vec2::new(c.x, c.y - r),
            Vec2::new(c.x + k, c.y - r * 0.5),
            Vec2::new(c.x, c.y),
            Vec2::new(c.x - k, c.y - r * 0.5),
        ];
        let lf = [
            Vec2::new(c.x - k, c.y - r * 0.5),
            Vec2::new(c.x, c.y),
            Vec2::new(c.x, c.y + r),
            Vec2::new(c.x - k, c.y + r * 0.5),
        ];
        let rf = [
            Vec2::new(c.x, c.y),
            Vec2::new(c.x + k, c.y - r * 0.5),
            Vec2::new(c.x + k, c.y + r * 0.5),
            Vec2::new(c.x, c.y + r),
        ];
        self.block_face(top_face, top, 1.0, top_tint);
        self.block_face(lf, left, 0.8, side_tint);
        self.block_face(rf, right, 0.62, side_tint);
    }

    /// Textured quad (corners TL, TR, BR, BL of the texture) with a brightness factor.
    pub fn tex_quad(&mut self, p: [Vec2; 4], layer: u32, shade: f32) {
        self.block_face(p, layer, shade, [255; 3]);
    }

    /// `tex_quad` showing the part `uv` (per corner) of the texture.
    pub fn tex_quad_uv(&mut self, p: [Vec2; 4], uv: [[f32; 2]; 4], layer: u32, shade: f32) {
        let c = [shade, shade, shade, 1.0];
        self.push(p, uv, [c; 4], [layer as f32, shade, 0.0, 0.0], MODE_BLOCK);
    }

    /// `tex_quad` with a color tint.
    pub fn tex_quad_tint(&mut self, p: [Vec2; 4], layer: u32, shade: f32, tint: [u8; 3]) {
        self.block_face(p, layer, shade, tint);
    }

    // ---------- text ----------

    pub fn text_width(&self, s: &str, size: f32) -> f32 {
        self.font.text_width(s, size)
    }

    fn draw_text(
        &mut self,
        s: &str,
        x: f32,
        y: f32,
        size: f32,
        c: Color,
        xf: Option<(Vec2, Vec2)>,
    ) {
        let mut pen = x;
        for ch in s.chars() {
            let g = *self.font.glyph(ch);
            if g.w > 0.0 {
                let [qx, qy, qw, qh] = g.quad;
                let (x0, y0) = (pen + qx * size, y + qy * size);
                let (x1, y1) = (x0 + qw * size, y0 + qh * size);
                let mut p = [
                    Vec2::new(x0, y0),
                    Vec2::new(x1, y0),
                    Vec2::new(x1, y1),
                    Vec2::new(x0, y1),
                ];
                if let Some((center, rot)) = xf {
                    for q in &mut p {
                        *q = center + rot.rotate(*q - center);
                    }
                }
                let uv = [[g.u0, g.v0], [g.u1, g.v0], [g.u1, g.v1], [g.u0, g.v1]];
                self.push(p, uv, [c; 4], [0.0; 4], MODE_TEXT);
            }
            pen += g.adv * size;
        }
    }

    /// A soft shadow under text (not the hard, full-strength block shadow).
    fn shadow_color(c: Color) -> Color {
        [c[0] * 0.08, c[1] * 0.08, c[2] * 0.1, c[3] * 0.5]
    }

    /// Draws text with its top-left at (x, y). Returns the width.
    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, c: Color, shadow: bool) -> f32 {
        let (x, y) = (x.round(), y.round());
        if shadow {
            let d = (size * 0.5).max(1.0);
            self.draw_text(s, x + d, y + d, size, Self::shadow_color(c), None);
        }
        self.draw_text(s, x, y, size, c, None);
        self.text_width(s, size)
    }

    /// Splits `text` into lines no wider than `max_w` pixels at `size` (at spaces).
    pub fn wrap(&self, text: &str, max_w: f32, size: f32) -> Vec<String> {
        self.font.wrap(text, max_w, size)
    }

    pub fn text_centered(&mut self, s: &str, cx: f32, y: f32, size: f32, c: Color, shadow: bool) {
        let w = self.text_width(s, size);
        self.text(s, cx - w * 0.5, y, size, c, shadow);
    }
}
