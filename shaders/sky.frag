#version 450
#extension GL_GOOGLE_include_directive : require
#include "frame.glsl"
#include "common.glsl"

layout(location = 0) in vec2 vNdc;
layout(location = 0) out vec4 outColor;

float cloudDensity(vec2 p) {
    // Bend the large shapes so they form groups instead of straight noise bands.
    vec2 warp = vec2(
        vnoise(p * 0.32 + vec2(11.7, 4.3)),
        vnoise(p * 0.32 + vec2(-5.4, 13.1))
    ) - 0.5;
    p += warp * 0.8;
    // Broad masses with smaller lobes and a little detail at the silhouette.
    return 0.58 * vnoise(p * 0.48)
         + 0.32 * vnoise(p * 1.2 + vec2(7.9, 2.6))
         + 0.10 * vnoise(p * 3.4 + vec2(-3.1, 9.7));
}

float distantClouds(vec3 dir, float time) {
    // Rounded lobes on a far sky ring retain a visible silhouette at the horizon.
    // Wrap the cells around the ring so there is no seam behind the camera.
    const float cells = 64.0;
    float u = mod((atan(dir.z, dir.x) + 3.14159265) * (cells / 6.2831853)
                  + time * 0.01, cells);
    float cell = floor(u);
    float edge = vnoise(vec2(u * 1.3, dir.y * 34.0) + vec2(6.8, 2.1));
    float cloud = 0.0;
    for (int i = -1; i <= 1; i++) {
        float index = cell + float(i);
        float id = mod(index + cells, cells);
        float seed = hash12(vec2(id, 17.0));
        float side = hash12(vec2(id, 29.0));
        float tall = hash12(vec2(id, 43.0));
        float center = index + 0.5 + (side - 0.5) * 0.3;
        float height = 0.03 + 0.05 * tall;
        float base = 0.006 + 0.008 * side;
        vec2 local = vec2((u - center) / (0.58 + 0.16 * side),
                          (dir.y - base - height * 0.52) / (height * 0.55));
        vec2 left = vec2((u - center + 0.47) / 0.43,
                         (dir.y - base - height * 0.36) / (height * 0.39));
        vec2 right = vec2((u - center - 0.43) / 0.40,
                          (dir.y - base - height * 0.40) / (height * 0.43));
        float lobes = min(length(local), min(length(left), length(right)))
                    + (edge - 0.5) * 0.12;
        cloud = max(cloud, smoothstep(0.38, 0.55, seed)
                           * (1.0 - smoothstep(0.78, 1.07, lobes)));
    }
    return cloud;
}

void main() {
    vec4 w = frame.invViewProj * vec4(vNdc, 1.0, 1.0);
    vec3 dir = normalize(w.xyz / w.w - frame.camPos.xyz);
    vec3 sunDir = frame.sunDir.xyz;
    float day = frame.sunDir.w;
    float time = frame.camPos.w;

    vec3 col = skyColor(dir, sunDir);
    float mu = dot(dir, sunDir);

    // Sun disc
    col += vec3(1.0, 0.92, 0.75) * smoothstep(0.99955, 0.9998, mu) * 24.0;

    // Moon with soft halo
    float mm = dot(dir, -sunDir);
    col += vec3(0.78, 0.82, 0.95) * smoothstep(0.99962, 0.99982, mm) * 2.5;
    col += vec3(0.2, 0.25, 0.4) * pow(max(mm, 0.0), 300.0) * 0.5 * (1.0 - day);

    // Stars
    float night = 1.0 - day;
    if (night > 0.01 && dir.y > -0.05) {
        vec3 d = dir * 180.0;
        vec3 cell = floor(d);
        float h = hash13(cell);
        if (h > 0.996) {
            vec3 f = fract(d) - 0.5;
            float s = smoothstep(0.35, 0.0, length(f));
            float tw = 0.7 + 0.3 * sin(time * (2.0 + h * 5.0) + h * 80.0);
            col += vec3(0.85, 0.9, 1.0) * s * tw * night * 1.6 * smoothstep(-0.05, 0.2, dir.y);
        }
    }

    // A distant cloud bank has its own height profile. A single flat cloud plane
    // becomes a set of thin horizontal streaks when viewed toward the horizon.
    if (frame.misc.x > 0.5) {
        if (frame.camPos.y < 210.0 && dir.y > 0.0 && dir.y < 0.14) {
            float bank = distantClouds(dir, frame.misc.w);
            float topLight = smoothstep(0.01, 0.09, dir.y);
            vec3 bankColor = mix(vec3(0.045, 0.055, 0.08), vec3(1.0, 1.0, 0.98), day);
            bankColor *= mix(0.72, 0.95, topLight);
            float sunset = clamp(1.0 - abs(sunDir.y) * 3.5, 0.0, 1.0);
            bankColor += vec3(0.7, 0.28, 0.12) * sunset * day * topLight * 0.25;
            col = mix(col, bankColor, bank * 0.8);
        }

        // The overhead layer takes over as the sightline rises above the distant bank.
        float cloudH = 210.0;
        float t = (cloudH - frame.camPos.y) / dir.y;
        if (t > 0.0 && t < 7000.0) {
            vec2 p = frame.camPos.xz + dir.xz * t;
            vec2 q = (p + vec2(frame.misc.w * 3.0, frame.misc.w * 1.2)) * 0.006;
            float density = cloudDensity(q);
            float cov = smoothstep(0.55, 0.65, density);
            if (cov > 0.0) {
                float body = smoothstep(0.58, 0.70, density);
                float sunward = vnoise(q * 1.2 + sunDir.xz * 0.3 + vec2(7.9, 2.6));
                float shade = mix(0.80, 1.10, sunward) - 0.12 * body;
                vec3 lit = mix(vec3(0.045, 0.055, 0.08), vec3(1.12, 1.10, 1.06), day);
                float sunset = clamp(1.0 - abs(sunDir.y) * 3.5, 0.0, 1.0);
                float rim = (1.0 - body) * smoothstep(0.2, 0.9, cov);
                vec3 cc = lit * shade;
                cc += vec3(1.0, 0.47, 0.22) * sunset * day * (0.15 + 0.3 * rim);
                cc += vec3(1.0, 0.88, 0.68) * pow(max(mu, 0.0), 10.0) * rim * day * 0.35;
                if (dir.y < 0.0) cc *= 0.7;
                float fade = exp(-t * 0.00024) * smoothstep(0.06, 0.16, abs(dir.y));
                col = mix(col, cc, cov * fade);
            }
        }
    }

    outColor = vec4(acesTonemap(col * frame.misc.y), 1.0);
}
