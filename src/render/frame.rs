//! What the game hands the renderer each frame: the uniform data the shaders read, the
//! geometry to draw and the scope's view, with the sizes and limits both sides share.

use crate::ui::UiVertex;
use crate::world::mesh::Vertex;
use glam::{Mat4, Vec3};

pub const SHADOW_SIZE: u32 = 4096;
/// The scope's view: its size in pixels (square) and format.
pub const SCOPE_SIZE: u32 = 512;

/// Weapon lights at once (`FrameUbo::spots`).
pub const MAX_SPOTS: usize = 4;
/// Each weapon light's shadow map: a square this big in a strip under the sun's in the same
/// depth image (as frame.glsl's `SHADOW_SPOT_*` reads them), one beside the other.
pub const SPOT_SHADOW: u32 = 1024;
/// The shadow depth image's height: the sun's square, and the weapon lights' strip under it.
pub const SHADOW_HEIGHT: u32 = SHADOW_SIZE + SPOT_SHADOW;
/// How far a weapon light reaches (blocks): its shadow map's far end.
pub const SPOT_REACH: f32 = 30.0;

/// Must match `heldLights` in frame.glsl.
pub const MAX_HELD_LIGHTS: usize = 8;

/// Mirrors `FrameData` in shaders/frame.glsl (std140, all vec4/mat4).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameUbo {
    pub view_proj: [f32; 16],
    pub inv_view_proj: [f32; 16],
    pub light_view_proj: [f32; 16],
    pub cam_pos: [f32; 4],
    pub sun_dir: [f32; 4],
    pub light_dir: [f32; 4],
    pub sun_color: [f32; 4],
    pub ambient: [f32; 4],
    pub fog: [f32; 4],
    pub misc: [f32; 4],
    /// Held torches and lanterns (this player's and the other LAN players'): position, intensity.
    pub held_lights: [[f32; 4]; MAX_HELD_LIGHTS],
    /// Weapon lights: pairs of (xyz position, w on) and (xyz direction, w the cosine of the
    /// cone's edge). Must match `spots` in frame.glsl.
    pub spots: [[f32; 4]; 2 * MAX_SPOTS],
    /// x: how many pixels a block at distance 1 covers (detail too small for the screen is
    /// simplified by it).
    pub detail: [f32; 4],
    /// Each weapon light's view (its shadow map's): what it lights, from where it is.
    pub spot_view_proj: [[f32; 16]; MAX_SPOTS],
}

impl Default for FrameUbo {
    fn default() -> Self {
        // (all zero: every field is floats)
        unsafe { std::mem::zeroed() }
    }
}

pub struct FrameInfo<'a> {
    pub ubo: FrameUbo,
    pub view_proj: Mat4,
    pub vm_view_proj: Mat4,
    pub light_view_proj: Mat4,
    pub cam_pos: Vec3,
    pub view_distance: f32,
    pub shadows: bool,
    pub shadow_distance: f32,
    /// How many pixels a block at distance 1 covers: far chunks leave out what is too small.
    pub detail_px: f32,
    pub ui: &'a [UiVertex],
    /// Scissor regions of the UI vertices: (first vertex, clip rectangle or None).
    pub ui_clips: &'a [(u32, Option<[f32; 4]>)],
    /// Box around the targeted block (world corners).
    pub outline: Option<(Vec3, Vec3)>,
    pub particles: &'a [Vertex],
    pub overlay: &'a [Vertex],
    pub viewmodel: &'a [Vertex],
    /// Player model: always casts shadows, drawn only when `entity_visible` (third person).
    pub entity: &'a [Vertex],
    pub entity_visible: bool,
    /// The first vertices in `entity` are this player's model.
    pub player_vertex_count: u32,
    pub player_opacity: f32,
    pub translucent: &'a [Vertex],
    /// The first-person gun's see-through glass (drawn blended after it), and its scope's
    /// eyepiece, which shows `scope`'s view.
    pub viewmodel_glass: &'a [Vertex],
    pub lens: &'a [Vertex],
    pub scope: Option<ScopeView>,
    /// A menu is open: `scope` is the plain view, and `lens` a quad over the whole screen
    /// (clip space) showing it blurred behind the menu.
    pub backdrop_blur: bool,
}

/// The view through the scope (magnified): rendered into the scope image before the main
/// pass, then shown on the eyepiece (`FrameInfo::lens`).
pub struct ScopeView {
    pub ubo: FrameUbo,
    pub view_proj: Mat4,
    pub cam_pos: Vec3,
    /// Pixels a block at distance 1 covers in the scope's image.
    pub detail_px: f32,
}
