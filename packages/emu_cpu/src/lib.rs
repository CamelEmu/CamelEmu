//! The ARM11 of the 3DS. The instructions are described by the tables of `arm`, `thumb` and
//! `vfp`, each entry of which tells what kind of instruction it is, its name, its encoding, what
//! it executes and how many cycles it takes. The decoder, the translator and the clock all work
//! from these tables.

mod arm;
mod cpu;
mod decoder;
mod exec;
mod ffi;
mod fpenv;
mod host;
mod instruction;
mod jit;
mod monitor;
mod state;
mod thumb;
mod vfp;

pub use host::HostCallbacks;
