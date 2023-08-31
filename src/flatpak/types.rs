use racros::{AutoDebug, AutoStr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(AutoDebug, Deserialize, Serialize)]
pub struct BuildOption {
    #[serde(rename = "build-args")]
    pub build_args: Vec<String>,
    #[serde(rename = "append-path")]
    pub append_path: Option<String>,
    #[serde(rename = "prepend-path")]
    pub prepend_path: Option<String>,
    #[serde(rename = "append-ld-library-path")]
    pub append_ld_library_path: Option<String>,
    #[serde(rename = "prepend-ld-library-path")]
    pub prepend_ld_library_path: Option<String>,
    #[serde(rename = "append-pkg-config-path")]
    pub append_pkg_config_path: Option<String>,
    #[serde(rename = "prepend-pkg-config-path")]
    pub prepend_pkg_config_path: Option<String>,
    pub env: HashMap<String, String>,
    #[serde(rename = "config-opts")]
    pub config_opts: Vec<String>,
}

#[derive(AutoDebug, AutoStr, Deserialize, Serialize)]
#[autorule = "lowercase"]
pub enum BuildSystem {
    Meson,
    Cmake,
    #[str("cmake-ninja")]
    #[serde(rename = "cmake-ninja")]
    CmakeNinja,
    Simple,
    Autotools,
    Qmake,
}

#[derive(AutoDebug, AutoStr, Deserialize, Serialize)]
#[autorule = "lowercase"]
pub enum SourceType {
    Archive,
    Git,
    Bzr,
    Svn,
    Dir,
    File,
    Script,
    Inline,
    Shell,
    Patch,
    #[str("extra-data")]
    #[serde(rename = "extra-data")]
    ExtraData,
}

#[derive(AutoDebug, Deserialize, Serialize)]
pub struct Source {
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub url: Option<String>,
    pub path: Option<String>,
    pub tag: Option<String>,
    pub commit: Option<String>,
    pub sha256: Option<String>,
}

#[derive(AutoDebug, Deserialize, Serialize)]
pub struct Module {
    pub name: String,
    #[serde(rename = "buildsystem")]
    pub build_system: Option<BuildSystem>,
    #[serde(rename = "config-opts")]
    pub config_opts: Option<Vec<String>>,
    pub sources: Vec<Source>,
    #[serde(rename = "build-commands")]
    pub build_commands: Vec<String>,
    #[serde(rename = "build-options")]
    pub build_options: Option<Vec<BuildOption>>,
    #[serde(rename = "post-install")]
    pub post_install: Option<Vec<String>>,
}

#[derive(AutoDebug, Deserialize, Serialize)]
pub struct ManifestSchema {
    pub id: Option<String>,
    pub branch: Option<String>,
    #[serde(rename = "app-id")]
    pub app_id: String,
    pub modules: Vec<Module>,
    pub sdk: String,
    pub runtime: String,
    #[serde(rename = "runtime-version")]
    pub runtime_version: String,
    #[serde(rename = "sdk-extensions")]
    pub sdk_extensions: Option<Vec<String>>,
    pub command: String,
    #[serde(rename = "finish-args")]
    pub finish_args: Vec<String>,
    #[serde(rename = "build-options")]
    pub build_options: Option<Vec<BuildOption>>,
    #[serde(rename = "x-run-args")]
    pub x_run_args: Option<Vec<String>>,
}

#[derive(AutoStr, Deserialize, Serialize)]
#[autorule = "lowercase"]
pub enum SdkExtension {
    Vala,
    Rust,
}
