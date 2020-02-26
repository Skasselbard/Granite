# Granite
Find Deadlocks in Rust with Petri-Net Model checking.
This project was startet as part of my masters thesis "[A Petri-Net Semantics for Rust](https://github.com/Skasselbard/Granite/blob/master/doc/MasterThesis/main.pdf)".

- used rust nightly can be found in the [rust-toolchain file](https://doc.rust-lang.org/nightly/edition-guide/rust-2018/rustup-for-managing-rust-versions.html#managing-versions)
- rustc-dev component is needed ``rustup toolchain install [nightly-x-y-z] --component rustc-dev``
- also the linker has to know about the lib folder from the sysroot fiting the toolchain.
- some useful can be found in the script folder. This includes:
    - an install script for the model checker LoLa
    - a run script that can translate programs from ``./tests/sample_programs``
    - and a script that can analyse the output

