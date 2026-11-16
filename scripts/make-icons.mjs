// Renders the 1024px master icon to scripts/icon-source.png (no deps).
// Then run `npx tauri icon scripts/icon-source.png` to produce every size/format.
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const S = Number(process.argv[2] || 1024);
const px = Buffer.alloc(S * S * 4);

function set(x, y, r, g, b, a) {
  const i = (y * S + x) * 4;
  px[i] = r; px[i + 1] = g; px[i + 2] = b; px[i + 3] = a;
}

// Circle with diagonal gradient + white "A" stroke shape (two legs + bar) and a small dot.
const c = S / 2, R = S / 2 - 6;
for (let y = 0; y < S; y++) {
  for (let x = 0; x < S; x++) {
    const d = Math.hypot(x - c + 0.5, y - c + 0.5);
    if (d > R + 1) continue;
    const edge = Math.max(0, Math.min(1, R + 1 - d)); // antialias
    const t = (x + y) / (2 * S);
    let r = Math.round(99 + (16 - 99) * t), g = Math.round(102 + (185 - 102) * t), b = Math.round(241 + (129 - 241) * t);
    // Letter "A"
    const nx = (x - c) / R, ny = (y - c) / R;
    const legL = Math.abs((nx + 0.0) + (ny + 0.55) * 0.42) < 0.075 && ny > -0.55 && ny < 0.5;
    const legR = Math.abs((nx - 0.0) - (ny + 0.55) * 0.42) < 0.075 && ny > -0.55 && ny < 0.5;
    const bar = Math.abs(ny - 0.18) < 0.06 && Math.abs(nx) < 0.3;
    if (legL || legR || bar) { r = 255; g = 255; b = 255; }
    set(x, y, r, g, b, Math.round(255 * edge));
  }
}

function crc32(buf) {
  let c, crc = 0xffffffff;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
const raw = Buffer.alloc(S * (S * 4 + 1));
for (let y = 0; y < S; y++) { raw[y * (S * 4 + 1)] = 0; px.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4); }
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0); ihdr.writeUInt32BE(S, 4); ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync(new URL("./icon-source.png", import.meta.url), png);
console.log("scripts/icon-source.png written (" + S + "px)");
