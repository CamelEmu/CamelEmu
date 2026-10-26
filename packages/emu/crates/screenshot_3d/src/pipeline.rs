use crate::context::Screenshot3dContext;
use crate::error::{Result, Screenshot3dError};
use crate::obj::ObjExport;
use crate::progress::{Meshes, Progress, Shaders};
use crate::scene::Scene;
use crate::settings::ExportSettings;

pub struct Screenshot3dPipeline {
    scene: Scene,
    settings: ExportSettings,
}

impl Screenshot3dPipeline {
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn build(contexts: Vec<Screenshot3dContext>, progress: &mut Progress) -> Option<Self> {
        let draws = contexts
            .iter()
            .map(|context| context.vertices.draws.len() as u64)
            .sum();
        progress.begin(&Shaders, draws);
        let frustums: Vec<_> = contexts
            .iter()
            .map(|context| Scene::frustums(context, progress))
            .collect();
        progress.begin(&Meshes, contexts.iter().map(Screenshot3dContext::triangle_count).sum());
        let scene = contexts
            .iter()
            .zip(&frustums)
            .map(|(context, frustums)| Scene::build(context, frustums, progress))
            .reduce(|top, bottom| Scene::from((top, bottom)))?;
        Some(Self {
            scene,
            settings: ExportSettings::default(),
        })
    }

    pub fn to_glb(&self) -> Result<Vec<u8>> {
        self.to_glb_with(&mut Progress::silent())
    }

    pub fn to_glb_with(&self, progress: &mut Progress) -> Result<Vec<u8>> {
        self.ensure_geometry()?;
        self.scene
            .merged(&self.settings, progress)
            .to_glb(&self.settings, progress)
    }

    pub fn to_obj(&self, mtl_name: &str) -> Result<ObjExport> {
        self.to_obj_with(mtl_name, &mut Progress::silent())
    }

    pub fn to_obj_with(&self, mtl_name: &str, progress: &mut Progress) -> Result<ObjExport> {
        self.ensure_geometry()?;
        self.scene
            .merged(&self.settings, progress)
            .to_obj(mtl_name, &self.settings, progress)
    }

    fn ensure_geometry(&self) -> Result<()> {
        if self.scene.meshes.is_empty() {
            return Err(Screenshot3dError::EmptyCapture);
        }
        Ok(())
    }
}

impl From<Screenshot3dContext> for Screenshot3dPipeline {
    fn from(context: Screenshot3dContext) -> Self {
        Self {
            scene: Scene::from(&context),
            settings: ExportSettings::default(),
        }
    }
}

impl From<(Screenshot3dPipeline, Screenshot3dPipeline)> for Screenshot3dPipeline {
    fn from((top, bottom): (Screenshot3dPipeline, Screenshot3dPipeline)) -> Self {
        Self {
            scene: Scene::from((top.scene, bottom.scene)),
            settings: top.settings,
        }
    }
}
