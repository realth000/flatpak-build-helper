#[allow(dead_code)]

pub static APP_LOG_VAR: &str = "FBH_LOG";
pub static BUILD_SYSTEM_BUILD_DIR: &str = "_build";

pub static SYSTEM_FONTS_DIR: &str = "/usr/share/fonts";
pub static SYSTEM_LOCAL_FONT_DIR: &str = "/usr/share/local/fonts";

pub static SYSTEM_FONT_CACHE_DIRS: [&str; 2] =
    ["/usr/lib/fontconfig/cache", "/var/cache/fontconfig"];

pub static FONT_DIR_CONTENT_HEADER: &str = r#"<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts:dtd">
<fontconfig>"#;

include!(concat!(env!("OUT_DIR"), "/constants.generated.rs"));
