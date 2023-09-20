use std::env;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(clippy::too_many_lines)]
fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let git_commit_time = run_command(
        "git",
        [
            "--no-pager",
            "show",
            "--oneline",
            "--format=%cd",
            "--date=format:%F",
            "-s",
            "HEAD",
        ],
    )
    .0;

    let git_commit_time_long = run_command(
        "git",
        [
            "--no-pager",
            "show",
            "--oneline",
            "--format=%cd",
            "--date=format:%F %T %z",
            "-s",
            "HEAD",
        ],
    )
    .0;

    let git_commit_revision_long = run_command(
        "git",
        [
            "--no-pager",
            "show",
            "--oneline",
            "--format=%H",
            "-s",
            "HEAD",
        ],
    )
    .0;

    let git_commit_revision = run_command(
        "git",
        [
            "--no-pager",
            "show",
            "--oneline",
            "--format=%h",
            "-s",
            "HEAD",
        ],
    )
    .0;

    let git_tag_data = run_command("git", ["describe", "--abbrev=0", "--tags"]).0;

    let git_tag = if git_tag_data.is_empty() {
        String::from("0.0.0")
    } else {
        String::from_utf8(git_tag_data).unwrap()
    };

    let mut data = String::new();
    data.push_str(make_trimmed_str_var_from_bytes("GIT_COMMIT_TIME", git_commit_time).as_str());
    data.push_str(
        make_trimmed_str_var_from_bytes("GIT_COMMIT_TIME_LONG", git_commit_time_long).as_str(),
    );
    data.push_str(
        make_trimmed_str_var_from_bytes("GIT_COMMIT_REVISION", git_commit_revision).as_str(),
    );
    data.push_str(
        make_trimmed_str_var_from_bytes("GIT_COMMIT_REVISION_LONG", git_commit_revision_long)
            .as_str(),
    );
    data.push_str(make_trimmed_str_var("GIT_TAG_VERSION", git_tag).as_str());

    let dst_path = PathBuf::from(out_dir).join("constants.generated.rs");
    generate_file(dst_path, data);
}

fn generate_file<P: AsRef<Path>, D: AsRef<[u8]>>(path: P, data: D) {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap();
    f.write_all(data.as_ref()).unwrap();
}

fn run_command<S, I>(command: &str, args: I) -> (Vec<u8>, Vec<u8>)
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = S>,
{
    let cmd_output = Command::new(command).args(args).output().unwrap();
    let cmd_stdout = cmd_output.stdout;
    let cmd_stderr = cmd_output.stderr;
    (cmd_stdout, cmd_stderr)
}

fn make_trimmed_str_var(name: &str, value: String) -> String {
    format!(
        "pub const {name}: &'static str = \"{}\";\n",
        value.trim_matches(|c| [' ', '\t', '"', '\'', '\r', '\n', '`'].contains(&c))
    )
}

fn make_trimmed_str_var_from_bytes(name: &str, value: Vec<u8>) -> String {
    let str = String::from_utf8(value).unwrap();
    make_trimmed_str_var(name, str)
}
