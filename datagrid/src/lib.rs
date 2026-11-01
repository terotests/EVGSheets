//! The datagrid's modules written in Ranger's Rust syntax. Ranger compiles
//! them with the .rgr files beside them, which import this file, to every
//! target; cargo builds them as one crate (../Cargo.toml).

#![allow(non_snake_case, unused_imports)]

pub mod cell_style;
pub mod data_validation;
pub mod date_serial;
pub mod formats;
pub mod formula_value;
mod rgr;
pub mod rich_text;
pub mod script;
pub mod sheet_image;
pub mod sheet_shape;
pub mod sheet_table;
pub mod xlsx;
pub mod xlsx_number_format;
