use std::collections::HashMap;

use crate::context::AlphaMode;
use crate::mesh::MeshData;
use crate::progress::{Atlas, Merging, Progress};
use crate::scene::{Scene, SceneImage};
use crate::settings::ExportSettings;

#[derive(Clone, Copy, PartialEq, Eq)]
struct GroupKey {
    alpha: AlphaMode,
    cutoff: u32,
    filters: Option<(bool, bool)>,
}

#[derive(Clone, Copy)]
struct Placement {
    page: usize,
    x: u32,
    y: u32,
}

struct Page {
    items: Vec<(usize, u32, u32)>,
    width: u32,
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
}

impl Page {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            width: 0,
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
        }
    }

    fn height(&self) -> u32 {
        self.shelf_y + self.shelf_height
    }

    fn try_place(&mut self, index: usize, width: u32, height: u32, extent: u32) -> bool {
        if self.items.is_empty() {
            self.items.push((index, 0, 0));
            self.width = width;
            self.shelf_x = width;
            self.shelf_height = height;
            return true;
        }
        if self.shelf_x + width > extent {
            let next_y = self.shelf_y + self.shelf_height;
            if next_y + height > extent || width > extent {
                return false;
            }
            self.shelf_y = next_y;
            self.shelf_x = 0;
            self.shelf_height = 0;
        }
        if self.shelf_y + height > extent {
            return false;
        }
        self.items.push((index, self.shelf_x, self.shelf_y));
        self.shelf_x += width;
        self.width = self.width.max(self.shelf_x);
        self.shelf_height = self.shelf_height.max(height);
        true
    }
}

impl Scene {
    pub fn merged(&self, settings: &ExportSettings, progress: &mut Progress) -> Scene {
        progress.begin(&Merging, self.meshes.len() as u64);
        let mut groups: Vec<(GroupKey, Vec<usize>)> = Vec::new();
        for (index, mesh) in self.meshes.iter().enumerate() {
            progress.advance(1);
            let filters = mesh
                .texture
                .and_then(|texture| self.images.get(texture))
                .map(|image| (image.magnify_linear, image.minify_linear));
            let key = GroupKey {
                alpha: mesh.alpha_mode,
                cutoff: mesh.alpha_cutoff.to_bits(),
                filters,
            };
            match groups.iter_mut().find(|(existing, _)| *existing == key) {
                Some((_, members)) => members.push(index),
                None => groups.push((key, vec![index])),
            }
        }

        progress.complete();

        let packed: usize = groups
            .iter()
            .filter(|(key, _)| key.filters.is_some())
            .map(|(_, members)| self.used_textures(members).len())
            .sum();
        progress.begin(&Atlas, packed as u64);

        let mut meshes = Vec::new();
        let mut images: Vec<SceneImage> = Vec::new();
        for (key, members) in groups {
            let Some((magnify_linear, minify_linear)) = key.filters else {
                let parts = members.iter().map(|&index| self.meshes[index].clone());
                meshes.push(combine(parts, &key, None));
                continue;
            };

            let used = self.used_textures(&members);
            let (pages, placements) = pack(&used, &self.images, settings, progress);

            for (page_index, page) in pages.iter().enumerate() {
                let parts = members
                    .iter()
                    .map(|&index| &self.meshes[index])
                    .filter_map(|mesh| {
                        let texture = mesh.texture?;
                        let placement = placements.get(&texture)?;
                        (placement.page == page_index).then(|| {
                            let source = &self.images[texture];
                            remapped(mesh, source, placement, page, settings)
                        })
                    });
                meshes.push(combine(parts, &key, Some(images.len() + page_index)));
            }
            images.extend(pages.into_iter().map(|mut page| {
                page.magnify_linear = magnify_linear;
                page.minify_linear = minify_linear;
                page
            }));
        }

        progress.complete();

        Scene {
            meshes,
            camera: self.camera.clone(),
            images,
        }
    }

    fn used_textures(&self, members: &[usize]) -> Vec<usize> {
        let mut used: Vec<usize> = members
            .iter()
            .filter_map(|&index| self.meshes[index].texture)
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }
}

fn combine(
    parts: impl Iterator<Item = MeshData>,
    key: &GroupKey,
    texture: Option<usize>,
) -> MeshData {
    let mut merged = MeshData {
        positions: Vec::new(),
        texcoords: Vec::new(),
        colors: Vec::new(),
        indices: Vec::new(),
        texture,
        screen_width: 0,
        alpha_mode: key.alpha,
        alpha_cutoff: f32::from_bits(key.cutoff),
    };
    for part in parts {
        let base = merged.positions.len() as u32;
        merged.screen_width = merged.screen_width.max(part.screen_width);
        merged.positions.extend(part.positions);
        merged.texcoords.extend(part.texcoords);
        merged.colors.extend(part.colors);
        merged
            .indices
            .extend(part.indices.iter().map(|index| index + base));
    }
    merged
}

fn remapped(
    mesh: &MeshData,
    source: &SceneImage,
    placement: &Placement,
    page: &SceneImage,
    settings: &ExportSettings,
) -> MeshData {
    let mut mesh = mesh.clone();
    let left = (placement.x + settings.atlas_padding) as f32;
    let top = (placement.y + settings.atlas_padding) as f32;
    for texcoord in &mut mesh.texcoords {
        *texcoord = [
            (left + texcoord[0] * source.width as f32) / page.width as f32,
            (top + texcoord[1] * source.height as f32) / page.height as f32,
        ];
    }
    mesh
}

fn pack(
    used: &[usize],
    images: &[SceneImage],
    settings: &ExportSettings,
    progress: &mut Progress,
) -> (Vec<SceneImage>, HashMap<usize, Placement>) {
    let padding = settings.atlas_padding;
    let mut order: Vec<usize> = used
        .iter()
        .copied()
        .filter(|&index| images.get(index).is_some())
        .collect();
    order.sort_by_key(|&index| std::cmp::Reverse(images[index].height));

    let mut pages: Vec<Page> = vec![Page::new()];
    for &index in &order {
        let width = images[index].width + 2 * padding;
        let height = images[index].height + 2 * padding;
        let placed = pages
            .last_mut()
            .is_some_and(|page| page.try_place(index, width, height, settings.atlas_max_extent));
        if !placed {
            let mut page = Page::new();
            page.try_place(index, width, height, settings.atlas_max_extent);
            pages.push(page);
        }
    }
    pages.retain(|page| !page.items.is_empty());

    let mut placements = HashMap::new();
    let mut atlases = Vec::with_capacity(pages.len());
    for (page_index, page) in pages.iter().enumerate() {
        let (width, height) = (page.width, page.height());
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        for &(index, x, y) in &page.items {
            let image = &images[index];
            blit(&mut rgba, width, image, x, y, padding);
            progress.advance(1);
            placements.insert(
                index,
                Placement {
                    page: page_index,
                    x,
                    y,
                },
            );
        }
        atlases.push(SceneImage {
            width,
            height,
            rgba,
            wrap_s: settings.pica_clamp_to_edge,
            wrap_t: settings.pica_clamp_to_edge,
            magnify_linear: false,
            minify_linear: false,
        });
    }
    (atlases, placements)
}

fn blit(target: &mut [u8], target_width: u32, image: &SceneImage, x: u32, y: u32, padding: u32) {
    let outer_width = image.width + 2 * padding;
    let outer_height = image.height + 2 * padding;
    for row in 0..outer_height {
        let source_row = row.saturating_sub(padding).min(image.height - 1);
        for column in 0..outer_width {
            let source_column = column.saturating_sub(padding).min(image.width - 1);
            let from = (source_row as usize * image.width as usize + source_column as usize) * 4;
            let to = ((y + row) as usize * target_width as usize + (x + column) as usize) * 4;
            target[to..to + 4].copy_from_slice(&image.rgba[from..from + 4]);
        }
    }
}
