// The sky: a full-screen triangle (the backdrop blur's first pass draws with its vertex
// shader too), its colour from the view direction: gradient, sun, moon, stars and clouds.
#include "common.wgsl"
#include "frame.wgsl"

struct SkyOutput {
    @builtin(position) pos: vec4f,
    @location(0) vNdc: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> SkyOutput {
    let p = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    var o: SkyOutput;
    o.vNdc = p * 2.0 - 1.0;
    o.pos = toClip(vec4f(o.vNdc, 0.0, 1.0));
    return o;
}

fn cloudDensity(p0: vec2f) -> f32 {
    // Bend the large shapes so they form groups instead of straight noise bands.
    let warp = vec2f(
        vnoise(p0 * 0.32 + vec2f(11.7, 4.3)),
        vnoise(p0 * 0.32 + vec2f(-5.4, 13.1))
    ) - 0.5;
    let p = p0 + warp * 0.8;
    // Broad masses with smaller lobes and a little detail at the silhouette.
    return 0.58 * vnoise(p * 0.48)
         + 0.32 * vnoise(p * 1.2 + vec2f(7.9, 2.6))
         + 0.10 * vnoise(p * 3.4 + vec2f(-3.1, 9.7));
}

fn distantClouds(dir: vec3f, time: f32) -> f32 {
    // Rounded lobes on a far sky ring retain a visible silhouette at the horizon.
    // Wrap the cells around the ring so there is no seam behind the camera.
    let cells = 64.0;
    let u = gmod((atan2(dir.z, dir.x) + 3.14159265) * (cells / 6.2831853) + time * 0.01, cells);
    let cell = floor(u);
    let edge = vnoise(vec2f(u * 1.3, dir.y * 34.0) + vec2f(6.8, 2.1));
    var cloud = 0.0;
    for (var i = -1; i <= 1; i++) {
        let index = cell + f32(i);
        let id = gmod(index + cells, cells);
        let seed = hash12(vec2f(id, 17.0));
        let side = hash12(vec2f(id, 29.0));
        let tall = hash12(vec2f(id, 43.0));
        let center = index + 0.5 + (side - 0.5) * 0.3;
        let height = 0.03 + 0.05 * tall;
        let base = 0.006 + 0.008 * side;
        let local = vec2f((u - center) / (0.58 + 0.16 * side),
                          (dir.y - base - height * 0.52) / (height * 0.55));
        let left = vec2f((u - center + 0.47) / 0.43,
                         (dir.y - base - height * 0.36) / (height * 0.39));
        let right = vec2f((u - center - 0.43) / 0.40,
                          (dir.y - base - height * 0.40) / (height * 0.43));
        let lobes = min(length(local), min(length(left), length(right)))
                  + (edge - 0.5) * 0.12;
        cloud = max(cloud, smoothstep(0.38, 0.55, seed)
                           * (1.0 - smoothstep(0.78, 1.07, lobes)));
    }
    return cloud;
}

@fragment
fn fs_main(in: SkyOutput) -> @location(0) vec4f {
    let w = frame.invViewProj * vec4f(in.vNdc, 1.0, 1.0);
    let dir = normalize(w.xyz / w.w - frame.camPos.xyz);
    let sunDir = frame.sunDir.xyz;
    let day = frame.sunDir.w;
    let time = frame.camPos.w;

    var col = skyColor(dir, sunDir);
    let mu = dot(dir, sunDir);

    // Sun disc
    col += vec3f(1.0, 0.92, 0.75) * smoothstep(0.99955, 0.9998, mu) * 24.0;

    // Moon with soft halo
    let mm = dot(dir, -sunDir);
    col += vec3f(0.78, 0.82, 0.95) * smoothstep(0.99962, 0.99982, mm) * 2.5;
    col += vec3f(0.2, 0.25, 0.4) * pow(max(mm, 0.0), 300.0) * 0.5 * (1.0 - day);

    // Stars
    let night = 1.0 - day;
    if (night > 0.01 && dir.y > -0.05) {
        let d = dir * 180.0;
        let cell = floor(d);
        let h = hash13(cell);
        if (h > 0.996) {
            let f = fract(d) - 0.5;
            let s = smoothstep(0.35, 0.0, length(f));
            let tw = 0.7 + 0.3 * sin(time * (2.0 + h * 5.0) + h * 80.0);
            col += vec3f(0.85, 0.9, 1.0) * s * tw * night * 1.6 * smoothstep(-0.05, 0.2, dir.y);
        }
    }

    // A distant cloud bank has its own height profile. A single flat cloud plane
    // becomes a set of thin horizontal streaks when viewed toward the horizon.
    if (frame.misc.x > 0.5) {
        if (frame.camPos.y < 210.0 && dir.y > 0.0 && dir.y < 0.14) {
            let bank = distantClouds(dir, frame.misc.w);
            let topLight = smoothstep(0.01, 0.09, dir.y);
            var bankColor = mix(vec3f(0.045, 0.055, 0.08), vec3f(1.0, 1.0, 0.98), day);
            bankColor *= mix(0.72, 0.95, topLight);
            let sunset = clamp(1.0 - abs(sunDir.y) * 3.5, 0.0, 1.0);
            bankColor += vec3f(0.7, 0.28, 0.12) * sunset * day * topLight * 0.25;
            col = mix(col, bankColor, bank * 0.8);
        }

        // The overhead layer takes over as the sightline rises above the distant bank.
        let cloudH = 210.0;
        let t = (cloudH - frame.camPos.y) / dir.y;
        if (t > 0.0 && t < 7000.0) {
            let p = frame.camPos.xz + dir.xz * t;
            let q = (p + vec2f(frame.misc.w * 3.0, frame.misc.w * 1.2)) * 0.006;
            let density = cloudDensity(q);
            let cov = smoothstep(0.55, 0.65, density);
            if (cov > 0.0) {
                let body = smoothstep(0.58, 0.70, density);
                let sunward = vnoise(q * 1.2 + sunDir.xz * 0.3 + vec2f(7.9, 2.6));
                let shade = mix(0.80, 1.10, sunward) - 0.12 * body;
                let lit = mix(vec3f(0.045, 0.055, 0.08), vec3f(1.12, 1.10, 1.06), day);
                let sunset = clamp(1.0 - abs(sunDir.y) * 3.5, 0.0, 1.0);
                let rim = (1.0 - body) * smoothstep(0.2, 0.9, cov);
                var cc = lit * shade;
                cc += vec3f(1.0, 0.47, 0.22) * sunset * day * (0.15 + 0.3 * rim);
                cc += vec3f(1.0, 0.88, 0.68) * pow(max(mu, 0.0), 10.0) * rim * day * 0.35;
                if (dir.y < 0.0) {
                    cc *= 0.7;
                }
                let fade = exp(-t * 0.00024) * smoothstep(0.06, 0.16, abs(dir.y));
                col = mix(col, cc, cov * fade);
            }
        }
    }

    return vec4f(acesTonemap(col * frame.misc.y), 1.0);
}
