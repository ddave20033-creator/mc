// The world: blocks, fluids, entities, particles, the hand. The vertex shader reads the full
// vertex, or with CHUNK a chunk mesh's packed one (vertex.wgsl); the fragment shader is made
// a second time with NO_DISCARD for the plain faces of whole blocks (render::pipelines),
// whose texels are all opaque so no test here would cut anything: without any `discard` the
// depth test runs before it.
#include "common.wgsl"
#include "frame.wgsl"
#include "wave.wgsl"
#include "vertex.wgsl"
#include "fire.wgsl"
#include "blocks.wgsl"

@group(0) @binding(0) var blocks: texture_2d_array<f32>;
@group(0) @binding(1) var blockSampler: sampler;
@group(0) @binding(2) var shadowMap: texture_depth_2d;
@group(0) @binding(3) var shadowSampler: sampler_comparison;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) vUV: vec2f,
    @location(1) @interpolate(flat) vLayer: f32,
    // (ao, sky, block, normal index)
    @location(2) vLight: vec4f,
    @location(3) vTint: vec3f,
    @location(4) vWorld: vec3f,
    @location(5) @interpolate(flat) vFlags: i32,
    @location(6) @interpolate(flat) vNormal: i32,
    @location(7) vSmoothN: vec3f,
}

@vertex
fn vs_main(i: VertexInput) -> VertexOutput {
    let v = readVertex(i);
    let flags = i32(i.tint.a * 255.0 + 0.5);
    let p = displace(v.pos, v.uv, flags, frame.camPos.w, v.layer, i.tint.rg);
    var clip = pc.viewProj * vec4f(p, 1.0);
    // Grass and flowers smaller on screen than where the fragment shader has faded them out:
    // dropped (the whole quad, judged by its block's middle, lands outside the view; a little
    // under PLANT_GONE_PX, as the middle is farther than the nearest texel).
    let blockPx = frame.detail.x / max(length(floor(v.pos.xz) + 0.5 - frame.camPos.xz), 1e-3);
    if ((flags & F_PLANT) != 0 && blockPx < PLANT_GONE_PX * 0.94) {
        clip = vec4f(0.0, 0.0, 2.0, 1.0);
    }
    var o: VertexOutput;
    o.pos = toClip(clip);
    let normal = i32(i.light.w * 255.0 + 0.5);
    o.vUV = v.uv;
    if ((flags & F_FLUID) != 0) {
        // Fluid texture coordinates come from the world position (uv holds animation data).
        if (normal == 2 || normal == 3) {
            o.vUV = p.xz;
        } else if (normal < 2) {
            o.vUV = vec2f(p.z, -p.y);
        } else {
            o.vUV = vec2f(p.x, -p.y);
        }
    }
    o.vLayer = v.layer;
    o.vLight = i.light;
    o.vTint = i.tint.rgb;
    o.vWorld = p;
    o.vFlags = flags;
    o.vNormal = normal;
    o.vSmoothN = vec3f(0.0, 1.0, 0.0);
    if (normal >= 16) {
        // A round surface (a log): its normal turns smoothly round the axis, given at each
        // corner as 16 + axis * 64 + angle (64 steps round), and lit as such (normal 7).
        let axis = (normal - 16) / 64;
        let a = f32((normal - 16) % 64) / 64.0 * 6.2831853;
        let cs = vec2f(cos(a), sin(a));
        if (axis == 0) {
            o.vSmoothN = vec3f(0.0, cs.y, cs.x);
        } else if (axis == 1) {
            o.vSmoothN = vec3f(cs.x, 0.0, cs.y);
        } else {
            o.vSmoothN = vec3f(cs.x, cs.y, 0.0);
        }
        o.vNormal = 7;
    }
    return o;
}

const LAVA_LAYER: f32 = 38.0;
const GRASS_SIDE_LAYER: f32 = 1.0;
const GRASS_TOP_LAYER: f32 = 0.0;
const SNOW_LAYER: f32 = 9.0;
const SNOWY_GRASS_SIDE_LAYER: f32 = 16.0;

// Where a block covers only a few pixels on this screen, its fine detail cannot show and just
// flickers as the view moves. Measured in pixels per block (the screen's height, field of view
// and zoom decide it, so a sharper screen keeps detail farther out): below FULL_DETAIL_PX the
// textures melt into their average color, fully at AVERAGE_PX (and grass and flowers fade
// out: PLANT_FULL_PX, PLANT_GONE_PX in flags.wgsl).
const FULL_DETAIL_PX: f32 = 4.0;
const AVERAGE_PX: f32 = 1.0;

// Ordered 4x4 dither threshold: fading without alpha blending.
fn bayer4(p: vec2f) -> f32 {
    let i = vec2i(p) & vec2i(3);
    var m = array<i32, 16>(0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
    return (f32(m[i.y * 4 + i.x]) + 0.5) / 16.0;
}
const FURNACE_LIT_LAYER: f32 = 63.0;

const TORCH_FLAME_LAYER: f32 = 113.0;
const GLASS_LAYER: f32 = 13.0;
const FURNACE_ANIM_LAYER: f32 = 117.0;
const FURNACE_FRAMES: f32 = 12.0;
// Water and lava animation frames (tex::WATER_ANIM, tex::LAVA_ANIM).
const WATER_ANIM_LAYER: f32 = 185.0;
const LAVA_ANIM_LAYER: f32 = 217.0;
const FLUID_FRAMES: f32 = 32.0;

fn faceNormal(i: i32) -> vec3f {
    var n = array<vec3f, 7>(
        vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), vec3f(0.0, -1.0, 0.0),
        vec3f(0.0, 0.0, 1.0), vec3f(0.0, 0.0, -1.0), vec3f(0.0, 1.0, 0.0)
    );
    return n[i];
}

// Texture axes of the faces (+u, and the direction texture v grows), as the mesher's FACE_U
// and -FACE_V.
fn faceTU(i: i32) -> vec3f {
    var t = array<vec3f, 6>(
        vec3f(0.0, 0.0, -1.0), vec3f(0.0, 0.0, 1.0), vec3f(1.0, 0.0, 0.0), vec3f(1.0, 0.0, 0.0),
        vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0)
    );
    return t[i];
}
fn faceTV(i: i32) -> vec3f {
    var t = array<vec3f, 6>(
        vec3f(0.0, -1.0, 0.0), vec3f(0.0, -1.0, 0.0), vec3f(0.0, 0.0, 1.0), vec3f(0.0, 0.0, -1.0),
        vec3f(0.0, -1.0, 0.0), vec3f(0.0, -1.0, 0.0)
    );
    return t[i];
}
// Ore nuggets are set into their block this deep (blocks = texture units).
const PIT_DEPTH: f32 = 1.0 / 32.0;
const PIT_STEPS: i32 = 24;
// Floor and bottom of the walls of a pit this much darker.
const PIT_SHADE: f32 = 0.68;

// A texel's surface material, from its alpha at full size (textures/mod.rs `Material`):
// alpha = 255 - code, code = pit | metal << 1 | shine << 2. Plain textures (alpha 255) give 0.
fn materialCode(uv: vec2f, layer: f32) -> i32 {
    let size = vec2i(textureDimensions(blocks));
    let t = vec2i(fract(uv) * vec2f(size)) % size;
    let a = textureLoad(blocks, t, i32(layer + 0.5), 0).a;
    return select(0, i32(round((1.0 - a) * 255.0)), a > 0.74);
}

fn isPit(uv: vec2f, layer: f32) -> bool {
    return (materialCode(uv, layer) & 1) != 0;
}

// Blinn-Phong highlight of a light from direction l.
fn highlight(n: vec3f, l: vec3f, v: vec3f, power: f32) -> f32 {
    return pow(max(dot(n, normalize(l + v)), 0.0), power) * step(0.0, dot(n, l));
}

// Minecraft-like light falloff: 15 -> 1.0, 8 -> ~0.2, 0 -> 0.
fn lightCurve(l: f32) -> f32 {
    return l / (4.0 - 3.0 * l);
}

// Classic per-face brightness: top 1.0, north/south 0.8, east/west 0.6, bottom 0.5.
fn faceShade(n: vec3f) -> f32 {
    let n2 = n * n;
    return n2.x * 0.6 + n2.z * 0.8 + n2.y * select(0.5, 1.0, n.y > 0.0);
}

fn shadowAt(p: vec3f, n: vec3f) -> f32 {
    if (frame.fog.w < 0.5) {
        return 1.0;
    }
    let lp = frame.lightViewProj * vec4f(p + n * 0.06, 1.0);
    let c = lp.xyz / lp.w;
    var uv = c.xy * 0.5 + 0.5;
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || c.z >= 1.0) {
        return 1.0;
    }
    let t = frame.misc.z;
    // (the sun's square is the top of the shadow image)
    uv.y *= SHADOW_SUN_V;
    // 4x4 grid of bilinear-filtered comparisons: soft, stable edges.
    var s = 0.0;
    for (var x = 0; x < 4; x++) {
        for (var y = 0; y < 4; y++) {
            let o = (vec2f(f32(x), f32(y)) - 1.5) * t * vec2f(1.0, SHADOW_SUN_V);
            s += textureSampleCompareLevel(shadowMap, shadowSampler, uv + o, c.z - 0.0003);
        }
    }
    s /= 16.0;
    let edge = max(abs(c.x), abs(c.y));
    return mix(s, 1.0, smoothstep(0.8, 1.0, edge));
}

// How much of weapon light `i`'s light gets to the point `p` (surface normal `n`): its shadow
// map, a 3x3 grid of filtered comparisons for a soft edge.
fn spotShadow(i: i32, p: vec3f, n: vec3f) -> f32 {
    let lp = frame.spotViewProj[i] * vec4f(p + n * 0.05, 1.0);
    if (lp.w <= 0.0) {
        return 0.0;
    }
    let c = lp.xyz / lp.w;
    var uv = c.xy * 0.5 + 0.5;
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || c.z >= 1.0) {
        return 1.0;
    }
    // (kept off the square's edge, so the filter does not reach the next one)
    uv = clamp(uv, vec2f(2.0 / 1024.0), vec2f(1.0 - 2.0 / 1024.0));
    let at = vec2f((f32(i) + uv.x) * SHADOW_SPOT_U, SHADOW_SUN_V + uv.y * SHADOW_SPOT_V);
    let texel = vec2f(SHADOW_SPOT_U, SHADOW_SPOT_V) / 1024.0;
    var s = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            s += textureSampleCompareLevel(shadowMap, shadowSampler, at + vec2f(f32(x), f32(y)) * texel, c.z - 0.00004);
        }
    }
    return s / 9.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let vLayer = in.vLayer;
    let vFlags = in.vFlags;
    let vNormal = in.vNormal;
    let vTint = in.vTint;
    let vWorld = in.vWorld;
    let vLight = in.vLight;
    let time = frame.camPos.w;
    // Block textures whose alpha may carry a material (not ones using alpha for other things).
    let materialLayer = abs(vLayer - GRASS_SIDE_LAYER) > 0.5 && abs(vLayer - SNOWY_GRASS_SIDE_LAYER) > 0.5
        && abs(vLayer - GLASS_LAYER) > 0.5 && abs(vLayer - FURNACE_LIT_LAYER) > 0.5;
    let water = (vFlags & F_WATER) != 0;
    let emissive = (vFlags & F_EMISSIVE) != 0;
    let vm = (vFlags & F_VIEWMODEL) != 0;
    let derivN = (vFlags & (F_VIEWMODEL | F_ENTITY)) != 0;
    let plant = (vFlags & F_PLANT) != 0;

    let fluid = (vFlags & F_FLUID) != 0;
    var uv = in.vUV;
    var flow = vec2f(0.0);
    if (fluid) {
        // Texture streams along the flow direction (downhill on top, downward on sides).
        let speed = select(0.3, 0.9, water);
        flow = vTint.rg * 2.0 - 1.0;
        let falling = vTint.b > 0.5;
        if (vNormal == 2) {
            if (length(flow) > 0.05) {
                uv -= flow * time * speed;
            } else {
                uv += vec2f(time * 0.03, time * 0.045) * speed;
            }
        } else if (vNormal != 3) {
            uv.y += time * speed * select(0.6, 1.6, falling);
        }
    }
    // The derivatives everything below needs, taken here: WGSL allows them only where every
    // pixel of the quad still runs (before any discard or return). (For all but the fluids
    // `uv` is still the face's own, so the mip level does not jump at the pits' edges.)
    let dUVdx = dpdx(uv);
    let dUVdy = dpdy(uv);
    let dWorldX = dpdx(vWorld);
    let dWorldY = dpdy(vWorld);

    let torchFire = abs(vLayer - TORCH_FLAME_LAYER) < 0.5;
    var tex: vec4f;
    if (abs(vLayer - GLASS_LAYER) < 0.5) {
        // Joined glass: near a joined edge, read the texture from farther in. Far away the
        // smaller mip levels would otherwise smear the frame into the pane and the panes
        // would look apart again.
        let joined = 255 - i32(vTint.r * 255.0 + 0.5);
        let lodLevel = max(blockLod(blocks, dUVdx, dUVdy), 0.0);
        let edge = 7.0 / 128.0 + exp2(lodLevel) * 1.5 / 128.0;
        var suv = uv;
        if ((joined & 1) != 0) {
            suv.x = max(suv.x, edge);
        }
        if ((joined & 2) != 0) {
            suv.x = min(suv.x, 1.0 - edge);
        }
        if ((joined & 4) != 0) {
            suv.y = max(suv.y, edge);
        }
        if ((joined & 8) != 0) {
            suv.y = min(suv.y, 1.0 - edge);
        }
        tex = sampleBlocks(blocks, blockSampler, suv, vLayer, dUVdx, dUVdy);
    } else if (fluid) {
        // Animated like Minecraft's still water (10 frames a second) and lava (slower).
        let lava = abs(vLayer - LAVA_LAYER) < 0.5;
        let fps = select(10.0, 6.67, lava);
        let animFrame = gmod(floor(time * fps), FLUID_FRAMES);
        tex = sampleBlocks(blocks, blockSampler, uv, select(WATER_ANIM_LAYER, LAVA_ANIM_LAYER, lava) + animFrame, dUVdx, dUVdy);
    } else {
        // Sunk ore nuggets (parallax): through a pit texel the eye ray goes on down into the
        // block until it meets a pit wall (the stone texel there) or the pit's floor.
        var pitShade = 1.0;
        let faceUV = vNormal < 6 && (vFlags & (F_VIEWMODEL | F_ENTITY | F_PLANT | F_LEAVES)) == 0
            && materialLayer;
        if (faceUV && isPit(uv, vLayer)) {
            let toEye = frame.camPos.xyz - vWorld;
            let px = frame.detail.x / max(length(toEye), 1e-3);
            // Flat far away (the pits only darker), deep up close.
            let depth = PIT_DEPTH * smoothstep(12.0, 32.0, px);
            var d = 1.0;
            if (depth > 0.0) {
                let e = normalize(toEye);
                let up = max(dot(e, faceNormal(vNormal)), 0.2);
                let dir = -vec2f(dot(e, faceTU(vNormal)), dot(e, faceTV(vNormal))) / up * depth;
                let start = uv;
                for (var i = 1; i <= PIT_STEPS; i++) {
                    let k = f32(i) / f32(PIT_STEPS);
                    if (!isPit(start + dir * k, vLayer)) {
                        d = k;
                        break;
                    }
                }
                uv = start + dir * d;
            }
            pitShade = mix(1.0, PIT_SHADE, d);
        }
        tex = sampleBlocks(blocks, blockSampler, uv, vLayer, dUVdx, dUVdy);
        tex = vec4f(tex.rgb * pitShade, tex.a);
    }

    // Detail too small for the screen: the texture turns into its average color (its smallest
    // mip level); a grass-block side into the grass top's, so the green and brown stripes of far
    // hillsides stop flickering; leaf gaps close.
    let camDist = distance(frame.camPos.xyz, vWorld);
    let blockPx = frame.detail.x / max(camDist, 1e-3);
    let leaves = (vFlags & F_LEAVES) != 0;
    let entity = (vFlags & (F_ENTITY | F_VIEWMODEL)) != 0;
    var far = 1.0 - smoothstep(AVERAGE_PX, FULL_DETAIL_PX, blockPx);
    if (water || fluid || entity) {
        far = 0.0;
    }
    if (far > 0.0) {
        var avgLayer = vLayer;
        let grassSide = abs(vLayer - GRASS_SIDE_LAYER) < 0.5;
        if (grassSide) {
            avgLayer = GRASS_TOP_LAYER;
        }
        if (abs(vLayer - SNOWY_GRASS_SIDE_LAYER) < 0.5) {
            avgLayer = SNOW_LAYER;
        }
        let top = f32(textureNumLevels(blocks) - 1u);
        let avg = textureSampleLevel(blocks, blockSampler, vec2f(0.5, 0.5), i32(floor(avgLayer + 0.5)), top);
        tex = vec4f(mix(tex.rgb, avg.rgb, far), tex.a);
        if (grassSide || leaves) {
            tex.a = mix(tex.a, 1.0, far);
        }
    }
    var plantFade = 1.0;
    if (plant && !entity) {
        plantFade = smoothstep(PLANT_GONE_PX, PLANT_FULL_PX, blockPx);
    }

    let furnaceLit = abs(vLayer - FURNACE_LIT_LAYER) < 0.5;
    if (furnaceLit && tex.a > 0.9) {
        // A resource pack's lit furnace (full alpha; built-in opaque textures use 0.6) plays
        // animation frames made from its own art, at 8 fps with a phase per furnace.
        let animFrame = gmod(floor(time * 8.0 + vTint.r * FURNACE_FRAMES), FURNACE_FRAMES);
        tex = sampleBlocks(blocks, blockSampler, uv, FURNACE_ANIM_LAYER + animFrame, dUVdx, dUVdy);
    }
    // The built-in lit furnace gets procedural fire in its opening instead.
    let furnaceFire = furnaceLit && tex.a < 0.9;
    // (the last derivative: of the alpha the cut-out edges are smoothed by, below)
    let alphaWidth = fwidth(tex.a);

    // Selection outline
    if (vLayer < 0.0) {
        return vec4f(0.0, 0.0, 0.0, 0.6);
    }

    if ((vFlags & F_OVERLAY) != 0) {
        // Multiply-blended (result = 2 * src * dst): mid-gray leaves the block unchanged, darker
        // pixels darken it, lighter ones brighten it. Minecraft does this in gamma space, so the
        // factor is converted to linear to match.
#ifndef NO_DISCARD
        if (tex.a < 0.1) {
            discard;
        }
#endif
        let factor = pow(2.0 * mix(vec3f(0.5), pow(tex.rgb, vec3f(1.0 / 2.2)), tex.a), vec3f(2.2));
        return vec4f(factor * 0.5, 1.0);
    }
    // Grass and leaves with anti-aliasing on (alpha to coverage): the cut-out edge covers only
    // some of a pixel's samples. Far away the filtered alpha is soft; sharpened to about a
    // pixel it smooths the edges that would otherwise flicker as the view moves.
    var coverage = 1.0;
    let cutout = (vFlags & (F_LEAVES | F_PLANT)) != 0;
#ifndef NO_DISCARD
    if (cutout && frame.lightDir.w > 0.5) {
        coverage = clamp((tex.a - 0.5) / max(alphaWidth, 1e-4) + 0.5, 0.0, 1.0);
        coverage *= plantFade;
        if (coverage <= 0.0) {
            discard;
        }
    } else if (!water && tex.a < select(0.5, 0.05, pc.params.x == 1.0 || pc.params.x == 4.0)) {
        // (Blended, glass shows as see-through as its texture; elsewhere it is cut out.)
        discard;
    } else if (plantFade < bayer4(in.pos.xy)) {
        discard;
    }
#else
    if (cutout && frame.lightDir.w > 0.5) {
        coverage = clamp((tex.a - 0.5) / max(alphaWidth, 1e-4) + 0.5, 0.0, 1.0);
        coverage *= plantFade;
    }
#endif
    // Glass faces carry their connection mask inverted in the red tint channel.
    let glass = abs(vLayer - GLASS_LAYER) < 0.5;
#ifndef NO_DISCARD
    if (glass && glassSeam(uv, 255 - i32(vTint.r * 255.0 + 0.5))) {
        discard;
    }
#endif

    var flame = vec4f(0.0);
    if (torchFire || furnaceFire) {
        // The mesh stores a single phase on all vertices of each flame.
        // Rounding interpolated world positions at a block edge caused speckles.
        let seed = vTint.r;
        if (torchFire) {
            flame = torchFlame(uv, time, seed);
#ifndef NO_DISCARD
            if (flame.a < 0.02) {
                discard;
            }
#endif
        } else {
            let opening = (uv - vec2f(32.0, 66.0) / 128.0) / (vec2f(64.0, 46.0) / 128.0);
            if (all(opening >= vec2f(0.0)) && all(opening <= vec2f(1.0))) {
                flame = furnaceFlame(opening, time, seed);
            }
        }
    }

    // Biome tint. The grass-block side uses its alpha channel as a tint mask (grass vs dirt).
    var tint = pow(vTint, vec3f(2.2));
    if (fluid || furnaceLit || glass) {
        tint = vec3f(1.0);
    }
    var mask = 1.0;
    if (abs(vLayer - GRASS_SIDE_LAYER) < 0.5) {
        mask = clamp((tex.a - 0.6) / 0.4, 0.0, 1.0);
    }
    let albedo = tex.rgb * mix(vec3f(1.0), tint, mask);

    var N = faceNormal(clamp(vNormal, 0, 6));
    if (derivN) {
        N = normalize(cross(dWorldX, dWorldY));
        if (dot(N, frame.camPos.xyz - vWorld) < 0.0) {
            N = -N;
        }
    }
    // A round surface: its own smooth normal (7, see the vertex shader).
    if (vNormal == 7) {
        N = normalize(in.vSmoothN);
    }

    // Blocks only a few pixels big: their top and side faces lit much alike (and no corner
    // shadows), so far terraced hillsides do not turn into flickering light and dark stripes.
    // Only past the shadows' reach (about 96 blocks): a face turned away from the sun given
    // sunlight would catch flickering specks of it from the shadow map.
    let even = far * 0.45 * smoothstep(80.0, 100.0, camDist);
    let NL = normalize(mix(N, vec3f(0.0, 1.0, 0.0), even));
    let ao = mix(mix(0.45, 1.0, vLight.x), 1.0, far);
    let sky = lightCurve(vLight.y);
    let blk = lightCurve(vLight.z);
    let L = frame.lightDir.xyz;
    let ndl = select(dot(NL, L), 0.8, plant);
    let exposure = smoothstep(0.45, 0.85, vLight.y);
    var vis = 0.0;
    if (ndl > 0.0 && exposure > 0.0) {
        vis = shadowAt(vWorld, select(N, vec3f(0.0, 1.0, 0.0), plant)) * smoothstep(0.0, 0.25, ndl) * exposure;
    }

    // Sky light with classic face shading, modulated by sun visibility:
    // shaded areas turn slightly cooler and darker, sunlit ones slightly warmer.
    let shade = select(faceShade(NL), 0.9, plant);
    let strength = frame.sunColor.w;
    let sunMod = mix(vec3f(1.0), mix(vec3f(0.7, 0.74, 0.84), frame.sunColor.rgb * 1.12, vis), strength);
    let skyLight = frame.ambient.rgb * sky * shade * sunMod;
    let torch = vec3f(1.0, 0.72, 0.42) * blk * shade * 1.2;
    var light = max(skyLight, torch) + min(skyLight, torch) * 0.3;
    for (var i = 0; i < 8; i++) {
        let held = frame.heldLights[i];
        if (held.w <= 0.0) {
            continue;
        }
        let falloff = max(0.0, 1.0 - distance(vWorld, held.xyz) / 7.0);
        light += vec3f(1.0, 0.65, 0.34) * falloff * falloff * 1.8 * held.w;
    }
    // Weapon lights: a cool white cone, bright in its middle, reaching far; lighting what
    // faces it.
    for (var i = 0; i < 4; i++) {
        let sp = frame.spots[2 * i];
        if (sp.w <= 0.0) {
            continue;
        }
        let sd = frame.spots[2 * i + 1];
        var Ls = vWorld - sp.xyz;
        let d = length(Ls);
        Ls /= max(d, 1e-4);
        let along = dot(Ls, sd.xyz);
        let cone = smoothstep(sd.w, mix(sd.w, 1.0, 0.6), along);
        let hot = smoothstep(mix(sd.w, 1.0, 0.75), 1.0, along) * 0.6;
        let fall = max(0.0, 1.0 - d / 26.0);
        let facing = select(max(dot(N, -Ls), 0.0) * 0.85 + 0.15, 0.8, plant);
        // Not through blocks (glass lets it through): its shadow map.
        let seen = spotShadow(i, vWorld, N);
        light += vec3f(0.95, 0.97, 1.0) * (cone + hot) * fall * fall * facing * 2.4 * sp.w * seen;
    }
    var col = albedo * (light * ao + vec3f(0.02));

    // Shine: highlights of the sun or moon, held lights and weapon lights on shiny texels,
    // a soft sheen near torches, and a little of the sky mirrored. Metal takes its own colour,
    // gems and polished stone white. Up close a few texels of a shiny surface are tilted
    // facets that flash as the eye moves past the right angle.
    var mat = 0;
    if (materialLayer && !derivN && !fluid && !water && !emissive && (vFlags & F_PLANT) == 0) {
        mat = materialCode(uv, vLayer);
    }
    let shine = f32(mat >> 2u) / 15.0;
    if (shine > 0.0) {
        let metal = (mat & 2) != 0;
        let V = normalize(frame.camPos.xyz - vWorld);
        let power = exp2(mix(4.0, 8.0, shine));
        let gain = mix(0.25, 1.6, shine * shine);
        // Facets: about one texel in sixteen, more on the shiniest surfaces.
        var Nf = N;
        var facet = 0.0;
        if (vNormal < 6) {
            let size = vec2f(textureDimensions(blocks));
            let t = floor(fract(uv) * size);
            let pick = hash12(t + vLayer * 17.0);
            if (pick > 1.0 - 0.07 * shine) {
                let h = vec2f(hash12(t.yx + 31.7 + vLayer), hash12(t + 7.3)) - 0.5;
                Nf = normalize(N + (faceTU(vNormal) * h.x + faceTV(vNormal) * h.y) * 0.9);
                facet = smoothstep(14.0, 32.0, blockPx);
            }
        }
        // A light's highlight: the surface's broad one, or a facet's sharp flash.
        let sun = highlight(N, L, V, power);
        let flash = facet * pow(max(dot(Nf, normalize(L + V)), 0.0), 900.0) * 6.0;
        var spec = frame.sunColor.rgb * strength * vis * (sun + flash);
        spec += vec3f(1.0, 0.72, 0.42) * blk * 0.2 * pow(max(dot(N, V), 0.0), power * 0.25);
        for (var i = 0; i < 8; i++) {
            let held = frame.heldLights[i];
            if (held.w <= 0.0) {
                continue;
            }
            let toL = held.xyz - vWorld;
            let falloff = max(0.0, 1.0 - length(toL) / 7.0);
            let l = normalize(toL);
            let f = facet * pow(max(dot(Nf, normalize(l + V)), 0.0), 900.0) * 6.0;
            spec += vec3f(1.0, 0.65, 0.34) * falloff * falloff * 1.8 * held.w * (highlight(N, l, V, power) + f);
        }
        for (var i = 0; i < 4; i++) {
            let sp = frame.spots[2 * i];
            if (sp.w <= 0.0) {
                continue;
            }
            let sd = frame.spots[2 * i + 1];
            var toL = sp.xyz - vWorld;
            let d = length(toL);
            toL /= max(d, 1e-4);
            let cone = smoothstep(sd.w, mix(sd.w, 1.0, 0.6), dot(-toL, sd.xyz));
            let fall = max(0.0, 1.0 - d / 26.0);
            let f = facet * pow(max(dot(Nf, normalize(toL + V)), 0.0), 900.0) * 6.0;
            spec += vec3f(0.95, 0.97, 1.0) * cone * fall * fall * 2.4 * sp.w * spotShadow(i, vWorld, N)
                * (highlight(N, toL, V, power) + f);
        }
        var R = reflect(-V, N);
        R.y = abs(R.y);
        let fres = pow(1.0 - max(dot(N, V), 0.0), 5.0);
        let env = skyColor(R, frame.sunDir.xyz) * sky * mix(select(0.02, 0.15, metal), 0.5, fres);
        var tintSpec = vec3f(1.0);
        if (metal) {
            tintSpec = albedo * 1.8 + 0.04;
        }
        col += tintSpec * (spec * gain + env * shine * ao) * (1.0 - far);
    }
    if (emissive) {
        col = albedo * 1.4;
    }
    if (torchFire) {
        col = flame.rgb * 1.25;
    }
    if (furnaceFire) {
        col = mix(col, flame.rgb * 0.88, flame.a);
    }

    let toCam = frame.camPos.xyz - vWorld;
    let dist = length(toCam);
    let V = toCam / max(dist, 1e-4);
    var alpha = coverage;
    if (torchFire) {
        alpha = flame.a;
    }
    // Glass (the scope's lenses) drawn blended (the translucent pass, 1, and the view model's
    // glass, 4): as see-through as its texture.
    if (!water && !torchFire && (pc.params.x == 1.0 || pc.params.x == 4.0)) {
        alpha *= tex.a;
    }

    if (water) {
        var n = N;
        if (vNormal == 2) {
            let q = vWorld.xz;
            let h1 = vnoise(q * 1.1 + vec2f(time * 0.5, time * 0.3));
            let h2 = vnoise(q * 2.3 - vec2f(time * 0.4, -time * 0.6));
            n = normalize(vec3f((h1 - 0.5) * 0.3 + (h2 - 0.5) * 0.18, 1.0, (h2 - 0.5) * 0.3 - (h1 - 0.5) * 0.12));
        }
        let cosv = abs(dot(n, V));
        let fres = 0.02 + 0.6 * pow(1.0 - cosv, 5.0);
        var R = reflect(-V, n);
        R.y = abs(R.y);
        let refl = skyColor(R, frame.sunDir.xyz) * mix(0.12, 0.8, sky);
        var base = tex.rgb * vec3f(0.1, 0.27, 0.6) * (light + 0.02);
        // Soft foam streaks on moving water, carried along by the scrolled texture coordinates.
        var moving = select(0.4, 1.0, vTint.b > 0.5);
        if (vNormal == 2) {
            moving = clamp(length(flow), 0.0, 1.0);
        }
        let foam = smoothstep(0.62, 0.95, vnoise(uv * 3.0)) * moving * 0.28;
        base = mix(base, vec3f(0.75, 0.85, 0.95) * (light + 0.02), foam);
        col = mix(base, refl, select(fres * (1.0 - foam), 0.0, frame.fog.z > 0.5));
        let spec = pow(max(dot(R, L), 0.0), 240.0) * vis;
        col += frame.sunColor.rgb * spec * strength * 3.0;
        alpha = clamp(mix(0.68, 0.92, fres) + spec + foam, 0.0, 1.0);
    }

    if (!vm) {
        var fogCol: vec3f;
        var f: f32;
        if (frame.fog.z > 0.5) {
            fogCol = vec3f(0.02, 0.07, 0.16) * (frame.ambient.rgb * 1.6 + 0.05);
            f = clamp(dist / frame.fog.y, 0.0, 1.0);
        } else {
            fogCol = skyColor(-V, frame.sunDir.xyz) * mix(0.08, 1.0, frame.ambient.w);
            f = clamp((dist - frame.fog.x) / max(frame.fog.y - frame.fog.x, 1.0), 0.0, 1.0);
            f = f * f * (3.0 - 2.0 * f);
        }
        col = mix(col, fogCol, f);
        if (torchFire) {
            alpha = alpha * (1.0 - f);
        } else {
            alpha = mix(alpha, 1.0, f);
        }
    }

    // params.y is set only for the local player pass. A blocked third-person camera can
    // move through the model; fading that pass keeps the world visible.
    if (pc.params.y > 0.0) {
        alpha *= pc.params.y;
    }
    return vec4f(acesTonemap(col * frame.misc.y), alpha);
}
