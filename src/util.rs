use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

#[macro_export]
macro_rules! box_error {
    ($($arg:tt)*) => {Err(Box::<dyn Error>::from(format!($($arg)*)))};
}

// Log level:
// 0 default
// 1 more log
// full full log
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        match std::env::var("FBH_LOG") {
            std::result::Result::Ok(v) => match v.as_str() {
                "1" | "2" | "full" => println!($($arg)*),
                _ => {},
            }
            std::result::Result::Err(_) => {},
        }
    }
}

#[macro_export]
macro_rules! full_println {
    ($($arg:tt)*) => {
        match std::env::var("FBH_LOG") {
            std::result::Result::Ok(v) => match v.as_str() {
                "2" | "full" => println!($($arg)*) ,
                _ => {},
            }
            std::result::Result::Err(_) => {},
        }
    }
}

pub fn get_user_fonts_cache_dir() -> PathBuf {
    get_user_cache_dir().join("fontconfig")
}

pub fn get_user_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or(
        dirs::home_dir()
            .expect("failed to get user data dir")
            .join(".cache"),
    )
}

pub fn get_user_cache_dir() -> PathBuf {
    // TODO: Need handle sandbox?
    dirs::cache_dir().unwrap_or(
        dirs::home_dir()
            .expect("failed to get user cache dir")
            .join(".cache"),
    )
}

pub fn get_user_fonts_dir() -> Vec<PathBuf> {
    vec![
        get_user_data_dir().join("fonts"),
        dirs::home_dir().unwrap().join("fonts"),
    ]
}
pub fn get_host_envs<'a, T: AsRef<[&'a str]> + Sized>(env_name_list: T) -> HashMap<String, String> {
    env_name_list
        .as_ref()
        .iter()
        .map(|x| (x.to_string(), env::var(x).unwrap_or_default()))
        .filter(|(_, x)| !x.is_empty())
        .collect::<HashMap<String, String>>()
}
