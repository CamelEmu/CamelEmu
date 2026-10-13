use std::sync::Arc;

use crate::custom_textures::material::Material;
use crate::surface_params::SurfaceParams;

pub const MAX_UPLOADS_PER_TICK: usize = 8;

/// The custom texture manager, which loads, dumps and decodes the custom textures of the title.
///
/// The uploads that wait for an asynchronous decode upload into the rasterizer cache, so the cache
/// keeps them and processes them on each frame as `CustomTexManager::TickFrame` does.
pub trait CustomTexManager {
    /// Whether the custom textures have been loaded, before which queued uploads are not processed.
    fn textures_loaded(&self) -> bool;

    /// Searches the load directory assigned to program_id for any custom textures and loads them
    fn find_custom_textures(&mut self);

    /// Saves the provided pixel data described by params to disk as png
    fn dump_texture(&mut self, params: &SurfaceParams, level: u32, data: &[u8], data_hash: u64);

    /// Returns the material assigned to the provided data hash
    fn get_material(&mut self, data_hash: u64) -> Option<Arc<Material>>;

    /// Decodes the textures in material to a consumable format. Returns true when the material was
    /// decoded right away and can be uploaded now, or false when the decode was queued and the
    /// upload waits for it.
    fn decode(&mut self, material: &Arc<Material>) -> bool;

    /// True when mipmap uploads should be skipped (legacy packs only)
    fn skip_mipmaps(&self) -> bool;

    /// Returns true if the pack uses the new hashing method.
    fn use_new_hash(&self) -> bool;
}
