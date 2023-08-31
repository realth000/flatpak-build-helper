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
