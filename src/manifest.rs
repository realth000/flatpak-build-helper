use std::collections::HashMap;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::str::from_utf8;

use libc::getuid;
use racros::AutoDebug;
use regex::Regex;

use crate::constants::{
    BUILD_SYSTEM_BUILD_DIR, FONT_DIR_CONTENT_HEADER, SYSTEM_FONTS_DIR, SYSTEM_FONT_CACHE_DIRS,
    SYSTEM_LOCAL_FONT_DIR,
};
use crate::flatpak::types::{BuildOption, BuildSystem, ManifestSchema, Module};
use crate::util::{
    get_host_envs, get_user_cache_dir, get_user_fonts_cache_dir, get_user_fonts_dir,
};
use crate::{box_error, debug_println, full_println};

static ENV_NAME_LIT: [&str; 11] = [
    "COLORTERM",
    "DESKTOP_SESSION",
    "LANG",
    "WAYLAND_DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_SEAT",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_ID",
    "XDG_SESSION_TYPE",
    "XDG_VTNR",
    "AT_SPI_BUS_ADDRESS",
];

/// Combine environment variables from manifest schema, host env, and default values.
///
/// e.g. For `PATH` env, add the following env string to `ret` arg:
/// "--env=PATH=${prepend env from schema}:${host env}:${default env value}:${append env from schema}"
macro_rules! override_env {
    ($ret: tt, $manifest: tt, $env_name: tt, $default_value: ident, $prepend_ident: ident, $append_ident: ident) => {
        let module = $manifest.module().unwrap();
        let prepend_env_list = vec![
            $manifest
                .manifest
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .$prepend_ident
                .clone()
                .unwrap_or_default(),
            module
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .$prepend_ident
                .clone()
                .unwrap_or_default(),
        ];

        let append_env_list = vec![
            $manifest
                .manifest
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .$append_ident
                .clone()
                .unwrap_or_default(),
            module
                .build_options
                .as_ref()
                .unwrap_or(&BuildOption::default())
                .$append_ident
                .clone()
                .unwrap_or_default(),
        ];

        let host_env = std::env::var($env_name).unwrap_or_default();

        let mut all = vec![];
        all.extend(prepend_env_list);
        all.push(host_env);
        all.extend($default_value);
        all.extend(append_env_list);
        all.retain(|x| !&x.is_empty());
        $ret.push(format!("--env={}={}", $env_name, all.join(":")));
    };
}

#[derive(AutoDebug)]
pub struct Manifest {
    pub root_dir: PathBuf,
    pub manifest: ManifestSchema,
    pub manifest_path: PathBuf,
    pub repo_dir: PathBuf,
    pub build_dir: PathBuf,
    pub state_dir: PathBuf,
    pub id: String,

    fonts_args: Vec<String>,
    a11y_bus_args: Vec<String>,
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
            fonts_args: vec![],
            a11y_bus_args: vec![],
        }
    }

    pub fn init_build(&self) -> Result<(), Box<dyn Error>> {
        // flatpak build-init $RepoDir $Id $Sdk $Runtime $runtimeVersion
        let mut cmd = Command::new("flatpak");

        cmd.arg("build-init")
            .arg(self.repo_dir.to_str().unwrap())
            .arg(&self.id)
            .arg(&self.manifest.sdk)
            .arg(&self.manifest.runtime)
            .arg(&self.manifest.runtime_version);

        full_println!("initialize command: {:#?}", cmd);

        let cmd_output = cmd.output()?;
        print!("{}", from_utf8(cmd_output.stdout.as_ref()).unwrap());

        if !cmd_output.status.success() {
            eprintln!(
                "failed to build-init {}",
                from_utf8(cmd_output.stderr.as_ref()).unwrap()
            );
        }

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        if !self.root_dir.exists() {
            full_println!(
                "initialize check not passed: root_dir not exists: {}",
                self.root_dir.to_str().unwrap()
            );
            return false;
        }

        let metadata_file = self.repo_dir.clone().join("metadata");
        let files_dir = self.repo_dir.clone().join("files");
        let var_dir = self.repo_dir.clone().join("var");

        full_println!(
            "metadata: {}, files: {}, var: {}",
            metadata_file.is_file(),
            files_dir.is_dir(),
            var_dir.is_dir()
        );

        metadata_file.is_file() && files_dir.is_dir() && var_dir.is_dir()
    }

    pub fn update_dependencies(&self) -> Result<(), Box<dyn Error>> {
        let mut cmd = Command::new("flatpak-builder");
        cmd.arg("--ccache")
            .arg("--force-clean")
            .arg("--disable-updates")
            .arg("--download-only")
            .arg(format!("--state-dir={}", self.state_dir.to_str().unwrap()).as_str())
            .arg(format!(
                "--stop-at={}",
                self.module().expect("no module found in manifest")
            ))
            .arg(self.repo_dir.to_str().unwrap())
            .arg(self.path());

        full_println!("update dependencies command: {:#?}", cmd);

        let cmd_output = cmd.output()?;

        print!("{}", from_utf8(cmd_output.stdout.as_ref()).unwrap());

        if !cmd_output.status.success() {
            eprintln!(
                "failed to update dependencies {}",
                from_utf8(cmd_output.stderr.as_ref()).unwrap()
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
        let mut cmd = Command::new("flatpak-builder");
        cmd.arg("--ccache")
            .arg("--force-clean")
            .arg("--disable-updates")
            .arg("--disable-download")
            .arg("--build-only")
            .arg("--keep-build-dirs")
            .arg(format!("--state-dir={}", self.state_dir.to_str().unwrap()).as_str())
            .arg(format!(
                "--stop-at={}",
                self.module().expect("no module found in manifest")
            ))
            .arg(self.repo_dir.to_str().unwrap())
            .arg(self.path());

        let cmd_output = cmd.output()?;

        full_println!("build dependencies command: {:#?}", cmd);

        print!("{}", from_utf8(cmd_output.stdout.as_ref()).unwrap());

        if !cmd_output.status.success() {
            eprintln!(
                "failed to build dependencies {}",
                from_utf8(cmd_output.stderr.as_ref()).unwrap()
            );
        }

        Ok(())
    }

    pub fn build(&self, rebuild: bool) -> Result<(), Box<dyn Error>> {
        debug_println!("setup command...");
        let mut commands = self.setup_command(rebuild)?;
        debug_println!("running build commands");
        for command in &mut commands {
            debug_println!("{:#?}", command);
            if !command.status()?.success() {
                return box_error!("failed to build");
            }
        }
        debug_println!("build success");
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

        let mut build_args = vec![
            "--share=network".to_string(),
            format!("--filesystem={}", self.root_dir.to_str().unwrap()),
            format!("--filesystem={}", self.repo_dir.to_str().unwrap()),
        ];
        build_args.extend(build_envs);

        build_args.extend(self.get_envs());

        // Need these?
        // build_args.push(host_var_path);
        // build_args.push("--env=LD_LIBRARY_PATH=/app/lib/".to_string());
        // build_args.push("--env=PKG_CONFIG_PATH=/app/lib/pkgconfig:/app/share/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig".to_string());

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

        let commands = match *build_system {
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

        debug_println!("build commands count: {}", commands.len());
        full_println!("build commands: {:#?}", commands);
        Ok(commands)
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
                x.split(' ').for_each(|xx| _ = command.arg(xx));
                command
            })
            .collect()
    }

    fn get_envs(&self) -> Vec<String> {
        let mut envs = vec![];

        let default_path = vec!["/app/bin".to_string(), "/usr/bin".to_string()];
        override_env!(envs, self, "PATH", default_path, prepend_path, append_path);
        let default_ld_library_path = vec!["/app/lib".to_string()];
        override_env!(
            envs,
            self,
            "LD_LIBRARY_PATH",
            default_ld_library_path,
            prepend_ld_library_path,
            append_ld_library_path
        );
        let default_pkg_config_path = vec![
            "/app/lib/pkgconfig".to_string(),
            "/app/share/pkgconfig".to_string(),
            "/usr/lib/pkgconfig".to_string(),
            "/usr/share/pkgconfig".to_string(),
        ];
        override_env!(
            envs,
            self,
            "PKG_CONFIG_PATH",
            default_pkg_config_path,
            prepend_pkg_config_path,
            append_pkg_config_path
        );

        envs
    }
}

/// Implement run
impl Manifest {
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let uid = Manifest::get_uid();
        let app_id = self.id.as_str();
        if self.fonts_args.is_empty() {
            self.fonts_args = self.get_fonts_args()?;
        }

        if self.a11y_bus_args.is_empty() {
            self.a11y_bus_args = self.get_a11y_bus_args()?;
        }

        let mut args: Vec<String> = vec![
            "build".to_string(),
            "--with-appdir".to_string(),
            "--allow=devel".to_string(),
            format!(
                "--bind-mount=/run/user/{}/doc=/run/user/{}/doc/by-app/{}",
                uid, uid, app_id
            ),
        ];

        args.extend(self.get_finish_args());
        args.extend([
            "--talk-name=org.freedesktop.portal.*".to_string(),
            "--talk-name=org.a11y.Bus".to_string(),
        ]);

        args.extend(self.a11y_bus_args.to_owned());

        args.extend(
            get_host_envs(ENV_NAME_LIT)
                .iter()
                .map(|(key, value)| format!("--env={}={}", key, value))
                .collect::<Vec<String>>(),
        );

        // TODO: Check mountExtensions?
        args.push("--share=network".to_string());

        args.extend(self.fonts_args.to_owned());
        args.push(self.repo_dir.to_str().unwrap().to_string());

        // TODO: Customize shellCommand
        args.push(self.manifest.command.to_string());

        let mut command = Command::new("flatpak");
        command.args(args);

        debug_println!("start running");
        full_println!("{:#?}", command);

        let output = command.output()?;

        if !output.status.success() {
            println!("{}", String::from_utf8(output.stderr).unwrap());
            return box_error!("error running command");
        }

        println!("{}", String::from_utf8(output.stdout).unwrap());

        Ok(())
    }

    fn get_finish_args(&self) -> Vec<String> {
        // Ugly, do not have a &&str, so use any() instead of contains()
        self.manifest
            .finish_args
            .iter()
            .filter(|x| {
                !["--metadata", "--require-version"]
                    .iter()
                    .any(|xx| *xx == x.split_once('=').unwrap_or(("", "")).0)
            })
            .map(|x| x.to_string())
            .collect()
    }

    fn get_uid() -> u32 {
        unsafe { getuid() }
    }

    fn get_fonts_args(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let mut fonts_args = vec![];
        let mapped_font_file = get_user_cache_dir().join("font-dirs.xml");
        let mut font_dir_content = String::from(FONT_DIR_CONTENT_HEADER);

        // TODO: Handle sandbox?

        if PathBuf::from(SYSTEM_FONTS_DIR).exists() {
            fonts_args.push(format!("--bind-mount=/run/host/fonts={}", SYSTEM_FONTS_DIR));
            font_dir_content.push_str(
                format!(
                    "\t<remap-dir as-path={}>/run/host/fonts/</remap-dir>\n",
                    SYSTEM_FONTS_DIR,
                )
                .as_str(),
            );
        }

        if PathBuf::from(SYSTEM_LOCAL_FONT_DIR).exists() {
            fonts_args.push(format!(
                "--bind-mount=/run/host/local-fonts={}",
                SYSTEM_LOCAL_FONT_DIR
            ));
            font_dir_content.push_str(
                format!(
                    "\t<remap-dir as-path={}>/run/host/fonts/</remap-dir>\n",
                    SYSTEM_LOCAL_FONT_DIR,
                )
                .as_str(),
            )
        }

        SYSTEM_FONT_CACHE_DIRS
            .iter()
            .filter(|x| PathBuf::from(x).exists())
            .for_each(|x| fonts_args.push(format!("--bind-mount=/run/host/local-fonts={}", x)));

        get_user_fonts_dir()
            .iter()
            .filter(|x| PathBuf::from(x).exists())
            .map(|x| x.to_str().unwrap())
            .for_each(|x| {
                fonts_args.push(format!("--filesystem={};ro", x));
                font_dir_content.push_str(
                    format!("\t<remap-dir as-path={}>/run/host/fonts/</remap-dir>\n", x).as_str(),
                );
            });

        let user_fonts_cache_dir = get_user_fonts_cache_dir();
        if user_fonts_cache_dir.exists() {
            fonts_args.push(format!(
                "--filesystem={};ro",
                user_fonts_cache_dir.to_str().unwrap()
            ));
            fonts_args.push(format!(
                "--bind-mount=/run/host/user-fonts-cache={}",
                user_fonts_cache_dir.to_str().unwrap()
            ));
        }

        font_dir_content.push_str("</fontconfig>\n");
        fonts_args.push(format!(
            "--bind-mount=/run/host/font-dirs.xml={}",
            mapped_font_file.to_str().unwrap()
        ));

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(mapped_font_file)?;

        file.write_all(&font_dir_content.into_bytes())?;

        Ok(fonts_args)
    }

    fn get_a11y_bus_args(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let mut bus_args = vec![];
        // Get from gdbus
        let gdbus_output = Command::new("gdbus")
            .arg("call")
            .arg("--session")
            .arg("--dest=org.a11y.Bus")
            .arg("--object-path=/org/a11y/bus")
            .arg("--method=org.a11y.Bus.GetAddress")
            .output()?;
        if !gdbus_output.status.success() {
            println!("{}", String::from_utf8(gdbus_output.stderr).unwrap());
            return box_error!("failed to get a11y dbus args when running gdbus command");
        }

        // TODO: Trim each line?
        // ('unix:path=/run/user/1000/at-spi/bus_0,guid=xxx123xxx',)
        let dbus_output = String::from_utf8(gdbus_output.stdout)
            .unwrap()
            .replace("',(", "")
            .replace("',)", "");

        // TODO: Handle situation without suffix part.
        let re = Regex::new(r"^.*unix:path=(?<unix_path>[^,]+),(?<suffix>[0-9a-z=]+).*\n$")?;
        match re.captures(dbus_output.as_str()) {
            Some(v) => {
                let unix_path = v.name("unix_path").unwrap().as_str().to_string();
                let suffix = v.name("suffix").unwrap().as_str().to_string();
                bus_args.push(format!(
                    "--bind-mount=/run/flatpak/at-spi-bus={}",
                    unix_path
                ));
                bus_args.push(format!(
                    "--env=AT_SPI_BUS_ADDRESS=unix:path=/run/flatpak/at-spi-bus{}",
                    suffix
                ));
            }
            None => return box_error!("failed to parse a11y gdbus address: {}", dbus_output),
        }

        Ok(bus_args)
    }
}
