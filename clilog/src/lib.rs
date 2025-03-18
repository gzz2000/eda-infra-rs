//! This crate provides a wrapper over `log` crate that allows you
//! to specify the type of messages and automatically suppress
//! types of messages that are overwhelmingly sent.
//!
//! Basic usage:
//! ```
//! clilog::info!(I01TEST, "test message");
//! ```
//! when message tagged `I01TEST` is sent over 20 times, a tip will be
//! printed and further such messages will be suppressed.
//!
//! At the end, you can optionally print a statistics of how many messages
//! are suppressed.
//! (TODO: not implemented yet.)

use std::sync::Mutex;
use std::collections::HashMap;
use std::sync::Once;

pub use log;
pub use paste;

pub use log::Level;

lazy_static::lazy_static! {
    /// this records all about print counts for typed messages.
    /// the first of the tuple indicates the number of invocations.
    /// the second one gives the limit.
    static ref PRINT_COUNT: Mutex<HashMap<(Level, &'static str), (u64, u64)>>
        = Mutex::new(HashMap::new());

    /// this records all prefixs for which timer output is enabled.
    static ref ENABLED_TIMER_PATHS: Mutex<Vec<String>>
        = Mutex::new(Vec::new());
}

/// The default max print count for all types of messages.
pub static mut DEFAULT_MAX_PRINT_COUNT: u64 = 5;

static LOGGING_INIT_ONCE: Once = Once::new();

/// convenient shortcut that you can call in your `main()`
/// to initialize logging globally with stderr color output.
///
/// if the logging is already initialized, it does nothing.
pub fn init_stderr_color_debug() {
    use simplelog::*;
    LOGGING_INIT_ONCE.call_once(|| {
        TermLogger::init(
            LevelFilter::Debug,
            ConfigBuilder::new()
                .set_location_level(LevelFilter::Debug)
                .set_thread_level(LevelFilter::Trace)
                .add_filter_ignore_str("rustyline")
                .build(),
            TerminalMode::Stderr,
            ColorChoice::Auto,
        ).unwrap();
    });
}

/// initialize simple verbose logging globally to stdout.
/// this is useful when calling logging in unittests,
/// and ensures logs suppression of passed tests.
///
/// log suppression is not currently working. waiting for the
/// simplelog crate upstream update.
///
/// if the logging is already initialized, it does nothing.
pub fn init_stdout_simple_trace() {
    use simplelog::*;
    LOGGING_INIT_ONCE.call_once(|| {
        TestLogger::init(
            LevelFilter::Trace,
            ConfigBuilder::new()
                .set_location_level(LevelFilter::Debug)
                .set_thread_level(LevelFilter::Trace)
                .add_filter_ignore_str("rustyline")
                .build()
        ).unwrap();
    });
}

/// Set max print count.
/// This is only effective if the type of message is not printed
/// before.
pub fn set_default_max_print_count(c: u64) {
    unsafe { DEFAULT_MAX_PRINT_COUNT = c; }
}

/// Set max print count of a specific kind of message.
pub fn set_max_print_count(typ: Level, id: &'static str, c: u64) {
    let mut print_counts = PRINT_COUNT.lock().unwrap();
    match print_counts.get_mut(&(typ, id)) {
        Some((_v, limit)) => {
            *limit = c;
        },
        None => {
            print_counts.insert((typ, id), (0, c));
        }
    }
}

/// get and increment by 1 the number of occurrence for a specific
/// kind of message, as well as its limits.
///
/// this is not intended to be used separately,
/// but is expected to be called from our macro expansion.
pub fn obtain_count_and_limit(typ: Level, id: &'static str) -> (u64, u64) {
    let mut print_counts = PRINT_COUNT.lock().unwrap();
    match print_counts.get_mut(&(typ, id)) {
        Some((v, limit)) => {
            *v += 1;
            (*v, *limit)
        },
        None => {
            let default_count = unsafe { DEFAULT_MAX_PRINT_COUNT };
            print_counts.insert((typ, id), (1, default_count));
            (1, default_count)
        }
    }
}

/// manually enable the timer for some module path prefix.
/// if an empty str is provided, it will effectively enable all timers.
pub fn enable_timer(prefix: impl Into<String>) {
    ENABLED_TIMER_PATHS.lock().unwrap().push(prefix.into());
}

/// check if the timer is enabled for some module path.
pub fn is_timer_enabled(cur: &str) -> bool {
    for prefix in ENABLED_TIMER_PATHS.lock().unwrap().iter() {
        if cur.starts_with(prefix) { return true }
    }
    false
}

/// general logging macro that can either log normally, or
/// check the count before logging.
/// this is the basis of other macros like `info`, `warn`, etc.
#[macro_export]
macro_rules! log_monitor {
    ($typ:ident, $id:ident, $fmt:expr $(,$param:expr)*) => {{
        let (count, limit) = $crate::obtain_count_and_limit(
            $crate::paste::paste!($crate::log::Level::[<$typ:camel>]),
            stringify!($id));
        if count <= limit {
            $crate::log::$typ!(concat!("(", stringify!($id), ") ", $fmt)
                               $(,$param)*);
        }
        if count == limit {
            $crate::log::$typ!(
                concat!("Further ",
                        // stringify will not work properly with paste inside.
                        stringify!($typ),
                        " (", stringify!($id), ") will be suppressed."));
        }
    }};
    ($typ:ident, $fmt:expr $(,$param:expr)*) => {{
        $crate::log::$typ!($fmt $(,$param)*);
    }}
}

// unstable yet:

// macro_rules! define_log {
//     ($($n:ident),+) => {$(
//         #[macro_export]
//         macro_rules! $n {
//             ($$($$p:tt),+) => ($crate::log_monitor!($n $$(,$$p)+))
//         }
//     )+}
// }

// define_log!(info, warn, error);

#[macro_export]
macro_rules! info {
    ($t:tt $(,$p:expr)*) => ($crate::log_monitor!(info, $t $(,$p)*))
}

#[macro_export]
macro_rules! warn {
    ($t:tt $(,$p:expr)*) => ($crate::log_monitor!(warn, $t $(,$p)*))
}

#[macro_export]
macro_rules! error {
    ($t:tt $(,$p:expr)*) => ($crate::log_monitor!(error, $t $(,$p)*))
}

#[macro_export]
macro_rules! debug {
    ($t:tt $(,$p:expr)*) => ($crate::log_monitor!(debug, $t $(,$p)*))
}

#[macro_export]
macro_rules! trace {
    ($t:tt $(,$p:expr)*) => ($crate::log_monitor!(trace, $t $(,$p)*))
}

mod logging_timer;
pub use logging_timer::*;
