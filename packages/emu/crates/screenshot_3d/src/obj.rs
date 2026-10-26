use std::fmt::Write;

use crate::error::Result;
use crate::progress::{Exporting, Progress};
use crate::scene::Scene;
use crate::settings::ExportSettings;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjExport {
    pub obj: String,
    pub mtl: String,
    pub textures: Vec<(String, Vec<u8>)>,
}

impl Scene {
    pub fn to_obj(
        &self,
        mtl_name: &str,
        settings: &ExportSettings,
        progress: &mut Progress,
    ) -> Result<ObjExport> {
        progress.begin(&Exporting, (self.images.len() + self.meshes.len()) as u64);
        let mut export = ObjExport::default();
        for (index, image) in self.images.iter().enumerate() {
            progress.advance(1);
            let file = format!("{}{index}.png", settings.texture_file_prefix);
            export.textures.push((file.clone(), image.encode_png(index)?));
            let _ = writeln!(
                export.mtl,
                "newmtl {}{index}\nKd 1 1 1\nmap_Kd {file}\n",
                settings.material_name_prefix
            );
        }

        let _ = writeln!(export.obj, "mtllib {mtl_name}");
        let mut vertex_base = 1;
        for (index, mesh) in self.meshes.iter().enumerate() {
            progress.advance(1);
            let has_texcoords = mesh.texcoords.len() == mesh.positions.len();
            let _ = writeln!(export.obj, "o {}{index}", settings.mesh_name_prefix);
            for [x, y, z] in &mesh.positions {
                let _ = writeln!(export.obj, "v {x} {y} {z}");
            }
            if has_texcoords {
                for [u, v] in &mesh.texcoords {
                    let _ = writeln!(export.obj, "vt {u} {v}");
                }
            }
            if let Some(texture) = mesh.texture {
                let _ = writeln!(export.obj, "usemtl {}{texture}", settings.material_name_prefix);
            }
            for triangle in mesh.indices.chunks_exact(3) {
                let corner = |i: u32| {
                    let n = vertex_base + i;
                    if has_texcoords {
                        format!("{n}/{n}")
                    } else {
                        n.to_string()
                    }
                };
                let _ = writeln!(
                    export.obj,
                    "f {} {} {}",
                    corner(triangle[0]),
                    corner(triangle[1]),
                    corner(triangle[2])
                );
            }
            vertex_base += mesh.positions.len() as u32;
        }
        Ok(export)
    }
}
