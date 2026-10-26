#![deny(unsafe_code)]

mod config;
mod error;
mod fragment;
mod pica;
mod profile;
mod settings;

#[allow(unsafe_code)]
mod abi;

pub use config::{ConfigLayout, FsConfig};
pub use error::{Result, ShaderError};
pub use profile::Profile;
pub use settings::FragmentSettings;

pub fn generate_fragment_shader(config: &[u8], profile: Profile) -> Result<Vec<u32>> {
    let config = FsConfig::from_bytes(config)?;
    fragment::generate_fragment_shader(config, profile)
}
