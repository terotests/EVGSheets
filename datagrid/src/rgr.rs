//! The .rgr classes the crate's modules use, imported once.
//!
//! Ranger reads the classes of each file named here. rustc includes the
//! Rust module build.rs writes for DgRgr.rgr, which imports all of them.
//! `cfg(any())` is never on for rustc, and Ranger reads it.

#[allow(dead_code, unused, non_snake_case)]
mod rgr {
    ranger::import_rgr!("./DgRgr");
    #[cfg(any())]
    ranger::import_rgr!("../../../lib/evg/EVGCodepoint");
    #[cfg(any())]
    ranger::import_rgr!("../../ooxml/OoxmlText");
    #[cfg(any())]
    ranger::import_rgr!("../../ooxml/OpcPackage");
    #[cfg(any())]
    ranger::import_rgr!("DgNative");
    #[cfg(any())]
    ranger::import_rgr!("RichText");
    #[cfg(any())]
    ranger::import_rgr!("xlsx/XlsxOfficeCrypto");
}
pub use rgr::{CellRun, DgNative, EVGCodepoint, OoxmlText, OpcPackage, OpcRelationship, RichText, XlsxOfficeCrypto};
