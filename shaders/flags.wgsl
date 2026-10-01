// What a vertex is (its tint's alpha: world::mesh::flags), and the detail far blocks lose, in
// pixels per block (frame.detail.x over the distance; render::cull leaves out the same). One
// copy for all the shaders; render::frame's test checks they are the game's.
const F_LEAVES: i32 = 1;
const F_PLANT: i32 = 2;
const F_EMISSIVE: i32 = 4;
const F_WATER: i32 = 8;
const F_OVERLAY: i32 = 16;
const F_VIEWMODEL: i32 = 32;
const F_ENTITY: i32 = 64;
const F_FLUID: i32 = 128;
// Grass and flowers fade out between these (world.wgsl's fragment shader; its vertex shader
// drops them below).
const PLANT_FULL_PX: f32 = 10.0;
const PLANT_GONE_PX: f32 = 5.0;
