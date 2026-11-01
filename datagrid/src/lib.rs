//! The datagrid's modules written in Ranger's Rust syntax. Ranger compiles
//! them with the .rgr files beside them, which import this file, to every
//! target; cargo builds them as one crate (../Cargo.toml).

#![allow(non_snake_case, unused_imports)]

pub mod formats;
mod rgr;
