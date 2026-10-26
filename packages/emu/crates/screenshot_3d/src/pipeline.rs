use crate::context::Screenshot3dContext;
use crate::error::{Result, Screenshot3dError};
use crate::obj::ObjExport;
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

    pub fn to_glb(&self) -> Result<Vec<u8>> {
        self.ensure_geometry()?;
        self.scene.merged(&self.settings).to_glb(&self.settings)
    }

    pub fn to_obj(&self, mtl_name: &str) -> Result<ObjExport> {
        self.ensure_geometry()?;
        self.scene
            .merged(&self.settings)
            .to_obj(mtl_name, &self.settings)
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
