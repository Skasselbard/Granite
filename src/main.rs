#![feature(rustc_private)]
#![deny(rust_2018_idioms)]
#![feature(option_expect_none)]
#![feature(box_patterns)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate rustc;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_mir;

mod init;
mod petri_net;
mod translator;

use crate::translator::Translator;
use clap::{Arg, ArgMatches};
use rustc_driver::Compilation;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::interface;
use rustc_interface::Queries;

struct PetriConfig<'a> {
    arguments: ArgMatches<'a>,
}

impl<'a> rustc_driver::Callbacks for PetriConfig<'a> {
    fn after_analysis<'tcx>(
        &mut self,
        compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        init::init_late_loggers();
        compiler.session().abort_if_errors();

        queries.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            let (entry_def_id, _) = tcx.entry_fn(LOCAL_CRATE).expect("no main function found!");
            let mir_dump = match self.arguments.values_of("mir_dump") {
                Some(path) => Some(out_file("mir")),
                None => None,
            };
            let mut pass = Translator::new(tcx, mir_dump).expect("Unable to create translator");
            let net = pass.petrify(entry_def_id).expect("translation failed");
            for format in self
                .arguments
                .values_of("output_format")
                .expect("no output format given")
            {
                let mut file = out_file(format);
                if format == "pnml" {
                    info!("generating pnml");
                    net.to_pnml(&mut file).expect("write error");
                }
                if format == "lola" {
                    info!("generating lola");
                    net.to_lola(&mut file).expect("write error");
                }
                if format == "dot" {
                    info!("generating dot");
                    net.to_dot(&mut file).expect("write error");
                }
            }
        });

        compiler.session().abort_if_errors();

        Compilation::Stop
    }
}
pub fn main() {
    init::init_early_loggers();
    let matches = clap::App::new("granite")
        .version("0.1")
        .author("Tom Meyer <tom.meyer89@gmail.com>")
        .about("Translate rust programs into petri nets")
        .arg(
            Arg::with_name("output_format")
                .long("format")
                .value_name("FORMAT")
                .help("Defines the output standard for the generated petri net")
                .possible_values(&["pnml", "lola", "dot"])
                .multiple(true)
                .default_value("pnml"),
        )
        .arg(
            Arg::with_name("mir_dump")
                .long("mir_dump")
                .help("Dumps pretty printed mir into the given file")
                .required(false),
        );
    let (mut rustc_args, mut granite_args) = init::parse_arguments();
    init::check_sysroot(&mut rustc_args);

    // clap needs an executable path (or at least the first argument is ignored in parsing)
    granite_args.insert(0, rustc_args.first().unwrap().into());
    let mut config = PetriConfig {
        arguments: matches.get_matches_from(granite_args),
    };
    let result = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&rustc_args, &mut config, None, None)
    })
    .and_then(|result| result);
    std::process::exit(result.is_err() as i32);
}

fn out_file(format: &str) -> std::fs::File {
    match std::fs::File::create(format!("net.{}", format)) {
        Ok(file) => file,
        Err(err) => {
            panic!("Unable to create file: {}", err);
        }
    }
}
