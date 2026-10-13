use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

use crate::custom_textures::custom_tex_manager::CustomTexManager;
use crate::custom_textures::material::Material;
use crate::framebuffer_base::FramebufferParams;
use crate::interval::IntervalMap;
use crate::pixel_format::PixelFormat;
use crate::sampler_params::SamplerParams;
use crate::slot_id::{FramebufferId, SamplerId, SurfaceId};
use crate::slot_vector::SlotVector;
use crate::surface_base::SurfaceBase;
use crate::surface_params::SurfaceParams;
use crate::texture_cube::{TextureCube, TextureCubeConfig};
use crate::utils::{BufferTextureCopy, StagingData, TextureBlit, TextureClear, TextureCopy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMatch {
    Exact,   // Only accept same res scale
    Upscale, // Only allow higher scale than params
    Ignore,  // Accept every scaled res
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchFlags(pub u32);

impl MatchFlags {
    pub const EXACT: Self = Self(1 << 0); // Surface perfectly matches params
    pub const SUB_RECT: Self = Self(1 << 1); // Surface encompasses params
    pub const COPY: Self = Self(1 << 2); // Surface that can be used as a copy source
    pub const TEX_COPY: Self = Self(1 << 3); // Surface that will match a display transfer "texture copy" parameters
    pub const REINTERPRET: Self = Self(1 << 4); // Surface might have different pixel format.

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureFilter {
    #[default]
    NoFilter = 0,
    Anime4K = 1,
    Bicubic = 2,
    ScaleForce = 3,
    XBrz = 4,
    Mmpx = 5,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureSampling {
    #[default]
    GameControlled = 0,
    NearestNeighbor = 1,
    Linear = 2,
}

/// The settings the rasterizer cache reads, which the core reads from `Settings::values`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheSettings {
    pub texture_filter: TextureFilter,
    pub texture_sampling: TextureSampling,
    pub dump_textures: bool,
    pub custom_textures: bool,
}

/// The physical memory as the rasterizer cache reads and writes it.
pub trait CacheMemory {
    /// Returns the memory from the address to the end of the memory that backs it, as
    /// `MemorySystem::GetPhysicalRef` does, or `None` when nothing backs the address.
    fn get_physical_ref(&self, address: u32) -> Option<*mut [u8]>;

    /// Marks the region as cached by the rasterizer, so that the CPU accesses it through the
    /// rasterizer cache, as `MemorySystem::RasterizerMarkRegionCached` does.
    fn rasterizer_mark_region_cached(&self, start: u32, size: u32, cached: bool);
}

/// A surface of the backend, which holds the host texture of a cached surface.
pub trait Surface: Deref<Target = SurfaceBase> + DerefMut {
    type Runtime;

    /// Scales the surface up to the new resolution scale.
    fn scale_up(&mut self, runtime: &mut Self::Runtime, new_scale: u32);

    /// Returns the bytes per pixel of the host texture.
    fn get_internal_bytes_per_pixel(&self) -> u32;

    /// Uploads the staging data into the texture.
    fn upload(
        &mut self,
        runtime: &mut Self::Runtime,
        upload: &BufferTextureCopy,
        staging: &StagingData,
    );

    /// Downloads the texture into the staging data.
    fn download(
        &mut self,
        runtime: &mut Self::Runtime,
        download: &BufferTextureCopy,
        staging: &StagingData,
    );

    /// Uploads the custom texture of the material into the level.
    fn upload_custom(&mut self, runtime: &mut Self::Runtime, material: &Arc<Material>, level: u32);
}

/// A framebuffer of the backend.
pub trait Framebuffer {
    /// The resolution scale of the attachments.
    fn scale(&self) -> u32;
}

/// The texture runtime of the backend, which creates the host objects and runs the texture
/// operations.
pub trait Runtime: Sized {
    type Surface: Surface<Runtime = Self>;
    type Sampler;
    type Framebuffer: Framebuffer;

    fn create_surface(&mut self, params: &SurfaceParams) -> Self::Surface;

    fn create_custom_surface(
        &mut self,
        surface: &SurfaceBase,
        material: &Arc<Material>,
    ) -> Self::Surface;

    fn create_sampler(&mut self, params: &SamplerParams) -> Self::Sampler;

    fn create_framebuffer(
        &mut self,
        params: &FramebufferParams,
        color: Option<&Self::Surface>,
        depth: Option<&Self::Surface>,
    ) -> Self::Framebuffer;

    /// The number of frames a sentenced surface waits before it is destroyed.
    fn remove_threshold(&self) -> u64;

    fn clear_texture(&mut self, surface: &mut Self::Surface, clear: &TextureClear) -> bool;

    fn copy_textures(
        &mut self,
        source: &mut Self::Surface,
        dest: &mut Self::Surface,
        copies: &[TextureCopy],
    ) -> bool;

    fn blit_textures(
        &mut self,
        source: &mut Self::Surface,
        dest: &mut Self::Surface,
        blit: &TextureBlit,
    ) -> bool;

    fn generate_mipmaps(&mut self, surface: &mut Self::Surface);

    fn find_staging(&mut self, size: u32, upload: bool) -> StagingData;

    fn needs_conversion(&self, pixel_format: PixelFormat) -> bool;

    fn reinterpret(
        &mut self,
        source: &mut Self::Surface,
        dest: &mut Self::Surface,
        copy: &TextureCopy,
    ) -> bool;

    fn finish(&mut self);

    /// Opens a debug scope labelled for graphics debuggers, as the `DebugScope` of the backend.
    fn push_debug_scope(&mut self, _color: [f32; 4], _label: fmt::Arguments) {}

    /// Closes the debug scope opened last.
    fn pop_debug_scope(&mut self) {}
}

/// A custom texture upload that waits for its material to be decoded.
pub(crate) struct AsyncUpload {
    pub material: Arc<Material>,
    pub surface_id: SurfaceId,
    pub level: u32,
}

pub struct RasterizerCache<R: Runtime> {
    pub(crate) memory: Rc<dyn CacheMemory>,
    pub(crate) custom_tex_manager: Rc<RefCell<dyn CustomTexManager>>,
    pub runtime: R,
    pub settings: CacheSettings,
    pub(crate) texture_cube_cache: HashMap<TextureCubeConfig, TextureCube>,
    pub(crate) page_table: HashMap<u64, Vec<SurfaceId>>,
    pub(crate) framebuffers: HashMap<FramebufferParams, FramebufferId>,
    pub(crate) samplers: HashMap<SamplerParams, SamplerId>,
    pub(crate) sentenced: Vec<(SurfaceId, u64)>,
    pub(crate) slot_surfaces: SlotVector<R::Surface>,
    pub(crate) slot_samplers: SlotVector<R::Sampler>,
    pub(crate) slot_framebuffers: SlotVector<R::Framebuffer>,
    pub(crate) dirty_regions: IntervalMap<SurfaceId>,
    pub(crate) cached_pages: IntervalMap<i32>,
    pub(crate) resolution_scale_factor: u32,
    pub(crate) frame_tick: u64,
    pub(crate) filter: TextureFilter,
    pub(crate) dump_textures: bool,
    pub(crate) use_custom_textures: bool,
    pub(crate) async_uploads: Vec<AsyncUpload>,
}
