use crate::full_println;
use std::env;
use std::sync::Mutex;

static ADDON_ENV_PREFIX: Mutex<String> = Mutex::new(String::new());

pub fn set_override_env_prefix(prefix: &str) {
    let mut lock = ADDON_ENV_PREFIX.lock().unwrap();
    *lock = prefix.to_owned();
    drop(lock);
}

pub fn load_envs_from_os() -> Vec<String> {
    let lock = ADDON_ENV_PREFIX.lock().unwrap();
    let prefix = (*lock).clone();
    drop(lock);

    if prefix.is_empty() {
        return vec![];
    }

    let vars: Vec<String> = env::vars()
        .filter(|(name, _)| name.starts_with(prefix.as_str()))
        .map(|(name, value)| format!("--env={name}={value}"))
        .collect();

    full_println!("override envs from host: {vars:#?}",);

    vars
}
