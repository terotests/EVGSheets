// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;

use crate::formats::sheet_bytes::SheetBytes;

/// Cfb: the OLE compound file a .xls (and an encrypted .xlsx) is stored in.
///
/// [MS-CFB]: a little FAT file system. A 512-byte header names the sector size,
/// where the FAT sectors are (the first 109 in the header, the rest in a DIFAT
/// chain), where the directory starts and where the mini stream's FAT starts.
/// Streams under the cutoff (4096 bytes) live in 64-byte mini sectors inside
/// the root entry's own stream.
///
/// Only reading, and only what a workbook needs: find a stream in the root
/// storage by name and hand back its bytes. Every chain walk is bounded, so a
/// damaged file ends in `lastError`, not in a loop.
#[derive(Clone)]
pub struct CfbEntry {
    pub name: String,
    pub kind: i64,
    pub left: i64,
    pub right: i64,
    pub child: i64,
    pub start: i64,
    pub size: i64,
}

#[derive(Clone)]
pub struct Cfb {
    pub data: Vec<u8>,
    pub sectorSize: i64,
    pub miniCutoff: i64,
    pub fat: Vec<i64>,
    pub miniFat: Vec<i64>,
    pub entries: Vec<CfbEntry>,
    pub miniStream: Vec<u8>,
    pub lastError: String,
}

impl Cfb {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            sectorSize: 512,
            miniCutoff: 4096,
            fat: Vec::new(),
            miniFat: Vec::new(),
            entries: Vec::new(),
            miniStream: Vec::new(),
            lastError: String::new(),
        }
    }
    /// The compound file signature D0 CF 11 E0 A1 B1 1A E1, and room for a
    /// header.
    pub fn isCompound(b: &[u8]) -> bool {
        if b.len() < 512 {
            return false;
        }
        let sig: Vec<u8> = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        let mut i = 0;
        while i < 8 {
            if b[i] != sig[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    pub fn sectorAt(&self, id: i64) -> i64 {
        (id + 1) * self.sectorSize
    }
    fn fail(&mut self, msg: &str) -> bool {
        self.lastError = format!("damaged compound file: {}", msg);
        false
    }
    pub fn open(&mut self, b: Vec<u8>) -> bool {
        self.lastError = String::new();
        if !Cfb::isCompound(&b) {
            self.lastError = "not a compound file".to_string();
            return false;
        }
        match SheetBytes::u16(&b, 30) {
            9 => self.sectorSize = 512,
            12 => self.sectorSize = 4096,
            _ => return self.fail("unsupported sector size"),
        }
        let per = self.sectorSize / 4;
        let n_sectors = b.len() as i64 / self.sectorSize - 1;
        self.miniCutoff = SheetBytes::i32(&b, 56);
        if self.miniCutoff <= 0 {
            self.miniCutoff = 4096;
        }
        // --- the FAT sectors: 109 in the header, then the DIFAT chain
        let mut fat_sectors: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < 109 {
            let s = SheetBytes::i32(&b, 76 + i * 4);
            if s >= 0 {
                fat_sectors.push(s);
            }
            i += 1;
        }
        let mut next = SheetBytes::i32(&b, 68);
        let mut hops = 0;
        while next >= 0 {
            hops += 1;
            if hops > n_sectors + 1 {
                return self.fail("DIFAT chain loops");
            }
            let at = self.sectorAt(next);
            let mut k = 0;
            while k < per - 1 {
                let s = SheetBytes::i32(&b, at + k * 4);
                if s >= 0 {
                    fat_sectors.push(s);
                }
                k += 1;
            }
            next = SheetBytes::i32(&b, at + (per - 1) * 4);
        }
        for sector in fat_sectors.iter() {
            let at = self.sectorAt(*sector);
            if at + self.sectorSize > b.len() as i64 {
                return self.fail("FAT sector out of range");
            }
            let mut k = 0;
            while k < per {
                self.fat.push(SheetBytes::i32(&b, at + k * 4));
                k += 1;
            }
        }
        let dir_start = SheetBytes::i32(&b, 48);
        let mini_start = SheetBytes::i32(&b, 60);
        // the sectors are read from here on
        self.data = b;
        // --- the directory: 128-byte entries
        let dir = self.readChain(dir_start, -1);
        if !self.lastError.is_empty() {
            return false;
        }
        let n_ent = dir.len() as i64 / 128;
        let mut e = 0;
        while e < n_ent {
            let base = e * 128;
            let name_len = SheetBytes::u16(&dir, base + 64).min(64);
            let units = if name_len >= 2 { (name_len - 2) / 2 } else { 0 };
            self.entries.push(CfbEntry {
                name: SheetBytes::utf16(&dir, base, units, true),
                kind: SheetBytes::u8(&dir, base + 66),
                left: SheetBytes::i32(&dir, base + 68),
                right: SheetBytes::i32(&dir, base + 72),
                child: SheetBytes::i32(&dir, base + 76),
                start: SheetBytes::i32(&dir, base + 116),
                size: SheetBytes::i32(&dir, base + 120),
            });
            e += 1;
        }
        if self.entries.is_empty() || self.entries[0].kind != 5 {
            return self.fail("missing root entry");
        }
        // --- the mini FAT and the mini stream it indexes
        if mini_start >= 0 {
            let raw = self.readChain(mini_start, -1);
            if !self.lastError.is_empty() {
                return false;
            }
            let nm = raw.len() as i64 / 4;
            let mut m = 0;
            while m < nm {
                self.miniFat.push(SheetBytes::i32(&raw, m * 4));
                m += 1;
            }
        }
        let root_start = self.entries[0].start;
        let root_size = self.entries[0].size;
        if root_start >= 0 {
            self.miniStream = self.readChain(root_start, root_size);
            if !self.lastError.is_empty() {
                return false;
            }
        }
        true
    }
    /// The sectors of a chain, joined. `limit` < 0 means the whole chain.
    pub fn readChain(&mut self, start: i64, limit: i64) -> Vec<u8> {
        let mut ids: Vec<i64> = Vec::new();
        let mut id = start;
        let n = self.fat.len() as i64;
        while id >= 0 {
            if id >= n {
                self.fail("sector missing from the FAT");
                return Vec::new();
            }
            if ids.len() as i64 > n {
                self.fail("sector chain loops");
                return Vec::new();
            }
            ids.push(id);
            if limit >= 0 && ids.len() as i64 * self.sectorSize >= limit {
                break;
            }
            id = self.fat[id as usize];
        }
        let mut total = ids.len() as i64 * self.sectorSize;
        if limit >= 0 && limit < total {
            total = limit;
        }
        let mut out = vec![0u8; total as usize];
        let mut written = 0;
        for sector in ids.iter() {
            let at = self.sectorAt(*sector);
            let take = self.sectorSize.min(total - written);
            if at + take > self.data.len() as i64 {
                self.fail("sector out of range (the file may be truncated)");
                return Vec::new();
            }
            out[written as usize..(written + take) as usize].copy_from_slice(&self.data[at as usize..(at + take) as usize]);
            written += take;
        }
        out
    }
    pub fn readMiniChain(&mut self, start: i64, limit: i64) -> Vec<u8> {
        let mut out = vec![0u8; limit as usize];
        let mut id = start;
        let mut written = 0;
        let mut guard = 0;
        let n = self.miniFat.len() as i64;
        while id >= 0 && written < limit {
            if id >= n {
                self.fail("mini sector missing from the mini FAT");
                return Vec::new();
            }
            guard += 1;
            if guard > n + 1 {
                self.fail("mini sector chain loops");
                return Vec::new();
            }
            let at = id * 64;
            let take = (limit - written).min(64);
            if at + take > self.miniStream.len() as i64 {
                self.fail("mini sector out of range");
                return Vec::new();
            }
            out[written as usize..(written + take) as usize].copy_from_slice(&self.miniStream[at as usize..(at + take) as usize]);
            written += take;
            id = self.miniFat[id as usize];
        }
        out
    }
    /// The index of a stream directly under the root storage, or -1. The
    /// directory is a red-black tree per storage; walking all of it is cheaper
    /// to get right than following the comparisons, and a workbook has a
    /// handful of entries.
    pub fn findRootStream(&self, name: &str) -> i64 {
        let want = name.to_lowercase();
        let n = self.entries.len() as i64;
        let mut stack: Vec<i64> = vec![self.entries[0].child];
        let mut seen = 0;
        while !stack.is_empty() {
            let i = stack[stack.len() - 1];
            stack.pop();
            if i < 0 || i >= n {
                continue;
            }
            seen += 1;
            if seen > n {
                return -1;
            }
            let e = &self.entries[i as usize];
            if e.kind == 2 && e.name.to_lowercase() == want {
                return i;
            }
            stack.push(e.left);
            stack.push(e.right);
        }
        -1
    }
    pub fn hasStream(&self, name: &str) -> bool {
        self.findRootStream(name) >= 0
    }
    pub fn stream(&mut self, name: &str) -> Vec<u8> {
        let i = self.findRootStream(name);
        if i < 0 {
            return Vec::new();
        }
        let start = self.entries[i as usize].start;
        let size = self.entries[i as usize].size;
        if size <= 0 {
            return Vec::new();
        }
        if size < self.miniCutoff {
            return self.readMiniChain(start, size);
        }
        self.readChain(start, size)
    }
}
