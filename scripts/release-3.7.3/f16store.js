// Is float -> half rounded (nearest-even) or truncated when an F16 canvas stores pixels?
// Inputs go in through float ImageData: an array fillStyle is already about
// 5e-4 relative off before the store (SkPaint keeps its color in sRGB).
const { Canvas } = require(process.argv[2]);
const cpu = (w, h, o) => { const c = new Canvas(w, h, o); c.gpu = false; return c; };
const F32 = { colorType: "RGBAF32", colorSpace: "srgb-linear" };
const ulp = (v) => Math.pow(2, Math.floor(Math.log2(v)) - 10);
const rne = (v) => { const q = ulp(v), n = v / q, f = Math.floor(n), r = n - f;
  return (r > 0.5 || (r === 0.5 && f % 2)) ? (f + 1) * q : f * q; };
const trunc = (v) => Math.floor(v / ulp(v)) * ulp(v);
const vals = [];
for (let k = 0; k < 400; k++) { const base = 0.001 + k * 0.0024; const q = ulp(base);
  vals.push(trunc(base) + 0.75 * q, trunc(base) + 0.25 * q); }   // 0.75 ulp: RNE rounds up, truncation down
vals.push(3e-5, 1e-5); // half denormals
const data = cpu(vals.length, 1, F32).getContext("2d").getImageData(0, 0, vals.length, 1, F32);
const px = new Float32Array(data.data.buffer, data.data.byteOffset, vals.length * 4);
vals.forEach((v, i) => px.set([v, v, v, 1], i * 4));
const c = cpu(vals.length, 1, { colorType: "RGBAF16", colorSpace: "srgb-linear" });
c.getContext("2d").putImageData(data, 0, 0);
const got = new Float32Array(c.toBufferSync("raw", F32).buffer.slice(0));
let asRne = 0, asTrunc = 0, other = 0;
for (let i = 0; i < vals.length - 2; i += 2) { const g = got[i * 4];
  if (Math.abs(g - rne(vals[i])) < 1e-9) asRne++; else if (Math.abs(g - trunc(vals[i])) < 1e-9) asTrunc++; else other++; }
console.log(`${(vals.length - 2) / 2} values at +0.75 ulp: ${asRne} round-to-nearest, ${asTrunc} truncated, ${other} other`);
console.log(`denormals 3e-5, 1e-5 stored as: ${got[(vals.length - 2) * 4]}, ${got[(vals.length - 1) * 4]}`);
