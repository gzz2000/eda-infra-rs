//! This source is adapted from https://github.com/PhilipDaniels/logging_timer/blob/master/src/lib.rs.
//! Copyright (C) 2019, PhilipDaniels, MIT License
//!
//! It is modified by introducing crate-based filters.
//! 
//! This crate provides a couple of simple timers that log messages indicating the elapsed
//! time between their creation and dropping. Messages are output via the
//! [log](https://crates.io/crates/log) crate.
//!
//! Timers have names, and the log messages are constructed in such a way that they contain
//! the module, filename and line number of the place where the timer was constructed.
//!
//! # Using the Timer Attributes
//!
//! The simplest way to get started is to use one of the two attributes `time` or `stime` to
//! instrument a function, the name of the function is used as the name of the timer:
//!
//! ```text
//! use logging_timer::{time, stime};
//!
//! #[time]
//! fn find_files(dir: PathBuf) -> Vec<PathBuf> {
//!     let files = vec![];
//!
//!     // expensive operation here
//!
//!     return files;
//! } // 'TimerFinished' message is logged here
//! ```
//!
//! Both attributes accept two optional arguments, to specify the log level and a pattern
//! into which the name of the function will be substituted. The pattern is helpful to
//! disambiguate functions when you have many in the same module with the same name: `new`
//! might occur many times on different structs, for example.
//!
//! ```text
//! #[time]                                 // Use default log level of Debug
//! #[time("info")]                         // Set custom log level
//! #[time("info", "FirstStruct::{}")]      // Logs "FirstStruct::new()" at Info
//! #[time("info", "SecondStruct::{}")]     // Logs "SecondStruct::new()" at Info
//! #[time("ThirdStruct::{}")]              // Logs "ThirdStruct::new()" at Debug
//! #[time("never")]                        // Turn off instrumentation at compile time
//! ```
//!
//! Valid values for the log level are error, warn, info, debug and trace. Debug is the default.
//! You can also specify 'never' to completely disable the instrumentation at compile time.
//! The log level should appear first but as shown above can be omitted. The macros distinguish
//! the log level from the pattern by looking for "{}".
//!
//!
//! # Using the Inline Timers
//!
//! More flexibility, including logging extra information, is provided by the two function-like
//! macro forms, `timer!` and `stimer!`. The difference is that `timer!` returns a timer that
//! logs a message only when it is dropped, while `stimer!` returns a timer that logs a started
//! message as soon as it is created, and a finished message when it is dropped.
//!
//! In this example "FIND_FILES" is the name of the timer (using all UPPERCASE for the timer
//! name is optional but helps make the name stand out in the log)
//!
//! ```text
//! use logging_timer::{timer};
//!
//! fn find_files(dir: PathBuf) -> Vec<PathBuf> {
//!     let _tmr = timer!("FIND_FILES");
//!     let files = vec![];
//!
//!     // expensive operation here
//!
//!     return files;
//! } // _tmr is dropped here and a 'TimerFinished' message is logged
//!```
//!
//! Note that you have to assign the result of the macro to a variable, otherwise Rust will
//! drop the returned timer object *immediately*, which is not what you want (you want it to
//! be dropped at the end of scope).
//!
//! You can replace `timer!` with `stimer!` to get a timer that logs a starting message as
//! well, giving you a pair of 'bracketing' log messages.
//!
//! In addition, both timer macros accept [format_args!](https://doc.rust-lang.org/std/macro.format_args.html)
//! style parameters, allowing you to include extra information in the log messages.
//!
//! ```text
//! let _tmr = timer!("FIND_FILES", "Directory = {}", dir);
//! ```
//!
//! # Outputting Intermediate Messages
//!
//! The `executing!` macro allows you to make the timer produce a message before it is dropped.
//! You can call it as many times as you want. A pseudocode example:
//!
//! ```text
//! use logging_timer::{timer, executing};
//!
//! fn find_files(dir: PathBuf) -> Vec<PathBuf> {
//!     let tmr = timer!("FIND_FILES");
//!     let files = vec![];
//!
//!     for dir in sub_dirs(dir) {
//!         // expensive operation
//!         executing!(tmr, "Processed {}", dir);
//!     }
//!
//!     return files;
//! } // tmr is dropped here and a 'TimerFinished' message is logged
//!```
//!
//! # Controlling the Final Message
//!
//! The `finish!` macro also makes the timer log a message, but it also has the side
//! effect of suppressing the normal drop message.  `finish!` is useful when you want the final
//! message to include some information that you did not have access to until the calculation had
//! finished.
//!
//! ```text
//! use logging_timer::{timer, executing, finish};
//!
//! fn find_files(dir: PathBuf) -> Vec<PathBuf> {
//!     let tmr = timer!("FIND_FILES");
//!     let files = vec![];
//!
//!     finish!(tmr, "Found {} files", files.len());
//!     return files;
//! } // tmr is dropped here but no message is produced.
//!```
//!
//! # Setting the log level
//!
//! By default both `timer` and `stimer` log at `Debug` level. An optional first parameter to
//! these macros allows you to set the log level. **To aid parsing of the macro arguments this
//! first parameter is terminated by a semi-colon.** For example:
//!
//! ```text
//! let tmr1 = timer!(Level::Warn; "TIMER_AT_WARN");
//! let tmr2 = stimer!(Level::Info; "TIMER_AT_INFO");
//! ```
//! # Example of Timer Output
//!
//! The overall format will depend on how you customize the output format of the log crate, but as an illustrative example:
//!
//! ```text
//! 2019-05-30T21:41:41.847982550Z DEBUG [TimerStarting] [dnscan/src/main.rs/63] DIRECTORY_ANALYSIS
//! 2019-05-30T21:41:41.868690703Z INFO [dnlib::configuration] [dnlib/src/configuration.rs/116] Loaded configuration from "/home/phil/.dnscan/.dnscan.json"
//! 2019-05-30T21:41:41.897609281Z DEBUG [TimerFinished] [dnlib/src/io.rs/67] FIND_FILES, Elapsed=28.835275ms, Dir="/home/phil/mydotnetprojects", NumSolutions=1 NumCsproj=45, NumOtherFiles=12
//! 2019-05-30T21:41:41.955140835Z DEBUG [TimerFinished] [dnlib/src/analysis.rs/93] LOAD_SOLUTIONS, Elapsed=57.451736ms
//! 2019-05-30T21:41:42.136762196Z DEBUG [TimerFinished] [dnlib/src/analysis.rs/108] LOAD_PROJECTS, Elapsed=181.563223ms, Found 43 linked projects and 2 orphaned projects
//! 2019-05-30T21:41:42.136998556Z DEBUG [TimerStarting] [dnscan/src/main.rs/87] CALCULATE_PROJECT_GRAPH
//! 2019-05-30T21:41:42.143072972Z DEBUG [TimerExecuting] [dnscan/src/main.rs/87] CALCULATE_PROJECT_GRAPH, Elapsed=6.075205ms, Individual graphs done
//! 2019-05-30T21:41:42.149218039Z DEBUG [TimerFinished] [dnscan/src/main.rs/87] CALCULATE_PROJECT_GRAPH, Elapsed=12.219438ms, Found 19 redundant project relationships
//! 2019-05-30T21:41:42.165724712Z DEBUG [TimerFinished] [dnscan/src/main.rs/108] WRITE_OUTPUT_FILES, Elapsed=16.459312ms
//! 2019-05-30T21:41:42.166445Z INFO [TimerFinished] [dnscan/src/main.rs/63] DIRECTORY_ANALYSIS, Elapsed=318.48581ms
//! ```
//!
//! Here the `[Timer*]` blocks are the `target` field from log's [Record](https://docs.rs/log/0.4.6/log/struct.Record.html)
//! struct and `[dnscan/src/main.rs/63]` is the filename and number from `Record` - this captures the place where the timer was
//! instantiated. The module is also set, but is not shown in these examples.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use super::is_timer_enabled;

// Proc-macros have to be defined in their own lib crate (for now).
// Re-export them so that users only have to care about this one.
// [TODO:ZZ] do this in the future.
// pub use logging_timer_proc_macros::{time, stime};

/*
 * Sizes in bytes on 64bit Linux:
 *   level       =  8
 *   file        = 16
 *   module_path = 16
 *   line        =  4
 *   finished    =  1
 *   start_time  = 16
 *   name        = 16
 *   extra_info  = 24
 *
 *   TOTAL       = 104
 *
 * Wrapping in an Option<T> so that we can avoid most computation if log_enabled!(level)
 * returns false does not increase the size of the value at all. Rust is cool :-)
 */

 /// When this struct is dropped, it logs a message stating its name and how long
/// the execution time was. Can be used to time functions or other critical areas.
pub struct LoggingTimer<'name> {
    /// The log level. Defaults to Debug.
    level: ::log::Level,
    /// Set by the file!() macro to the name of the file where the timer is instantiated.
    file: &'static str,
    /// Set by the module_path!() macro to the module where the timer is instantiated.
    module_path: &'static str,
    /// Set by the line!() macro to the line number where the timer is instantiated.
    line: u32,
    /// A flag used to suppress printing of the 'Finished' message in the drop() function
    /// It is set by the finish method.
    finished: AtomicBool,
    /// The instant, in UTC, that the timer was instantiated.
    start_time: Instant,
    /// The name of the timer. Used in messages to identify it.
    name: &'name str,
    /// Any extra information to be logged along with the name. Unfortunately, due
    /// to the lifetimes associated with a `format_args!` invocation, this currently allocates
    /// if you use it.
    extra_info: Option<String>,
}

impl<'name> LoggingTimer<'name> {
    /// Constructs a new `LoggingTimer` that prints only a 'TimerFinished' message.
    /// This method is not usually called directly, use the `timer!` macro instead.
    pub fn new(
        file: &'static str,
        module_path: &'static str,
        line: u32,
        name: &'name str,
        extra_info: Option<String>,
        level: ::log::Level,
    ) -> Option<Self> {
        if ::log::log_enabled!(level) && is_timer_enabled(module_path) {
            Some(LoggingTimer {
                level,
                start_time: Instant::now(),
                file,
                module_path,
                line,
                name,
                finished: AtomicBool::new(false),
                extra_info
            })
        } else {
            None
        }
    }

    /// Constructs a new `LoggingTimer` that prints a 'TimerStarting' and a 'TimerFinished' message.
    /// This method is not usually called directly, use the `stimer!` macro instead.
    pub fn with_start_message(
        file: &'static str,
        module_path: &'static str,
        line: u32,
        name: &'name str,
        extra_info: Option<String>,
        level: ::log::Level,
    ) -> Option<Self> {
        if ::log::log_enabled!(level) && is_timer_enabled(module_path) {
            let tmr = Self::new(file, module_path, line, name, extra_info, level).unwrap();
            tmr.log_impl(TimerTarget::Starting, None);
            Some(tmr)
        } else {
            None
        }
    }

    /// Returns how long the timer has been running for.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Sets the logging level.
    /// Note that this consumes self, so that it can be called in a one-liner like this:
    ///
    /// ```text
    /// let tmr = timer!("foo").level(Level::Trace);
    /// ```
    #[deprecated(since = "0.3", note = "Please use the first parameter to the `timer` or `stimer` macro instead")]
    pub fn level(mut self, level: ::log::Level) -> Self {
        self.level = level;
        self
    }

    /// Outputs a log message with a target of 'TimerExecuting' showing the current elapsed time, but does not
    /// stop the timer. This method can be called multiple times.
    /// The message can include further information via a `format_args!` approach.
    /// This method is usually not called directly, it is easier to use the `executing!` macro.
    pub fn executing(&self, args: Option<fmt::Arguments>) {
        self.log_impl(TimerTarget::Executing, args);
    }

    /// Outputs a log message with a target of 'TimerFinished' and suppresses the normal message
    /// that is output when the timer is dropped. The message can include further `format_args!`
    /// information. This method is normally called using the `finish!` macro. Calling
    /// `finish()` again will have no effect.
    pub fn finish(&self, args: Option<fmt::Arguments>) {
        if !self.finished.load(Ordering::SeqCst) {
            self.finished.store(true, Ordering::SeqCst);
            self.log_impl(TimerTarget::Finished, args);
        }
    }

    fn log_impl(&self, target: TimerTarget, args: Option<fmt::Arguments>) {
        if !::log::log_enabled!(self.level) {
            return;
        }

        match (target, self.extra_info.as_ref(), args) {
            (TimerTarget::Starting, Some(info), Some(args)) => {
                self.log_record(target, format_args!("{}, {}, {}", self.name, info, args))
            }
            (TimerTarget::Starting, Some(info), None) => {
                self.log_record(target, format_args!("{}, {}", self.name, info))
            }
            (TimerTarget::Starting, None, Some(args)) => {
                self.log_record(target, format_args!("{}, {}", self.name, args))
            }
            (TimerTarget::Starting, None, None) => self.log_record(target, format_args!("{}", self.name)),

            (_, Some(info), Some(args)) => {
                self.log_record(target, format_args!("{}, Elapsed={:?}, {}, {}", self.name, self.elapsed(), info, args))
            }
            (_, Some(info), None) => {
                self.log_record(target, format_args!("{}, Elapsed={:?}, {}", self.name, self.elapsed(), info))
            }
            (_, None, Some(args)) => {
                self.log_record(target, format_args!("{}, Elapsed={:?}, {}", self.name, self.elapsed(), args))
            }
            (_, None, None) => self.log_record(target, format_args!("{}, Elapsed={:?}", self.name, self.elapsed())),
        };
    }

    fn log_record(&self, target: TimerTarget, args: fmt::Arguments) {
        ::log::logger().log(
            &log::RecordBuilder::new()
                .level(self.level)
                .target(match target {
                    TimerTarget::Starting => "TimerStarting",
                    TimerTarget::Executing => "TimerExecuting",
                    TimerTarget::Finished => "TimerFinished",
                })
                .file(Some(self.file))
                .module_path(Some(self.module_path))
                .line(Some(self.line))
                .args(args)
                .build(),
        );
    }
}

impl<'a> Drop for LoggingTimer<'a> {
    /// Drops the timer, outputting a log message with a target of `TimerFinished`
    /// if the `finish` method has not yet been called.
    fn drop(&mut self) {
        self.finish(None);
    }
}

#[derive(Debug, Copy, Clone)]
enum TimerTarget {
    Starting,
    Executing,
    Finished,
}

/* TODO: These macro definitions are very verbose, especially the duplication to get
 * 'level' to work, but after much hacking this was the only combination I could
 * get to work. There is probably a way to reduce the duplication, especially
 * by making the 'level' bit optional.
 */

/// Creates a timer that does not log a starting message, only a finished one.
///
/// # Examples
/// Note that when specifying the log level you must use a semi-colon as a
/// separator, this is to ensure disambiguous parsing of the macro arguments.
///
/// ```text
///
/// use logging_timer::{stime, time, stimer, timer, Level};
///
/// let _tmr1 = timer!("FIND_FILES");
/// let _tmr2 = timer!(Level::Info; "FIND_FILES");
/// let _tmr3 = timer!("FIND_FILES", "Found {} files", 42);
/// let _tmr4 = timer!(Level::Trace; "FIND_FILES", "Found {} files", 42);
/// ```
#[macro_export]
macro_rules! timer {
    ($name:expr) => {
        {
            $crate::LoggingTimer::new(
                file!(),
                module_path!(),
                line!(),
                $name,
                None,
                $crate::log::Level::Debug,
                )
        }
    };

    ($level:expr; $name:expr) => {
        {
            $crate::LoggingTimer::new(
                file!(),
                module_path!(),
                line!(),
                $name,
                None,
                $level,
                )
        }
    };

    ($name:expr, $format:tt) => {
        {
            $crate::LoggingTimer::new(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format)),
                $crate::log::Level::Debug,
                )
        }
    };

    ($level:expr; $name:expr, $format:tt) => {
        {
            $crate::LoggingTimer::new(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format)),
                $level,
                )
        }
    };

    ($name:expr, $format:tt, $($arg:expr),*) => {
        {
            $crate::LoggingTimer::new(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format, $($arg), *)),
                $crate::log::Level::Debug,
                )
        }
    };

    ($level:expr; $name:expr, $format:tt, $($arg:expr),*) => {
        {
            $crate::LoggingTimer::new(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format, $($arg), *)),
                $level,
                )
        }
    };
}

/// Creates a timer that logs a starting mesage and a finished message.
///
/// # Examples
/// Note that when specifying the log level you must use a semi-colon as a
/// separator, this is to ensure disambiguous parsing of the macro arguments.
///
/// ```text
///
/// use logging_timer::{stime, time, stimer, timer, Level};
///
/// let _tmr1 = stimer!("FIND_FILES");
/// let _tmr2 = stimer!(Level::Info; "FIND_FILES");
/// let _tmr3 = stimer!("FIND_FILES", "Found {} files", 42);
/// let _tmr4 = stimer!(Level::Trace; "FIND_FILES", "Found {} files", 42);
/// ```
#[macro_export]
macro_rules! stimer {
    ($name:expr) => {
        {
            $crate::LoggingTimer::with_start_message(
                file!(),
                module_path!(),
                line!(),
                $name,
                None,
                $crate::log::Level::Debug,
                )
        }
    };

    ($level:expr; $name:expr) => {
        {
            $crate::LoggingTimer::with_start_message(
                file!(),
                module_path!(),
                line!(),
                $name,
                None,
                $level,
                )
        }
    };

    ($level:expr; $name:expr, $format:tt) => {
        {
            $crate::LoggingTimer::with_start_message(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format)),
                $level,
                )
        }
    };

    ($name:expr, $format:tt) => {
        {
            $crate::LoggingTimer::with_start_message(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format)),
                $crate::log::Level::Debug,
                )
        }
    };

    ($name:expr, $format:tt, $($arg:expr),*) => {
        {
            $crate::LoggingTimer::with_start_message(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format, $($arg), *)),
                $crate::log::Level::Debug,
                )
        }
    };

    ($level:expr; $name:expr, $format:tt, $($arg:expr),*) => {
        {
            $crate::LoggingTimer::with_start_message(
                file!(),
                module_path!(),
                line!(),
                $name,
                Some(format!($format, $($arg), *)),
                $level,
                )
        }
    };
}

/// Makes an existing timer output an 'executing' mesasge.
/// Can be called multiple times.
#[macro_export]
macro_rules! executing {
    ($timer:expr) => ({
        if let Some(ref tmr) = $timer {
            tmr.executing(None);
        }
    });

    ($timer:expr, $format:tt) => ({
        if let Some(ref tmr) = $timer {
            tmr.executing(Some(format_args!($format)))
        }
    });

    ($timer:expr, $format:tt, $($arg:expr),*) => ({
        if let Some(ref tmr) = $timer {
            tmr.executing(Some(format_args!($format, $($arg), *)))
        }
    })
}

/// Makes an existing timer output a 'finished' mesasge and suppresses
/// the normal drop message.
/// Only the first call has any effect, subsequent calls will be ignored.
#[macro_export]
macro_rules! finish {
    ($timer:expr) => ({
        if let Some(ref tmr) = $timer {
            tmr.finish(None)
        }
    });

    ($timer:expr, $format:tt) => ({
        if let Some(ref tmr) = $timer {
            tmr.finish(Some(format_args!($format)))
        }
    });

    ($timer:expr, $format:tt, $($arg:expr),*) => ({
        if let Some(ref tmr) = $timer {
            tmr.finish(Some(format_args!($format, $($arg), *)))
        }
    })
}
