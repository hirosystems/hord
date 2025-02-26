#[macro_use]
extern crate hiro_system_kit;

#[macro_use]
extern crate serde_derive;

extern crate serde;

pub mod bitcoind;
pub mod cli;
pub mod config;
pub mod db;
pub mod scan;
pub mod service;

#[macro_export]
macro_rules! try_info {
    ($a:expr, $tag:expr, $($args:tt)*) => {
        $a.try_log(|l| info!(l, $tag, $($args)*));
    };
    ($a:expr, $tag:expr) => {
        $a.try_log(|l| info!(l, $tag));
    };
}

#[macro_export]
macro_rules! try_debug {
    ($a:expr, $tag:expr, $($args:tt)*) => {
        $a.try_log(|l| debug!(l, $tag, $($args)*));
    };
    ($a:expr, $tag:expr) => {
        $a.try_log(|l| debug!(l, $tag));
    };
}

#[macro_export]
macro_rules! try_warn {
    ($a:expr, $tag:expr, $($args:tt)*) => {
        $a.try_log(|l| warn!(l, $tag, $($args)*));
    };
    ($a:expr, $tag:expr) => {
        $a.try_log(|l| warn!(l, $tag));
    };
}

#[macro_export]
macro_rules! try_error {
    ($a:expr, $tag:expr, $($args:tt)*) => {
        $a.try_log(|l| error!(l, $tag, $($args)*));
    };
    ($a:expr, $tag:expr) => {
        $a.try_log(|l| error!(l, $tag));
    };
}

// #[tokio::main]
fn main() {
    cli::main();
}
