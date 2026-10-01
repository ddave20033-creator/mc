//! Vulkan, below the renderer: the device, the swapchain and the frames in flight (`gpu`),
//! buffers, images and textures (`resources`), and graphics pipelines (`pipeline`). What is
//! drawn with them is `render`'s.

pub mod gpu;
pub mod pipeline;
pub mod resources;

pub use gpu::{Gpu, FRAMES_IN_FLIGHT};
pub use resources::{Buffer, SamplerKind, Texture};
