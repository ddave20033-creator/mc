// Per-frame uniform data shared by the world, shadow and sky shaders (render::FrameUbo), and
// the world pipelines' immediates (render::pipelines::DrawPush).
struct FrameData {
    viewProj: mat4x4f,
    invViewProj: mat4x4f,
    lightViewProj: mat4x4f,
    camPos: vec4f,    // xyz camera position, w = time in seconds
    sunDir: vec4f,    // xyz direction towards the sun, w = daylight factor
    lightDir: vec4f,  // xyz direction towards the active light (sun or moon), w = anti-aliasing on
    sunColor: vec4f,  // rgb direct light color (already scaled by intensity)
    ambient: vec4f,   // rgb sky ambient, w = how exposed the camera is to the sky
    fog: vec4f,       // x start, y end, z underwater, w shadows enabled
    misc: vec4f,      // x clouds enabled, y exposure, z shadow texel size, w cloud time
    // Torches and lanterns held by players (this one and the others on the LAN):
    // xyz position, w = intensity (zero for unused slots).
    heldLights: array<vec4f, 8>,
    // Weapon lights: pairs of (xyz position, w on) and (xyz direction, w cosine of the cone's
    // edge).
    spots: array<vec4f, 8>,
    detail: vec4f,    // x: pixels a block at distance 1 covers (field of view setting and zoom)
    // Each weapon light's view (its shadow map's).
    spotViewProj: array<mat4x4f, 4>,
}

@group(0) @binding(4) var<uniform> frame: FrameData;

// The shadow depth image: the sun's square on top, the weapon lights' squares in a strip under
// it, side by side (render/frame.rs: SHADOW_SIZE, SPOT_SHADOW).
const SHADOW_SUN_V: f32 = 4096.0 / 5120.0;
const SHADOW_SPOT_U: f32 = 1024.0 / 4096.0;
const SHADOW_SPOT_V: f32 = 1024.0 / 5120.0;

struct Push {
    viewProj: mat4x4f,
    // x: 0 opaque, 1 translucent, 2 view model, 3 the sun's shadow map, 4 view model glass,
    // 5 a weapon light's shadow map; y: the local player's opacity when it fades
    params: vec4f,
}

var<immediate> pc: Push;
