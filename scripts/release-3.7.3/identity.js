// Byte identity of raw exports between two builds of phyron-skia-canvas.
// usage: node identity.js <dir A> <dir B>
// Scenes cover lazy and eager drawImage(canvas) sources, 8-bit / F16 / F32,
// srgb-linear and display-p3-linear, plus-lighter, destination-in, filters.
const [dirA, dirB] = process.argv.slice(2);
const A = require(dirA), B = require(dirB);

const SPACES = [
  { colorType: "rgba", colorSpace: "srgb" },
  { colorType: "RGBAF16", colorSpace: "srgb-linear" },
  { colorType: "RGBAF32", colorSpace: "srgb-linear" },
  { colorType: "RGBAF16", colorSpace: "display-p3-linear" },
  { colorType: "RGBAF32", colorSpace: "display-p3-linear" },
];

function scene({ Canvas }, opts, variant) {
  const mk = (w, h, o = opts) => { const c = new Canvas(w, h, o); c.gpu = false; return c; };
  const src = mk(160, 120);
  const s = src.getContext("2d");
  s.fillStyle = "#20a0e0"; s.fillRect(10, 10, 100, 60);
  s.filter = "blur(5px)"; s.fillStyle = "rgba(255,128,0,0.6)"; s.fillRect(40, 30, 90, 70); s.filter = "none";
  s.font = "20px sans-serif"; s.fillStyle = "#fff"; s.fillText("Ab", 20, 100);
  const mask = mk(160, 120);
  mask.getContext("2d").fillRect(20, 20, 100, 80);
  const dst = mk(160, 120);
  const d = dst.getContext("2d");
  d.fillStyle = "#101010"; d.fillRect(0, 0, 160, 120);
  d.globalCompositeOperation = "lighter"; d.globalAlpha = 0.5;
  if (variant === "lazy") d.drawImage(src, 0, 0);
  else if (variant === "image") d.drawImage(src.toImage(), 0, 0);
  else if (variant === "imagedata") d.drawImage(s.getImageData(0, 0, 160, 120, { colorType: "RGBAF32", colorSpace: opts.colorSpace }), 0, 0);
  d.globalAlpha = 1; d.globalCompositeOperation = "destination-in"; d.drawImage(mask, 0, 0);
  d.globalCompositeOperation = "source-over"; d.filter = "blur(3px)"; d.drawImage(src, 20, 10, 80, 60); d.filter = "none";
  const out = mk(160, 120, { colorType: "rgba", colorSpace: "srgb" });
  out.getContext("2d").drawImage(dst, 0, 0);
  return [out.toBufferSync("raw"), dst.toBufferSync("raw", { colorType: "RGBAF32", colorSpace: "srgb-linear" })];
}

let failures = 0, cases = 0;
for (const opts of SPACES) for (const variant of ["lazy", "imagedata"]) {
  const a = scene(A, opts, variant), b = scene(B, opts, variant);
  for (let i = 0; i < a.length; i++) {
    cases++;
    if (!a[i].equals(b[i])) {
      failures++;
      let diff = 0, max = 0;
      for (let k = 0; k < a[i].length; k++) { const dd = Math.abs(a[i][k] - b[i][k]); if (dd) diff++; if (dd > max) max = dd; }
      console.log(`DIFF ${opts.colorType} ${opts.colorSpace} ${variant} buf${i}: ${diff} bytes differ, max ${max}`);
    }
  }
}
// toImage() must equal the lazy path within one build
if (B.Canvas.prototype.toImage) for (const opts of SPACES) {
  const lazy = scene(B, opts, "lazy"), img = scene(B, opts, "image");
  cases++;
  if (!lazy[1].equals(img[1])) { failures++; console.log(`DIFF toImage vs lazy ${opts.colorType} ${opts.colorSpace}`); }
}
console.log(`${cases - failures}/${cases} identical`);
process.exit(failures ? 1 : 0);
