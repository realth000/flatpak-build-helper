use std::env::set_var;
use std::error::Error;
use std::path::PathBuf;

use clap::ArgAction::Count;
use clap::{Arg, ArgMatches, Command};

use crate::constants::APP_LOG_VAR;
use crate::flatpak::parse::find_manifest_and_parse;

mod constants;
mod flatpak;
mod manifest;
mod util;

fn main() -> Result<(), Box<dyn Error>> {
    let build_command = Command::new("build").about("build package");
    let run_command = Command::new("run").about("run package");

    let mut command = Command::new("fbh")
        .about("flatpak-build-helper")
        .version("0.1.0")
        .subcommand(build_command)
        .subcommand(run_command)
        .arg(Arg::new("root-dir").index(1).global(true))
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help(format!(
                    "verbose output, -v or -vv or set {} to 1/2/full",
                    APP_LOG_VAR
                ))
                .global(true)
                .action(Count),
        );

    let command_matches = command.clone().get_matches();

    match command_matches.get_count("verbose") {
        1 => set_var(APP_LOG_VAR, "1"),
        2 => set_var(APP_LOG_VAR, "2"),
        _ => {}
    }

    match command_matches.subcommand() {
        Some(("build", args)) => handle_build_command(args),
        Some(("run", args)) => handle_run_command(args),
        _ => {
            command.print_help()?;
            Ok(())
        }
    }
}

fn handle_build_command(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let root_dir = args.get_one::<String>("root-dir").map(PathBuf::from);
    let schema = find_manifest_and_parse(root_dir)?;

    full_println!("build command, schema: {:#?}", schema);

    debug_println!("check initialization");
    if !schema.is_initialized() {
        debug_println!("running build-init");
        schema.init_build()?;
    } else {
        debug_println!("skip build-init: already initialized");
    }

    // TODO: Check already build

    debug_println!("updating dependencies");
    schema.update_dependencies()?;

    debug_println!("building dependencies");
    schema.build_dependencies()?;

    debug_println!("building targets");
    // TODO: Handle rebuild
    schema.build(false)?;

    Ok(())
}

fn handle_run_command(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let root_dir = args.get_one::<String>("root-dir").map(PathBuf::from);
    let mut schema = find_manifest_and_parse(root_dir)?;
    schema.run()
}
