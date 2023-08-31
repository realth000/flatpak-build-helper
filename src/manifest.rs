use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::str::from_utf8;

use racros::AutoDebug;

use crate::flatpak::types::ManifestSchema;

#[derive(AutoDebug)]
pub struct Manifest {
    pub root_dir: PathBuf,
    pub manifest: ManifestSchema,
    pub repo_dir: PathBuf,
    pub id: String,
}

impl Manifest {
    pub fn new(root_dir: PathBuf, manifest: ManifestSchema) -> Manifest {
        let mut repo_dir = root_dir.clone();
        repo_dir.push(".flatpak");
        repo_dir.push("repo");

        // FIXME: manifest.id may be None.
        let id = manifest.id.clone().unwrap();

        Manifest {
            root_dir,
            manifest,
            repo_dir,
            id,
        }
    }

    pub fn init_build(&self) -> Result<(), Box<dyn Error>> {
        // flatpak build-init $RepoDir $Id $Sdk $Runtime $runtimeVersion
        let cmd = Command::new("flatpak")
            .arg("build-init")
            .arg(self.repo_dir.to_str().unwrap())
            .arg(&self.id)
            .arg(&self.manifest.sdk)
            .arg(&self.manifest.runtime)
            .arg(&self.manifest.runtime_version)
            .output()?;

        if !cmd.status.success() {
            eprintln!("{}", from_utf8(cmd.stderr.as_ref()).unwrap());
        }

        Ok(())
    }
}
