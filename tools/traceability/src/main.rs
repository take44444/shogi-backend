#![feature(rustc_private)]

extern crate rustc_attr_ir;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod driver;
mod model;

fn main() {
    if let Err(error) = driver::run() {
        eprintln!("traceability compiler: {error}");
        std::process::exit(1);
    }
}
