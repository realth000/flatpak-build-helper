use std::error::Error;
use std::fs::read_to_string;
use std::path::PathBuf;

use crate::flatpak::types::ManifestSchema;
use crate::{box_error, full_println};

pub fn find_manifest_and_parse(
    root_directory: Option<PathBuf>,
) -> Result<ManifestSchema, Box<dyn Error>> {
    let mut work_directory = root_directory.unwrap_or(
        std::env::current_dir()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
            .parse::<PathBuf>()?,
    );

    work_directory.push("build-aux");

    if !work_directory.exists() {
        return box_error!(
            "directory build-aux not found in {}",
            work_directory.to_str().unwrap()
        );
    }

    if !work_directory.is_dir() {
        return box_error!(
            "not a directory: {}/build-aux",
            work_directory.to_str().unwrap()
        );
    }

    let manifest_path = work_directory
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
            work_directory.to_str().unwrap(),
        ))?
        .unwrap()
        .path();

    let manifest_data = read_to_string(manifest_path)?;

    let schema: ManifestSchema = serde_json::from_str(manifest_data.as_str())?;

    Ok(schema)
}
