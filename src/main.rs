#![feature(rustc_private)]
#![deny(rust_2018_idioms)]
#![feature(option_expect_none)]
#![feature(box_patterns)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate rustc;
extern crate rustc_driver;
extern crate rustc_index;
extern crate rustc_interface;

mod init;
mod intrinsics;
mod petri_net;
mod translator;

use crate::translator::Translator;
use rustc::hir::def_id::LOCAL_CRATE;
use rustc_driver::Compilation;
use rustc_interface::interface;

struct PetriConfig {
    _arguments: Vec<String>,
}

impl rustc_driver::Callbacks for PetriConfig {
    fn after_analysis(&mut self, compiler: &interface::Compiler) -> Compilation {
        init::init_late_loggers();
        compiler.session().abort_if_errors();

        compiler.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            let (entry_def_id, _) = tcx.entry_fn(LOCAL_CRATE).expect("no main function found!");
            let mut pass = Translator::new(tcx).expect("Unable to create translator");
            write_to_file(&pass.petrify(entry_def_id).expect("translation failed"));
        });

        compiler.session().abort_if_errors();

        Compilation::Stop
    }
}
pub fn main() {
    init::init_early_loggers();
    let (mut rustc_args, fairum_args) = init::parse_arguments();
    init::check_sysroot(&mut rustc_args);

    let mut config = PetriConfig {
        _arguments: fairum_args,
    };
    let result = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&rustc_args, &mut config, None, None)
    })
    .and_then(|result| result);
    std::process::exit(result.is_err() as i32);
}

fn write_to_file(xml: &str) {
    use std::io::Write;
    let mut file = match std::fs::File::create("net.pnml") {
        Ok(file) => file,
        Err(err) => {
            error!("Unable to create file: {}", err);
            return;
        }
    };
    match file.write_all(xml.as_bytes()) {
        Ok(()) => {}
        Err(err) => error!("Unable to write file: {}", err),
    }
}
