// Is float -> half rounded (nearest-even) or truncated when an F16 canvas stores pixels?
const { Canvas } = require(process.argv[2]);
const cpu = (w, h, o) => { const c = new Canvas(w, h, o); c.gpu = false; return c; };
const ulp = (v) => Math.pow(2, Math.floor(Math.log2(v)) - 10);
const rne = (v) => { const q = ulp(v), n = v / q, f = Math.floor(n), r = n - f;
  return (r > 0.5 || (r === 0.5 && f % 2)) ? (f + 1) * q : f * q; };
const trunc = (v) => Math.floor(v / ulp(v)) * ulp(v);
const vals = [];
for (let k = 0; k < 400; k++) { const base = 0.001 + k * 0.0024; const q = ulp(base);
  vals.push(trunc(base) + 0.75 * q, trunc(base) + 0.25 * q); }   // 0.75 ulp: RNE rounds up, truncation down
vals.push(3e-5, 1e-5); // half denormals
const c = cpu(vals.length, 1, { colorType: "RGBAF16", colorSpace: "srgb-linear" });
const x = c.getContext("2d");
vals.forEach((v, i) => { x.fillStyle = [v, v, v, 1]; x.fillRect(i, 0, 1, 1); });
const got = new Float32Array(c.toBufferSync("raw", { colorType: "RGBAF32", colorSpace: "srgb-linear" }).buffer.slice(0));
let asRne = 0, asTrunc = 0, other = 0;
for (let i = 0; i < vals.length - 2; i += 2) { const g = got[i * 4];
  if (Math.abs(g - rne(vals[i])) < 1e-9) asRne++; else if (Math.abs(g - trunc(vals[i])) < 1e-9) asTrunc++; else other++; }
console.log(`${(vals.length - 2) / 2} values at +0.75 ulp: ${asRne} round-to-nearest, ${asTrunc} truncated, ${other} other`);
console.log(`denormals 3e-5, 1e-5 stored as: ${got[(vals.length - 2) * 4]}, ${got[(vals.length - 1) * 4]}`);
