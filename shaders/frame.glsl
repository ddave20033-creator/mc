// Per-frame uniform data shared by the world, shadow and sky shaders.
layout(set = 0, binding = 2) uniform FrameData {
    mat4 viewProj;
    mat4 invViewProj;
    mat4 lightViewProj;
    vec4 camPos;    // xyz camera position, w = time in seconds
    vec4 sunDir;    // xyz direction towards the sun, w = daylight factor
    vec4 lightDir;  // xyz direction towards the active light (sun or moon), w = anti-aliasing on
    vec4 sunColor;  // rgb direct light color (already scaled by intensity)
    vec4 ambient;   // rgb sky ambient, w = how exposed the camera is to the sky
    vec4 fog;       // x start, y end, z underwater, w shadows enabled
    vec4 misc;      // x clouds enabled, y exposure, z shadow texel size, w cloud time
    // Torches and lanterns held by players (this one and the others on the LAN):
    // xyz position, w = intensity (zero for unused slots).
    vec4 heldLights[8];
    // Weapon lights: pairs of (xyz position, w on) and (xyz direction, w cosine of the cone's
    // edge).
    vec4 spots[8];
    vec4 detail;    // x: pixels a block at distance 1 covers (field of view setting and zoom)
} frame;

layout(push_constant) uniform Push {
    mat4 viewProj;
    vec4 params;    // x: 0 opaque, 1 translucent, 2 view model, 3 shadow, 4 view model glass
} pc;
