use std::{cell::Cell, sync::Mutex};

pub const ERROR: usize = 0;
pub const WARN: usize = 1;
pub const INFO: usize = 2;
pub const DEBUG: usize = 3;

/// Current log level.
pub static LEVEL: Mutex<Cell<usize>> = Mutex::new(Cell::new(ERROR));

/// Compile-time constant to disable most debugging output.
/// This is useful since this logging has a significant runtime cost (!)
/// and can interfere with benchmarking.
pub const HARD_QUIET: bool = true;

pub fn set_level(new_level: usize) {
    LEVEL.lock().unwrap().set(new_level);
}

#[macro_export]
macro_rules! info {
    ($($x:tt)*) => {
        if !crate::util::HARD_QUIET {
            if crate::util::LEVEL.lock().unwrap().get() >= crate::util::INFO {
                println!($($x)*)
            }
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($x:tt)*) => {
        if !crate::util::HARD_QUIET {
            if crate::util::LEVEL.lock().unwrap().get() >= crate::util::DEBUG {
                println!($($x)*)
            }
        }
    };
}
