// Studio motion-blur loop with toImage()/release() at every level (sample, effect raster, accumulator).
// Minimal phyron-skia-canvas reproduction of Studio's per-frame call shape for
// motion-blurred, gaussian-blurred mask shapes inside a nested comp (CPU path,
// srgb-linear F16, floatRaster false). Mirrors, per call:
//   renderItem.ts motion-blur loop  -> per sample: new temp canvas, clearRect,
//     scale/translate, the item's effect-space raster (createEffectSpaceSurface
//     with strict: assertCanvasHoldsPixels = 1px fill, drawImage into a 1x1
//     probe, getImageData on the probe, clearRect on the source), the shape
//     drawn there, that raster drawn into the temp with the blur as an image
//     filter on the draw, then accum.drawImage(temp) plus-lighter alpha 1/N.
//   renderItemList.ts mask stack -> target layer, mask layer (= accum drawn in),
//     mask drawn onto target with destination-in, target into the comp canvas.
//   renderNestedComp.ts -> nested comp canvas drawn into the root.
//   encode -> root drawn into an sRGB RGBA8888 canvas, toBuffer("raw").
// usage: node --expose-gc studioLoopRepro.mjs <skia-canvas dir> [masks=30] [samples=64]
import { createRequire } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";

const [dir, masksArg = "30", samplesArg = "64"] = process.argv.slice(2);
const { Canvas } = createRequire(import.meta.url)(dir);
const MASKS = Number(masksArg);
const SAMPLES = Number(samplesArg);
const W = 1920, H = 1080;
const OPT = { colorType: "RGBAF16", colorSpace: "srgb-linear" };
const hwm = () => Number(/VmHWM:\s+(\d+)/.exec(readFileSync("/proc/self/status", "utf8"))[1]) / 1024;
const rss = () => process.memoryUsage().rss / 1048576;

function surface(w, h, opts = OPT) {
  const c = new Canvas(w, h, opts);
  c.gpu = false; // applyRasterEngine under PHY_SKIA_CPU=1 (setter, not the constructor option)
  return c;
}
function strictProbe(c) { // assertCanvasHoldsPixels
  const ctx = c.getContext("2d");
  ctx.save(); ctx.fillStyle = "#ffffff"; ctx.fillRect(c.width - 1, c.height - 1, 1, 1); ctx.restore();
  const p = surface(1, 1, { colorType: "rgba", colorSpace: "srgb" });
  p.getContext("2d").drawImage(c, c.width - 1, c.height - 1, 1, 1, 0, 0, 1, 1);
  p.getContext("2d").getImageData(0, 0, 1, 1);
  ctx.clearRect(c.width - 1, c.height - 1, 1, 1);
}

const t0 = Date.now();
const before = rss();
writeFileSync("/proc/self/clear_refs", "5");
const root = surface(W, H);
const comp = surface(W, H); // nested comp canvas (whole content, not cut to view)
const cctx = comp.getContext("2d");
for (let m = 0; m < MASKS; m++) {
  // Motion blur bbox: 1500x200 shape, 53px blur (reach 3*53), 40px of motion.
  const aw = 1500 + 2 * 159 + 40, ah = 200 + 2 * 159;
  const accum = surface(aw, ah);
  const actx = accum.getContext("2d");
  actx.clearRect(0, 0, aw, ah);
  for (let s = 0; s < SAMPLES; s++) {
    const temp = surface(aw, ah);
    const tctx = temp.getContext("2d");
    tctx.clearRect(0, 0, aw, ah);
    const fx = surface(aw, ah); // effect-space raster of the item
    strictProbe(fx);
    const fctx = fx.getContext("2d");
    fctx.clearRect(0, 0, aw, ah);
    fctx.fillStyle = "#ffffff";
    fctx.fillRect(159 + (s * 40) / SAMPLES, 159, 1500, 200);
    tctx.save();
    tctx.filter = "blur(53px)";
    const fxImage = fx.toImage(); fx.release(); tctx.drawImage(fxImage, 0, 0); fxImage.release();
    tctx.restore();
    actx.save();
    actx.globalCompositeOperation = "lighter";
    actx.globalAlpha = 1 / SAMPLES;
    const tempImage = temp.toImage(); temp.release(); actx.drawImage(tempImage, 0, 0); tempImage.release();
    actx.restore();
  }
  // Mask stack: target layer, mask layer, destination-in, onto the comp.
  const target = surface(W, H);
  const tg = target.getContext("2d");
  tg.fillStyle = "#20a0e0";
  tg.fillRect(100, 60 + m * 30, 1200, 120);
  const mask = surface(W, H);
  // cut the chain at the accumulator too: rasterize its samples once and
  // free the recording that holds the 64 sample rasters
  const accumImage = accum.toImage();
  accum.release();
  mask.getContext("2d").drawImage(accumImage, -159, m * 30 - 119);
  accumImage.release();
  tg.save(); tg.globalCompositeOperation = "destination-in"; tg.drawImage(mask, 0, 0); tg.restore();
  cctx.drawImage(target, 0, 0);
}
root.getContext("2d").drawImage(comp, 0, 0);
const out = surface(W, H, { colorType: "rgba", colorSpace: "srgb" });
out.getContext("2d").drawImage(root, 0, 0);
const recordedPeak = hwm();
const buf = await out.toBuffer("raw");
console.log(JSON.stringify({ masks: MASKS, samples: SAMPLES, rssBeforeMB: Math.round(before), peakAfterRecordMB: Math.round(recordedPeak), peakAfterRasterMB: Math.round(hwm()), bytes: buf.length, ms: Date.now() - t0 }));
