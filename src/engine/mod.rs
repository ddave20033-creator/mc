//! The GPU, below the renderer (wgpu: Vulkan, DirectX 12 or Metal): the device, the window's
//! surface and the frames (`gpu`), buffers and textures (`resources`), and render pipelines
//! (`pipeline`). What is drawn with them is `render`'s.

pub mod gpu;
pub mod pipeline;
pub mod resources;

pub use gpu::{Frame, Gpu, FRAMES_IN_FLIGHT};
pub use resources::{Buffer, SamplerKind, Texture};
