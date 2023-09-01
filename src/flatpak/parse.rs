use std::error::Error;
use std::fs::read_to_string;
use std::path::PathBuf;

use crate::flatpak::types::ManifestSchema;
use crate::manifest::Manifest;
use crate::{box_error, full_println};

pub fn find_manifest_and_parse(
    root_directory: Option<PathBuf>,
) -> Result<Manifest, Box<dyn Error>> {
    let work_directory = root_directory.unwrap_or(
        std::env::current_dir()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
            .parse::<PathBuf>()?,
    );

    let mut check_directory = work_directory.clone();

    check_directory.push("build-aux");

    if !check_directory.exists() {
        return box_error!(
            "directory build-aux not found in {}",
            check_directory.to_str().unwrap()
        );
    }

    if !check_directory.is_dir() {
        return box_error!(
            "not a directory: {}/build-aux",
            check_directory.to_str().unwrap()
        );
    }

    let manifest_path = check_directory
        .read_dir()
        .unwrap()
        .find(|x| {
            full_println!("check path: {:#?}", x);
            x.is_ok()
                && x.as_ref().unwrap().path().is_file()
                && x.as_ref()
                    .unwrap()
                    .path()
                    .to_str()
                    .unwrap()
                    .ends_with(".Devel.json")
        })
        .ok_or(format!(
            "*.Devel.json not found in {}",
            check_directory.to_str().unwrap(),
        ))?
        .unwrap()
        .path();

    let manifest_data = read_to_string(manifest_path.clone())?;

    let schema: ManifestSchema = serde_json::from_str(manifest_data.as_str())?;

    Ok(Manifest::new(work_directory, schema, manifest_path))
}
