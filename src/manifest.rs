use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::str::from_utf8;

use racros::AutoDebug;

use crate::constants::BUILD_SYSTEM_BUILD_DIR;
use crate::flatpak::types::{BuildOption, BuildSystem, ManifestSchema, Module};
use crate::{box_error, debug_println};

#[derive(AutoDebug)]
pub struct Manifest {
    pub root_dir: PathBuf,
    pub manifest: ManifestSchema,
    pub manifest_path: PathBuf,
    pub repo_dir: PathBuf,
    pub build_dir: PathBuf,
    pub state_dir: PathBuf,
    pub id: String,
}

impl Manifest {
    pub fn new(root_dir: PathBuf, manifest: ManifestSchema, manifest_path: PathBuf) -> Manifest {
        let build_dir = root_dir.clone().join(".flatpak");
        let repo_dir = build_dir.clone().join("repo");

        let state_dir = build_dir.join("flatpak-builder");

        // FIXME: manifest.id may be None.
        let id = manifest.id.clone().unwrap();

        Manifest {
            root_dir,
            manifest,
            manifest_path,
            repo_dir,
            build_dir,
            state_dir,
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
            eprintln!(
                "failed to build-init {}",
                from_utf8(cmd.stderr.as_ref()).unwrap()
            );
        }

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        if !self.root_dir.exists() {
            return false;
        }

        let metadata_file = self.repo_dir.clone().join("metadata");
        let files_dir = self.repo_dir.clone().join("files");
        let var_dir = self.repo_dir.clone().join("var");
        metadata_file.is_file() && files_dir.is_dir() && var_dir.is_dir()
    }

    pub fn update_dependencies(&self) -> Result<(), Box<dyn Error>> {
        let cmd = Command::new("flatpak-builder")
            .arg("--ccache")
            .arg("--force-clean")
            .arg("--disable-updates")
            .arg("--download-only")
            .arg(format!("--state-dir={}", self.state_dir.to_str().unwrap()).as_str())
            .arg(format!(
                "--stop-at={}",
                self.module().expect("no module found in manifest")
            ))
            .arg(self.repo_dir.to_str().unwrap())
            .arg(self.path())
            .output()?;

        if !cmd.status.success() {
            eprintln!(
                "failed to update dependencies {}",
                from_utf8(cmd.stderr.as_ref()).unwrap()
            );
        }

        Ok(())
    }

    fn module(&self) -> Option<&Module> {
        self.manifest.modules.last()
    }

    fn path(&self) -> &str {
        self.manifest_path.to_str().unwrap()
    }

    pub fn build_dependencies(&self) -> Result<(), Box<dyn Error>> {
        let cmd = Command::new("flatpak-builder")
            .arg("--ccache")
            .arg("--force-clean")
            .arg("--disable-updates")
            .arg("--download-only")
            .arg("--keep-build-dirs")
            .arg(format!("--state-dir={}", self.state_dir.to_str().unwrap()).as_str())
            .arg(format!(
                "--stop-at={}",
                self.module().expect("no module found in manifest")
            ))
            .arg(self.repo_dir.to_str().unwrap())
            .arg(self.path())
            .output()?;

        if !cmd.status.success() {
            eprintln!(
                "failed to build dependencies {}",
                from_utf8(cmd.stderr.as_ref()).unwrap()
            );
        }

        Ok(())
    }

    pub fn build(&self, rebuild: bool) -> Result<(), Box<dyn Error>> {
        let mut commands = self.setup_command(rebuild)?;
        for command in &mut commands {
            debug_println!("{:#?}", command);
            if !command.status()?.success() {
                return box_error!("failed to build");
            }
        }
        Ok(())
    }

    fn setup_command(&self, rebuild: bool) -> Result<Vec<Command>, Box<dyn Error>> {
        let collect_envs = |x: &HashMap<String, String>| -> Vec<String> {
            x.iter()
                .map(|(key, value)| format!("--env={}={}", key, value))
                .collect()
        };

        let manifest_envs = collect_envs(
            &self
                .manifest
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .env,
        );

        let module_envs = collect_envs(
            &self
                .module()
                .unwrap()
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .env,
        );

        let mut build_envs: Vec<String> = vec![];
        build_envs.extend(manifest_envs);
        build_envs.extend(module_envs);

        let build_args = vec![
            "--share=network".to_string(),
            format!("--filesystem={}", self.root_dir.to_str().unwrap()),
            format!("--filesystem={}", self.repo_dir.to_str().unwrap()),
        ];

        let mut config_opts: Vec<String> = vec![];
        config_opts.extend(
            self.module()
                .unwrap()
                .config_opts
                .to_owned()
                .unwrap_or_default(),
        );

        config_opts.extend(
            self.manifest
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .config_opts
                .to_owned()
                .unwrap_or_default(),
        );

        let build_system = self
            .module()
            .ok_or("module not found in manifest")?
            .build_system
            .as_ref()
            .ok_or("build-system not found in manifest module")?;

        debug_println!("build-system: {}", build_system.to_string());

        let command = match *build_system {
            BuildSystem::Autotools => self.get_autotools_commands(rebuild, build_args, config_opts),
            BuildSystem::Cmake | BuildSystem::CmakeNinja => {
                self.get_cmake_commands(rebuild, build_args, config_opts)
            }
            BuildSystem::Meson => self.get_meson_commands(rebuild, build_args, config_opts),
            BuildSystem::Simple => self.get_simple_commands(
                self.module()
                    .ok_or(
                        "failed to get build command: build-system is Simple but no modules found",
                    )?
                    .build_commands
                    .to_owned()
                    .unwrap_or_default(),
                build_args,
            ),
            BuildSystem::Qmake => {
                return box_error!("QMake build-system not implement yet");
            }
        };

        Ok(command)
    }

    fn get_autotools_commands(
        &self,
        rebuild: bool,
        build_args: Vec<String>,
        config_opts: Vec<String>,
    ) -> Vec<Command> {
        // Logical cpu count.
        let cpu_num = num_cpus::get();

        let mut commands: Vec<Command> = vec![];

        if !rebuild {
            let mut cmd = Command::new("flatpak");
            cmd.arg("build");
            build_args.iter().for_each(|x| _ = cmd.arg(x));
            cmd.arg(&self.repo_dir)
                .arg("./configure")
                .arg("--prefix=/app");
            config_opts.iter().for_each(|x| _ = cmd.arg(x));
            commands.push(cmd);
        }

        let mut make_cmd = Command::new("flatpak");
        make_cmd.arg("build");
        build_args.iter().for_each(|x| _ = make_cmd.arg(x));
        make_cmd
            .arg(&self.repo_dir)
            .arg("make")
            .arg("-p")
            .arg("-n")
            .arg("-s");
        commands.push(make_cmd);

        let mut make_install_cmd = Command::new("flatpak");
        make_install_cmd.arg("build");
        build_args.iter().for_each(|x| _ = make_install_cmd.arg(x));
        make_install_cmd
            .arg(&self.repo_dir)
            .arg("make")
            .arg("V=0")
            .arg(format!("-j{}", cpu_num))
            .arg("install");

        commands.push(make_install_cmd);

        commands
    }

    fn get_cmake_commands(
        &self,
        rebuild: bool,
        build_args: Vec<String>,
        config_opts: Vec<String>,
    ) -> Vec<Command> {
        let mut commands: Vec<Command> = vec![];
        let cmake_build_dir = BUILD_SYSTEM_BUILD_DIR;
        let cmake_build_full_dir =
            format!("{}/{}", self.root_dir.to_str().unwrap(), cmake_build_dir);

        let mut build_args: Vec<String> = build_args;
        build_args.push(format!("--filesystem={}", cmake_build_full_dir));

        if !rebuild {
            let mut cmd = Command::new("mkdir");
            cmd.arg("-p").arg(cmake_build_dir);
            commands.push(cmd);

            let mut cmake_cmd = Command::new("flatpak");
            cmake_cmd.arg("build");
            build_args.iter().for_each(|x| _ = cmake_cmd.arg(x));
            cmake_cmd
                .arg(&self.repo_dir)
                .arg("cmake")
                .arg("-G")
                .arg("Ninja")
                .arg("..")
                .arg(".")
                .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=1")
                .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
                .arg("-DCMAKE_INSTALL_PREFIX=/app");
            config_opts.iter().for_each(|x| _ = cmake_cmd.arg(x));
            cmake_cmd.current_dir(cmake_build_full_dir.clone());
            commands.push(cmake_cmd);
        }

        let mut cmake_build_cmd = Command::new("flatpak");
        cmake_build_cmd.arg("build");
        build_args.iter().for_each(|x| _ = cmake_build_cmd.arg(x));
        cmake_build_cmd.arg(&self.repo_dir).arg("ninja");
        cmake_build_cmd.current_dir(cmake_build_full_dir.clone());
        commands.push(cmake_build_cmd);

        let mut cmake_install_cmd = Command::new("flatpak");
        cmake_install_cmd.arg("build");
        build_args.iter().for_each(|x| _ = cmake_install_cmd.arg(x));
        cmake_install_cmd
            .arg(&self.repo_dir)
            .arg("ninja")
            .arg("install");
        cmake_install_cmd.current_dir(cmake_build_full_dir.clone());
        commands.push(cmake_install_cmd);

        commands
    }

    fn get_meson_commands(
        &self,
        rebuild: bool,
        build_args: Vec<String>,
        config_opts: Vec<String>,
    ) -> Vec<Command> {
        let mut commands: Vec<Command> = vec![];
        let meson_build_dir = BUILD_SYSTEM_BUILD_DIR;
        let meson_build_full_dir =
            format!("{}/{}", self.root_dir.to_str().unwrap(), meson_build_dir);

        let mut build_args: Vec<String> = build_args;
        build_args.push(format!("--filesystem={}", meson_build_full_dir));

        if !rebuild {
            let mut meson_cmd = Command::new("flatpak");
            meson_cmd.arg("build");
            build_args.iter().for_each(|x| _ = meson_cmd.arg(x));
            meson_cmd
                .arg(&self.repo_dir)
                .arg("meson")
                .arg("--prefix")
                .arg("/app")
                .arg(meson_build_dir);
            config_opts.iter().for_each(|x| _ = meson_cmd.arg(x));
            commands.push(meson_cmd);
        }

        let mut meson_build_cmd = Command::new("flatpak");
        meson_build_cmd.arg("build");
        build_args.iter().for_each(|x| _ = meson_build_cmd.arg(x));
        meson_build_cmd
            .arg(&self.repo_dir)
            .arg("ninja")
            .arg("-C")
            .arg(meson_build_dir);
        commands.push(meson_build_cmd);

        let mut meson_install_cmd = Command::new("flatpak");
        meson_install_cmd.arg("build");
        build_args.iter().for_each(|x| _ = meson_install_cmd.arg(x));
        meson_install_cmd
            .arg(&self.repo_dir)
            .arg("ninja")
            .arg("install")
            .arg("-C")
            .arg(meson_build_dir);
        commands.push(meson_install_cmd);

        commands
    }

    fn get_simple_commands(
        &self,
        build_commands: Vec<String>,
        build_args: Vec<String>,
    ) -> Vec<Command> {
        build_commands
            .iter()
            .map(|x| {
                let mut command = Command::new("flatpak");
                command.arg("build");
                build_args.iter().for_each(|x| _ = command.arg(x));
                command.arg(&self.repo_dir);
                x.split(' ').for_each(|xx| _ = command.arg(x));
                command
            })
            .collect()
    }
}
