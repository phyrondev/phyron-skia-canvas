// For each 8-bit sRGB code boundary c+0.5, feed linear values whose exact sRGB
// encoding is c+0.5-d and c+0.5+d code units, through Studio's read-out paths,
// and report mismatches against round-half-up of the exact encoding.
const { Canvas } = require(process.argv[2]);
const enc = (l) => (l <= 0.0031308 ? 12.92 * l : 1.055 * Math.pow(l, 1 / 2.4) - 0.055);
const dec = (e) => (e <= 0.04045 ? e / 12.92 : Math.pow((e + 0.055) / 1.055, 2.4));
const cpu = (w, h, o) => { const c = new Canvas(w, h, o); c.gpu = false; return c; };
const f16 = (x) => { const a = new Float32Array([x]); const u = new Uint32Array(a.buffer)[0];
  // round-to-nearest-even to binary16 then back
  const buf = new DataView(new ArrayBuffer(4)); buf.setFloat32(0, x); 
  return Number(x.toPrecision(4)) && (function(){ const s = Math.sign(x) || 1, v = Math.abs(x);
    if (v === 0) return 0; const e = Math.floor(Math.log2(v)); const ee = Math.max(e, -14);
    const q = Math.pow(2, ee - 10); return s * Math.round(v / q) * q; })(); };
function run(label, type, ds, path) {
  const vals = [], expect = [];
  for (let c = 0; c < 255; c++) for (const d of ds) for (const side of [-1, 1]) {
    const e = (c + 0.5 + side * d) / 255;
    let lin = dec(e);
    if (type === "RGBAF16") lin = f16(lin);
    vals.push(lin);
    expect.push(Math.floor(enc(lin) * 255 + 0.5)); // round half up of the exact encoding of the stored value
  }
  const src = cpu(vals.length, 1, { colorType: type, colorSpace: "srgb-linear" });
  const s = src.getContext("2d");
  vals.forEach((v, i) => { s.fillStyle = [v, v, v, 1]; s.fillRect(i, 0, 1, 1); });
  let got;
  if (path === "drawImage") {
    const out = cpu(vals.length, 1, { colorType: "rgba", colorSpace: "srgb" });
    out.getContext("2d").drawImage(src, 0, 0);
    got = out.getContext("2d").getImageData(0, 0, vals.length, 1).data;
  } else if (path === "raw") {
    const out = cpu(vals.length, 1, { colorType: "rgba", colorSpace: "srgb" });
    out.getContext("2d").drawImage(src, 0, 0);
    got = out.toBufferSync("raw");
  } else if (path === "getImageData") {
    got = s.getImageData(0, 0, vals.length, 1, { colorType: "rgba", colorSpace: "srgb" }).data;
  } else if (path === "rawExport") {
    got = src.toBufferSync("raw", { colorType: "rgba", colorSpace: "srgb" });
  }
  let bad = 0, worst = null, trunc = 0;
  for (let i = 0; i < vals.length; i++) {
    const g = got[i * 4], x = expect[i];
    if (g !== x) { bad++; if (g === x - 1) trunc++; const de = Math.abs(enc(vals[i]) * 255 - (Math.round(enc(vals[i]) * 255 - 0.5) + 0.5));
      if (!worst || de > worst.de) worst = { i, g, x, code: enc(vals[i]) * 255, de }; }
  }
  console.log(`${label.padEnd(28)} d=${ds.join(",").padEnd(6)} ${bad}/${vals.length} differ (${trunc} one below)` +
    (worst ? `, farthest from a boundary: ${worst.de.toFixed(4)} code (exact ${worst.code.toFixed(4)}, got ${worst.g}, want ${worst.x})` : ""));
}
for (const type of ["RGBAF32", "RGBAF16"])
  for (const path of ["drawImage", "raw", "getImageData", "rawExport"])
    for (const ds of [[0.25], [0.1], [0.02], [0.002]])
      run(`${type} ${path}`, type, ds, path);
