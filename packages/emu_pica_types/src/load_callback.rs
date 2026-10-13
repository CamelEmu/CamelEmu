#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadCallbackStage {
    Prepare,
    Preload,
    Decompile,
    Build,
    Complete,
}

pub type DiskResourceLoadCallback<'a> = &'a dyn Fn(LoadCallbackStage, usize, usize);
