// inspired by and based on miri: https://github.com/rust-lang/miri/blob/master/src/bin/miri.rs

use std::env;
use std::str::FromStr;

pub fn init_early_loggers() {
    env_logger::init();
    if env::var("RUSTC_LOG").is_ok() {
        rustc_driver::init_rustc_env_logger();
    }
}

pub fn init_late_loggers() {
    // We initialize loggers right before we start evaluation. We overwrite the `RUSTC_LOG`
    // env var if it is not set, control it based on `RUST_LOG`.
    if let Ok(var) = env::var("RUST_LOG") {
        if env::var("RUSTC_LOG").is_err() {
            if log::Level::from_str(&var).is_ok() {
                env::set_var(
                    "RUSTC_LOG",
                    &format!("rustc::mir::interpret={0},rustc_mir::interpret={0}", var),
                );
            } else {
                env::set_var("RUSTC_LOG", &var);
            }
            rustc_driver::init_rustc_env_logger();
        }
    }
}

/// Returns the "default sysroot" that we will use if no `--sysroot` flag is set.
/// Should be a compile-time constant.
fn compile_time_sysroot() -> Option<String> {
    if option_env!("RUSTC_STAGE").is_some() {
        // This is being built as part of rustc, and gets shipped with rustup.
        // We can rely on the sysroot computation in librustc.
        return None;
    }
    // For builds outside rustc, we need to ensure that we got a sysroot
    // that gets used as a default.  The sysroot computation in librustc would
    // end up somewhere in the build dir.
    // Taken from PR <https://github.com/Manishearth/rust-clippy/pull/911>.
    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    Some(match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("RUST_SYSROOT")
            .expect("To build without rustup, set the `RUST_SYSROOT` env var at build time")
            .to_owned(),
    })
}

pub fn parse_arguments() -> (Vec<String>, Vec<String>) {
    // Parse our arguments and split them across `rustc` and `fairum`.
    let mut rustc_args = vec![];
    let mut granite_args = vec![];
    let mut after_dashdash = false;
    for arg in std::env::args() {
        if rustc_args.is_empty() {
            // Very first arg: for `rustc`.
            rustc_args.push(arg);
        } else if after_dashdash {
            // Everything that comes after are our args.
            granite_args.push(arg);
        } else {
            match arg.as_str() {
                "--" => {
                    after_dashdash = true;
                }
                _ => {
                    rustc_args.push(arg);
                }
            }
        }
    }
    (rustc_args, granite_args)
}

pub fn check_sysroot(rustc_args: &mut Vec<String>) {
    // Determine sysroot if needed.  Make sure we always call `compile_time_sysroot`
    // as that also does some sanity-checks of the environment we were built in.
    if let Some(sysroot) = compile_time_sysroot() {
        let sysroot_flag = "--sysroot";
        if !rustc_args.iter().any(|e| e == sysroot_flag) {
            // We need to overwrite the default that librustc would compute.
            rustc_args.push(sysroot_flag.to_owned());
            rustc_args.push(sysroot);
        }
    }
}
