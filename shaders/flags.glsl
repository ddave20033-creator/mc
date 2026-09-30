// What a vertex is (its tint's alpha: world::mesh::flags), and the detail far blocks lose, in
// pixels per block (frame.detail.x over the distance; render::cull leaves out the same). One
// copy for all the shaders; render::frame's test checks they are the game's.
#ifndef FLAGS_GLSL
#define FLAGS_GLSL
const int F_LEAVES = 1;
const int F_PLANT = 2;
const int F_EMISSIVE = 4;
const int F_WATER = 8;
const int F_OVERLAY = 16;
const int F_VIEWMODEL = 32;
const int F_ENTITY = 64;
const int F_FLUID = 128;
// Grass and flowers fade out between these (world.frag; world.vert drops them below).
const float PLANT_FULL_PX = 10.0;
const float PLANT_GONE_PX = 5.0;
#endif
