// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage, XlsxOfficeCrypto};

/// XlsxPackage: what a .xlsx calls its parts, and the one thing a .xlsx has
/// that no other OOXML file does.
///
/// The container is shared: a .xlsx is an OPC package like a .docx and a .pptx,
/// and that layer lives once in `gallery/ooxml/OpcPackage.rgr`. What stays here
/// is the two things that are genuinely the spreadsheet's own.
///
/// ENCRYPTION. A password-protected workbook is not a ZIP at all — it is an OLE
/// compound file with the ZIP inside it — so it has to become a package before
/// it can be opened as one. That has its own failure mode ("wrong password")
/// that has nothing to do with packaging, so the core never learns what a
/// password is: this class decrypts and hands plain ZIP bytes in.
///
/// PATHS. Callers open a workbook by full path and expect a `lastError` they can
/// show a user, which the container has no opinion about.
///
/// Everything else — parts, content types, relationships — is delegated, which
/// is also where the spreadsheet GAINS something. It had no relationship layer:
/// four callers scanned the `_rels` XML themselves, and the shared path resolver
/// said out loud that it handled "only the two forms a spreadsheet actually
/// writes". A chart's own relationships write `../../media/image1.png`, which
/// was not one of the two.
pub struct XlsxPackage {
    opc: Rc<RefCell<OpcPackage>>,
    /// The plain ZIP bytes this package was opened from — after decryption, so a
    /// password-protected workbook hands over the same thing an ordinary one
    /// does. A save that preserves the parts nobody modelled has to be able to
    /// read them back after the package is closed, and closing it is what the
    /// loader does the moment it is finished.
    pub plainBytes: Vec<u8>,
    pub openOk: bool,
    pub pathDir: String,
    pub pathName: String,
    pub lastError: String,
}

impl XlsxPackage {
    pub fn new() -> Self {
        Self {
            opc: Rc::new(RefCell::new(OpcPackage::new())),
            plainBytes: vec![0u8; 0],
            openOk: false,
            pathDir: ".".to_string(),
            pathName: String::new(),
            lastError: String::new(),
        }
    }
    /// Turn file bytes into a plain ZIP package. Password-protected Office files
    /// arrive as OLE compounds; without a password (or without a decryptor) we
    /// stop here with a clear error instead of hunting for a ZIP EOCD that does
    /// not exist yet.
    pub fn unwrapBytes(&mut self, bytes: &[u8], password: &str) -> Vec<u8> {
        self.lastError = String::new();
        if !XlsxOfficeCrypto::is_ole_compound(bytes) {
            return bytes.to_vec();
        }
        if password.is_empty() {
            self.lastError = "password-protected workbook (Office encryption); a password is required".to_string();
            return vec![0u8; 0];
        }
        if !XlsxOfficeCrypto::can_decrypt() {
            self.lastError = "password-protected workbook (Office encryption); decryption is not available on this target".to_string();
            return vec![0u8; 0];
        }
        let plain: Vec<u8> = XlsxOfficeCrypto::decrypt(bytes, password);
        if (plain.len() as i64) < 22 {
            self.lastError = "could not decrypt workbook (wrong password, or unsupported encryption)".to_string();
            return vec![0u8; 0];
        }
        plain
    }
    pub fn openPath(&mut self, full_path: &str) -> bool {
        self.openPathPassword(full_path, "")
    }
    pub fn openPathPassword(&mut self, full_path: &str, password: &str) -> bool {
        let mut dirs: Vec<String> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        OpcPackage::split_path(full_path, &mut dirs, &mut names);
        let dir: String = dirs[0].clone();
        let name: String = names[0].clone();
        self.pathDir = dir.clone();
        self.pathName = name.clone();
        let bytes: Vec<u8> = DgNative::read_bytes(&dir, &name);
        self.openBytesPassword(&bytes, password)
    }
    /// The package from bytes rather than from a path — what a browser has.
    pub fn openBytes(&mut self, bytes: &[u8]) -> bool {
        self.openBytesPassword(bytes, "")
    }
    pub fn openBytesPassword(&mut self, bytes: &[u8], password: &str) -> bool {
        self.pathDir = String::new();
        self.pathName = "workbook.xlsx".to_string();
        let data: Vec<u8> = self.unwrapBytes(bytes, password);
        if !self.lastError.is_empty() {
            self.openOk = false;
            return false;
        }
        self.plainBytes = data.clone();
        self.openOk = OpcPackage::open_bytes(&self.opc, &data, self.pathName.clone());
        if !self.openOk {
            self.lastError = "could not open the workbook: not a zip".to_string();
        }
        self.openOk
    }
    pub fn close(&mut self) {
        if self.openOk {
            OpcPackage::close(&self.opc);
        }
        self.openOk = false;
    }
    pub fn listFiles(&self) -> Vec<String> {
        OpcPackage::list_parts(&self.opc)
    }
    pub fn hasEntry(&self, name: &str) -> bool {
        OpcPackage::has_part(&self.opc, name)
    }
    /// The raw bytes of a part. Text parts go through `entryText`; a PNG does
    /// not survive being decoded as UTF-8, so an image has to come out as bytes.
    pub fn entryBytes(&self, name: &str) -> Vec<u8> {
        OpcPackage::read_binary(&self.opc, name)
    }
    pub fn entryText(&self, name: &str) -> String {
        OpcPackage::read_xml(&self.opc, name)
    }
    /// ------------------------------------------------------------------
    /// Relationships — new here, and the same ones every other format uses.
    /// ------------------------------------------------------------------
    /// The workbook part named by `_rels/.rels`, rather than assumed to be at
    /// `xl/workbook.xml`.
    pub fn workbookPart(&self) -> String {
        let named: String = OpcPackage::main_document_part(&self.opc);
        if !named.is_empty() {
            return named;
        }
        "xl/workbook.xml".to_string()
    }
    /// rId on a part → package path. An external target — a hyperlink out to the
    /// web — answers "" here and comes back whole from `relTarget`.
    pub fn resolveRel(&self, part: &str, rel_id: &str) -> String {
        OpcPackage::resolve_relationship(&self.opc, part, rel_id)
    }
    pub fn relTarget(&self, part: &str, rel_id: &str) -> String {
        OpcPackage::relationship_target(&self.opc, part, rel_id)
    }
    pub fn isExternalRel(&self, part: &str, rel_id: &str) -> bool {
        OpcPackage::is_external_relationship(&self.opc, part, rel_id)
    }
    pub fn findRelByType(&self, part: &str, type_suffix: &str) -> String {
        OpcPackage::find_relationship_by_type(&self.opc, part, type_suffix)
    }
    pub fn findRelsByType(&self, part: &str, type_suffix: &str) -> Vec<String> {
        OpcPackage::find_relationships_by_type(&self.opc, part, type_suffix)
    }
    pub fn contentTypeFor(&self, part: &str) -> String {
        OpcPackage::content_type_for(&self.opc, part)
    }
}
