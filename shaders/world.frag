#version 450
#extension GL_GOOGLE_include_directive : require
#include "frame.glsl"
#include "common.glsl"
#include "fire.glsl"

layout(set = 0, binding = 0) uniform sampler2DArray blocks;
layout(set = 0, binding = 1) uniform sampler2DShadow shadowMap;

layout(location = 0) in vec2 vUV;
layout(location = 1) flat in float vLayer;
layout(location = 2) in vec4 vLight;
layout(location = 3) in vec3 vTint;
layout(location = 4) in vec3 vWorld;
layout(location = 5) flat in int vFlags;
layout(location = 6) flat in int vNormal;

layout(location = 0) out vec4 outColor;

const int F_PLANT = 2;
const int F_EMISSIVE = 4;
const int F_WATER = 8;
const int F_OVERLAY = 16;
const int F_VIEWMODEL = 32;
const int F_ENTITY = 64;
const int F_FLUID = 128;
const float LAVA_LAYER = 38.0;
const float GRASS_SIDE_LAYER = 1.0;
const float FURNACE_LIT_LAYER = 63.0;
const float TORCH_FLAME_LAYER = 113.0;
const float GLASS_LAYER = 13.0;
const float FURNACE_ANIM_LAYER = 117.0;
const float FURNACE_FRAMES = 12.0;

const vec3 NORMALS[7] = vec3[7](
    vec3(1, 0, 0), vec3(-1, 0, 0), vec3(0, 1, 0), vec3(0, -1, 0),
    vec3(0, 0, 1), vec3(0, 0, -1), vec3(0, 1, 0)
);

// Minecraft-like light falloff: 15 -> 1.0, 8 -> ~0.2, 0 -> 0.
float lightCurve(float l) {
    return l / (4.0 - 3.0 * l);
}

// Classic per-face brightness: top 1.0, north/south 0.8, east/west 0.6, bottom 0.5.
float faceShade(vec3 n) {
    vec3 n2 = n * n;
    return n2.x * 0.6 + n2.z * 0.8 + n2.y * (n.y > 0.0 ? 1.0 : 0.5);
}

float shadowAt(vec3 p, vec3 n) {
    if (frame.fog.w < 0.5) return 1.0;
    vec4 lp = frame.lightViewProj * vec4(p + n * 0.06, 1.0);
    vec3 c = lp.xyz / lp.w;
    vec2 uv = c.xy * 0.5 + 0.5;
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || c.z >= 1.0) return 1.0;
    float t = frame.misc.z;
    // 4x4 grid of bilinear-filtered comparisons: soft, stable edges.
    float s = 0.0;
    for (int x = 0; x < 4; x++) {
        for (int y = 0; y < 4; y++) {
            vec2 o = (vec2(x, y) - 1.5) * t;
            s += texture(shadowMap, vec3(uv + o, c.z - 0.0003));
        }
    }
    s /= 16.0;
    float edge = max(abs(c.x), abs(c.y));
    return mix(s, 1.0, smoothstep(0.8, 1.0, edge));
}

void main() {
    // Selection outline
    if (vLayer < 0.0) {
        outColor = vec4(0.0, 0.0, 0.0, 0.6);
        return;
    }

    float time = frame.camPos.w;
    bool water = (vFlags & F_WATER) != 0;
    bool emissive = (vFlags & F_EMISSIVE) != 0;
    bool vm = (vFlags & F_VIEWMODEL) != 0;
    bool derivN = (vFlags & (F_VIEWMODEL | F_ENTITY)) != 0;
    bool plant = (vFlags & F_PLANT) != 0;

    bool fluid = (vFlags & F_FLUID) != 0;
    vec2 uv = vUV;
    vec2 flow = vec2(0.0);
    if (fluid) {
        // Texture streams along the flow direction (downhill on top, downward on sides).
        float speed = water ? 0.9 : 0.22;
        flow = vTint.rg * 2.0 - 1.0;
        bool falling = vTint.b > 0.5;
        if (vNormal == 2) {
            if (length(flow) > 0.05) {
                uv -= flow * time * speed;
            } else {
                uv += vec2(time * 0.03, time * 0.045) * speed;
            }
        } else if (vNormal != 3) {
            uv.y -= time * speed * (falling ? 1.6 : 0.6);
        }
    }
    bool torchFire = abs(vLayer - TORCH_FLAME_LAYER) < 0.5;
    vec4 tex = texture(blocks, vec3(uv, vLayer));
    bool furnaceLit = abs(vLayer - FURNACE_LIT_LAYER) < 0.5;
    if (furnaceLit && tex.a > 0.9) {
        // A resource pack's lit furnace (full alpha; built-in opaque textures use 0.6) plays
        // animation frames made from its own art, at 8 fps with a phase per furnace.
        float frame = mod(floor(time * 8.0 + vTint.r * FURNACE_FRAMES), FURNACE_FRAMES);
        tex = texture(blocks, vec3(uv, FURNACE_ANIM_LAYER + frame));
    }
    // The built-in lit furnace gets procedural fire in its opening instead.
    bool furnaceFire = furnaceLit && tex.a < 0.9;

    if ((vFlags & F_OVERLAY) != 0) {
        // Multiply-blended (result = 2 * src * dst): mid-gray leaves the block unchanged, darker
        // pixels darken it, lighter ones brighten it. Minecraft does this in gamma space, so the
        // factor is converted to linear to match.
        if (tex.a < 0.1) discard;
        vec3 factor = pow(2.0 * mix(vec3(0.5), pow(tex.rgb, vec3(1.0 / 2.2)), tex.a), vec3(2.2));
        outColor = vec4(factor * 0.5, 1.0);
        return;
    }
    if (!water && tex.a < 0.5) discard;
    // Glass faces carry their connection mask inverted in the red tint channel.
    bool glass = abs(vLayer - GLASS_LAYER) < 0.5;
    if (glass && glassSeam(uv, 255 - int(vTint.r * 255.0 + 0.5))) discard;

    vec4 flame = vec4(0.0);
    if (torchFire || furnaceFire) {
        // The mesh stores a single phase on all vertices of each flame.
        // Rounding interpolated world positions at a block edge caused speckles.
        float seed = vTint.r;
        if (torchFire) {
            flame = torchFlame(uv, time, seed);
            if (flame.a < 0.02) discard;
        } else {
            vec2 opening = (uv - vec2(32.0, 66.0) / 128.0) / (vec2(64.0, 46.0) / 128.0);
            if (all(greaterThanEqual(opening, vec2(0.0))) && all(lessThanEqual(opening, vec2(1.0)))) {
                flame = furnaceFlame(opening, time, seed);
            }
        }
    }

    // Biome tint. The grass-block side uses its alpha channel as a tint mask (grass vs dirt).
    vec3 tint = fluid || furnaceLit || glass ? vec3(1.0) : pow(vTint, vec3(2.2));
    float mask = abs(vLayer - GRASS_SIDE_LAYER) < 0.5 ? clamp((tex.a - 0.6) / 0.4, 0.0, 1.0) : 1.0;
    vec3 albedo = tex.rgb * mix(vec3(1.0), tint, mask);

    vec3 N = NORMALS[clamp(vNormal, 0, 6)];
    if (derivN) {
        N = normalize(cross(dFdx(vWorld), dFdy(vWorld)));
        if (dot(N, frame.camPos.xyz - vWorld) < 0.0) N = -N;
    }

    float ao = mix(0.45, 1.0, vLight.x);
    float sky = lightCurve(vLight.y);
    float blk = lightCurve(vLight.z);
    vec3 L = frame.lightDir.xyz;
    float ndl = plant ? 0.8 : dot(N, L);
    float exposure = smoothstep(0.45, 0.85, vLight.y);
    float vis = 0.0;
    if (ndl > 0.0 && exposure > 0.0) {
        vis = shadowAt(vWorld, plant ? vec3(0.0, 1.0, 0.0) : N) * smoothstep(0.0, 0.25, ndl) * exposure;
    }

    // Sky light with classic face shading, modulated by sun visibility:
    // shaded areas turn slightly cooler and darker, sunlit ones slightly warmer.
    float shade = plant ? 0.9 : faceShade(N);
    float strength = frame.sunColor.w;
    vec3 sunMod = mix(vec3(1.0), mix(vec3(0.7, 0.74, 0.84), frame.sunColor.rgb * 1.12, vis), strength);
    vec3 skyLight = frame.ambient.rgb * sky * shade * sunMod;
    vec3 torch = vec3(1.0, 0.72, 0.42) * blk * shade * 1.2;
    vec3 light = max(skyLight, torch) + min(skyLight, torch) * 0.3;
    for (int i = 0; i < 8; i++) {
        vec4 held = frame.heldLights[i];
        if (held.w <= 0.0) continue;
        float falloff = max(0.0, 1.0 - distance(vWorld, held.xyz) / 7.0);
        light += vec3(1.0, 0.65, 0.34) * falloff * falloff * 1.8 * held.w;
    }
    vec3 col = albedo * (light * ao + vec3(0.02));
    if (emissive) col = albedo * 1.4;
    if (torchFire) col = flame.rgb * 1.25;
    if (furnaceFire) col = mix(col, flame.rgb * 0.88, flame.a);

    vec3 toCam = frame.camPos.xyz - vWorld;
    float dist = length(toCam);
    vec3 V = toCam / max(dist, 1e-4);
    float alpha = 1.0;
    if (torchFire) alpha = flame.a;

    if (water) {
        vec3 n = N;
        if (vNormal == 2) {
            vec2 q = vWorld.xz;
            float h1 = vnoise(q * 1.1 + vec2(time * 0.5, time * 0.3));
            float h2 = vnoise(q * 2.3 - vec2(time * 0.4, -time * 0.6));
            n = normalize(vec3((h1 - 0.5) * 0.3 + (h2 - 0.5) * 0.18, 1.0, (h2 - 0.5) * 0.3 - (h1 - 0.5) * 0.12));
        }
        float cosv = abs(dot(n, V));
        float fres = 0.02 + 0.6 * pow(1.0 - cosv, 5.0);
        vec3 R = reflect(-V, n);
        R.y = abs(R.y);
        vec3 refl = skyColor(R, frame.sunDir.xyz) * mix(0.12, 0.8, sky);
        vec3 base = tex.rgb * vec3(0.1, 0.27, 0.6) * (light + 0.02);
        // Soft foam streaks on moving water, carried along by the scrolled texture coordinates.
        float moving = vNormal == 2 ? clamp(length(flow), 0.0, 1.0) : (vTint.b > 0.5 ? 1.0 : 0.4);
        float foam = smoothstep(0.62, 0.95, vnoise(uv * 3.0)) * moving * 0.28;
        base = mix(base, vec3(0.75, 0.85, 0.95) * (light + 0.02), foam);
        col = mix(base, refl, frame.fog.z > 0.5 ? 0.0 : fres * (1.0 - foam));
        float spec = pow(max(dot(R, L), 0.0), 240.0) * vis;
        col += frame.sunColor.rgb * spec * strength * 3.0;
        alpha = clamp(mix(0.68, 0.92, fres) + spec + foam, 0.0, 1.0);
    }

    if (!vm) {
        vec3 fogCol;
        float f;
        if (frame.fog.z > 0.5) {
            fogCol = vec3(0.02, 0.07, 0.16) * (frame.ambient.rgb * 1.6 + 0.05);
            f = clamp(dist / frame.fog.y, 0.0, 1.0);
        } else {
            fogCol = skyColor(-V, frame.sunDir.xyz) * mix(0.08, 1.0, frame.ambient.w);
            f = clamp((dist - frame.fog.x) / max(frame.fog.y - frame.fog.x, 1.0), 0.0, 1.0);
            f = f * f * (3.0 - 2.0 * f);
        }
        col = mix(col, fogCol, f);
        alpha = torchFire ? alpha * (1.0 - f) : mix(alpha, 1.0, f);
    }

    // params.y is set only for the local player pass. A blocked third-person camera can
    // move through the model; fading that pass keeps the world visible.
    if (pc.params.y > 0.0) alpha *= pc.params.y;
    outColor = vec4(acesTonemap(col * frame.misc.y), alpha);
}
