use crate::box_error;
use crate::flatpak::types::ManifestSchema;
use std::error::Error;
use std::path::PathBuf;

pub fn find_manifest_and_parse(
    root_directory: Option<PathBuf>,
) -> Result<ManifestSchema, Box<dyn Error>> {
    let work_directory = root_directory.unwrap_or(
        std::env::current_dir()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
            .parse::<PathBuf>()?,
    );
    box_error!("current directory: {}", work_directory.to_str().unwrap())
}
