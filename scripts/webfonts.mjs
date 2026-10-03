// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Clean a TrueType file so a browser's font sanitizer (OTS, in Firefox and
 * Chrome) accepts it without complaint.
 *
 * The site's fonts come from Ranger's PDF writer, and a few carry faults OTS
 * reports in the console every time the page registers them:
 *
 *   kern   Open Sans ships a legacy `kern` table with a subtable longer than
 *          its 16-bit length field; OTS drops it. It also has GPOS, which the
 *          core's measurer (TrueTypeFont.rgr) uses instead whenever present,
 *          so dropping `kern` from such a face changes no measurement.
 *   hdmx   Helvetica has `hdmx` without the head flags that allow it; OTS
 *          drops it. Nothing reads it.
 *   cmap   Helvetica's format-4 terminal 0xFFFF segment has idRangeOffset
 *          0xFFFF, which points outside the subtable. It is rewritten to map
 *          through idDelta to glyph 0, which is what OTS falls back to.
 *   header searchRange/entrySelector/rangeShift are recomputed.
 *   head   macStyle bold/italic bits are set from OS/2 fsSelection
 *          (Josefin Sans Bold says regular).
 *
 * Glyphs, metrics and every table the core reads stay byte-identical.
 */

function readTables(buf) {
  const v = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const numTables = v.getUint16(4);
  const tables = new Map();
  for (let i = 0; i < numTables; i++) {
    const r = 12 + 16 * i;
    const tag = String.fromCharCode(buf[r], buf[r + 1], buf[r + 2], buf[r + 3]);
    const off = v.getUint32(r + 8);
    const len = v.getUint32(r + 12);
    tables.set(tag, Buffer.from(buf.subarray(off, off + len)));
  }
  return { sfntVersion: v.getUint32(0), tables };
}

function checksum(data) {
  const padded = Buffer.alloc((data.length + 3) & ~3);
  data.copy(padded);
  let sum = 0;
  for (let i = 0; i < padded.length; i += 4) sum = (sum + padded.readUInt32BE(i)) >>> 0;
  return sum;
}

function fixCmap(cmap) {
  const numSub = cmap.readUInt16BE(2);
  const seen = new Set();
  for (let i = 0; i < numSub; i++) {
    const off = cmap.readUInt32BE(4 + 8 * i + 4);
    if (seen.has(off) || cmap.readUInt16BE(off) !== 4) continue;
    seen.add(off);
    const length = cmap.readUInt16BE(off + 2);
    const segX2 = cmap.readUInt16BE(off + 6);
    const ends = off + 14;
    const starts = ends + segX2 + 2;
    const deltas = starts + segX2;
    const ranges = deltas + segX2;
    for (let s = 0; s < segX2 / 2; s++) {
      const ro = cmap.readUInt16BE(ranges + 2 * s);
      if (ro === 0) continue;
      const start = cmap.readUInt16BE(starts + 2 * s);
      const end = cmap.readUInt16BE(ends + 2 * s);
      const last = ranges + 2 * s + ro + 2 * (end - start);
      if (last + 2 <= off + length) continue;
      // Points outside the subtable: map the segment to glyph 0 instead.
      cmap.writeUInt16BE(0, ranges + 2 * s);
      cmap.writeUInt16BE((0x10000 - start) & 0xffff, deltas + 2 * s);
    }
  }
}

export function sanitizeFont(input) {
  const { sfntVersion, tables } = readTables(input);
  if (tables.has("GPOS")) tables.delete("kern");
  tables.delete("hdmx");
  if (tables.has("cmap")) fixCmap(tables.get("cmap"));

  const head = tables.get("head");
  const os2 = tables.get("OS/2");
  if (head && os2) {
    const fsSelection = os2.readUInt16BE(62);
    let macStyle = head.readUInt16BE(44) & ~3;
    if (fsSelection & 0x20) macStyle |= 1;
    if (fsSelection & 0x01) macStyle |= 2;
    head.writeUInt16BE(macStyle, 44);
  }
  if (head) head.writeUInt32BE(0, 8); // checkSumAdjustment, set below

  const tags = [...tables.keys()].sort();
  const n = tags.length;
  const entrySelector = Math.floor(Math.log2(n));
  const searchRange = 16 * 2 ** entrySelector;
  const dirLen = 12 + 16 * n;
  let size = dirLen;
  for (const t of tags) size += (tables.get(t).length + 3) & ~3;

  const out = Buffer.alloc(size);
  out.writeUInt32BE(sfntVersion, 0);
  out.writeUInt16BE(n, 4);
  out.writeUInt16BE(searchRange, 6);
  out.writeUInt16BE(entrySelector, 8);
  out.writeUInt16BE(n * 16 - searchRange, 10);
  let at = dirLen;
  let headAt = -1;
  tags.forEach((t, i) => {
    const data = tables.get(t);
    const r = 12 + 16 * i;
    out.write(t, r, "latin1");
    out.writeUInt32BE(checksum(data), r + 4);
    out.writeUInt32BE(at, r + 8);
    out.writeUInt32BE(data.length, r + 12);
    data.copy(out, at);
    if (t === "head") headAt = at;
    at += (data.length + 3) & ~3;
  });
  if (headAt >= 0) out.writeUInt32BE((0xb1b0afba - checksum(out)) >>> 0, headAt + 8);
  return out;
}
