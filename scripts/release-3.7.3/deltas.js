// Pixel deltas between two builds of phyron-skia-canvas, per scene.
// usage: node deltas.js <dir A> <dir B>
// Prints, per scene and output: differing pixels, max per-channel difference
// (8-bit outputs in code values, float outputs in linear units).
const [dirA, dirB] = process.argv.slice(2);
const A = require(dirA), B = require(dirB);

const SPACES = [
  { colorType: "rgba", colorSpace: "srgb" },
  { colorType: "RGBAF16", colorSpace: "srgb-linear" },
  { colorType: "RGBAF32", colorSpace: "srgb-linear" },
  { colorType: "RGBAF16", colorSpace: "display-p3-linear" },
  { colorType: "RGBAF32", colorSpace: "display-p3-linear" },
];
const W = 160, H = 120;

function scene({ Canvas, Image }, opts, variant) {
  const mk = (w, h, o = opts) => { const c = new Canvas(w, h, o); c.gpu = false; return c; };
  const src = mk(W, H);
  const s = src.getContext("2d");
  s.fillStyle = "#20a0e0"; s.fillRect(10, 10, 100, 60);
  s.filter = "blur(5px)"; s.fillStyle = "rgba(255,128,0,0.6)"; s.fillRect(40, 30, 90, 70); s.filter = "none";
  const grad = s.createLinearGradient(0, 0, W, 0);
  grad.addColorStop(0, "#ff0040"); grad.addColorStop(1, "#00ff80");
  s.fillStyle = grad; s.fillRect(0, 100, W, 20);
  s.font = "bold 22px sans-serif"; s.fillStyle = "#fff"; s.fillText("Ag Wy", 12, 96);
  s.strokeStyle = "rgba(0,0,0,0.7)"; s.lineWidth = 1.5; s.strokeText("Ag", 90, 60);
  const mask = mk(W, H);
  mask.getContext("2d").fillRect(20, 20, 100, 80);
  const dst = mk(W, H);
  const d = dst.getContext("2d");
  d.fillStyle = "#101010"; d.fillRect(0, 0, W, H);
  d.globalCompositeOperation = "lighter"; d.globalAlpha = 0.5;
  if (variant === "lazy") d.drawImage(src, 0, 0);
  else if (variant === "eager") d.drawImage(s.getImageData(0, 0, W, H, { colorType: "RGBAF32", colorSpace: opts.colorSpace }), 0, 0);
  d.globalAlpha = 1; d.globalCompositeOperation = "destination-in"; d.drawImage(mask, 0, 0);
  d.globalCompositeOperation = "source-over";
  d.filter = "blur(3px)"; d.drawImage(src, 20, 10, 80, 60); d.filter = "none";
  // image smoothing: downscale and upscale at each quality
  for (const [i, q] of ["low", "medium", "high"].entries()) {
    d.imageSmoothingQuality = q;
    d.drawImage(src, 0, 0, W, H, 4 + i * 52, 4, 48, 36);   // minify
    d.drawImage(src, 30, 30, 12, 9, 4 + i * 52, 80, 48, 36); // magnify
  }
  const out = mk(W, H, { colorType: "rgba", colorSpace: "srgb" });
  out.getContext("2d").drawImage(dst, 0, 0);
  return {
    "8-bit sRGB": out.toBufferSync("raw"),
    "F32 linear": dst.toBufferSync("raw", { colorType: "RGBAF32", colorSpace: "srgb-linear" }),
  };
}

function probe188({ Canvas }) {
  const mk = (w, h, o) => { const c = new Canvas(w, h, o); c.gpu = false; return c; };
  const LIN = { colorType: "RGBAF16", colorSpace: "srgb-linear" };
  const a = mk(4, 4, LIN), ax = a.getContext("2d");
  ax.globalCompositeOperation = "lighter"; ax.globalAlpha = 0.5; ax.fillStyle = "#fff"; ax.fillRect(0, 0, 4, 4);
  const b = mk(4, 4, LIN), bx = b.getContext("2d");
  bx.fillStyle = "#000"; bx.fillRect(0, 0, 4, 4); bx.drawImage(a, 0, 0);
  const out = mk(4, 4, { colorType: "rgba", colorSpace: "srgb" });
  out.getContext("2d").drawImage(b, 0, 0);
  return out.getContext("2d").getImageData(1, 1, 1, 1).data[0];
}

// Differences are measured on premultiplied values (colour times alpha), as
// they composite. Raw exports are unpremultiplied, so a pixel at alpha 1/255
// can swing its colour channels by 255 with no visible effect.
function compare(a, b, float) {
  const va = float ? new Float32Array(a.buffer, a.byteOffset, a.length / 4) : a;
  const vb = float ? new Float32Array(b.buffer, b.byteOffset, b.length / 4) : b;
  const one = float ? 1 : 255;
  let pixels = 0, max = 0, maxRaw = 0;
  for (let p = 0; p < va.length; p += 4) {
    let differs = false;
    for (let c = 0; c < 4; c++) {
      const raw = Math.abs(va[p + c] - vb[p + c]);
      if (raw > 0) differs = true;
      if (raw > maxRaw) maxRaw = raw;
      const pa = c < 3 ? va[p + c] * va[p + 3] / one : va[p + c],
        pb = c < 3 ? vb[p + c] * vb[p + 3] / one : vb[p + c],
        diff = Math.abs(pa - pb);
      if (diff > max) max = diff;
    }
    if (differs) pixels++;
  }
  return { pixels, max, maxRaw };
}

let identical = 0, total = 0;
const rows = [];
for (const opts of SPACES) for (const variant of ["lazy", "eager"]) {
  const a = scene(A, opts, variant), b = scene(B, opts, variant);
  for (const out of Object.keys(a)) {
    total++;
    const { pixels, max, maxRaw } = compare(a[out], b[out], out.startsWith("F32"));
    if (!pixels) identical++;
    const fmt = (v) => out.startsWith("F32") ? v.toExponential(2) : v.toFixed(1);
    rows.push(`${`${opts.colorType} ${opts.colorSpace}`.padEnd(28)} ${variant.padEnd(6)} ${out.padEnd(11)} ` +
      (pixels ? `${String(pixels).padStart(5)} of ${W * H} px differ, max ${fmt(max)} premultiplied (${fmt(maxRaw)} raw)` : "identical"));
  }
}
console.log(rows.join("\n"));
console.log(`${identical}/${total} outputs identical`);
console.log(`188 probe: A ${probe188(A)}, B ${probe188(B)}`);
