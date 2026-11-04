// @ts-check

"use strict";

// Native memory: Canvas.toImage(), Canvas.release(), Image.release(), the
// external-memory report to V8, and the `gpu` constructor option (3.7.3).

const { assert, describe, test } = require("../runner"),
  { Canvas, Image, externalMemory } = require("../../lib"),
  { Worker } = require("node:worker_threads"),
  v8 = require("node:v8"),
  vm = require("node:vm");

// `gc` without `--expose-gc` on the whole test run
v8.setFlagsFromString("--expose-gc");
const gc = vm.runInNewContext("gc");

const cpu = (w, h, opts) => {
    let canvas = new Canvas(w, h, opts);
    canvas.gpu = false;
    return canvas;
  },
  LINEAR = { colorType: "RGBAF16", colorSpace: "srgb-linear" },
  external = () => externalMemory(),
  rss = () => process.memoryUsage().rss,
  pixels = (canvas, opts) =>
    canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height, opts)
      .data,
  paint = (ctx, offset = 0) => {
    ctx.fillStyle = "#20a0e0";
    ctx.fillRect(10 + offset, 10, 60, 40);
    ctx.filter = "blur(4px)";
    ctx.fillStyle = "rgba(255, 128, 0, 0.5)";
    ctx.fillRect(30, 30 + offset, 50, 50);
    ctx.filter = "none";
  };

describe("Canvas.toImage()", () => {
  test("is a complete raster Image of the canvas size", () => {
    let canvas = cpu(80, 60, LINEAR);
    paint(canvas.getContext("2d"));
    let image = canvas.toImage();
    assert(image instanceof Image);
    assert.equal(image.complete, true);
    assert.equal(image.width, 80);
    assert.equal(image.height, 60);
  });

  for (let [label, opts] of [
    ["8-bit sRGB", { colorType: "rgba", colorSpace: "srgb" }],
    ["F16 linear", LINEAR],
    ["F32 P3 linear", { colorType: "RGBAF32", colorSpace: "display-p3-linear" }],
  ]) {
    test(`draws the same pixels as drawImage(canvas) on the CPU (${label})`, () => {
      let source = cpu(80, 60, opts);
      paint(source.getContext("2d"));

      let viaCanvas = cpu(80, 60, opts),
        viaImage = cpu(80, 60, opts);
      for (let dst of [viaCanvas, viaImage]) {
        let ctx = dst.getContext("2d");
        ctx.fillStyle = "#000";
        ctx.fillRect(0, 0, 80, 60);
        ctx.globalCompositeOperation = "lighter";
        ctx.globalAlpha = 0.5;
      }
      viaCanvas.getContext("2d").drawImage(source, 0, 0);
      viaImage.getContext("2d").drawImage(source.toImage(), 0, 0);

      let read = { colorType: "RGBAF32", colorSpace: "srgb-linear" };
      assert.deepEqual(pixels(viaImage, read), pixels(viaCanvas, read));
    });
  }

  test("is a snapshot: later drawing on the canvas does not change it", () => {
    let source = cpu(40, 40);
    let ctx = source.getContext("2d");
    ctx.fillStyle = "#f00";
    ctx.fillRect(0, 0, 40, 40);
    let image = source.toImage();
    ctx.fillStyle = "#00f";
    ctx.fillRect(0, 0, 40, 40);

    let dst = cpu(40, 40);
    dst.getContext("2d").drawImage(image, 0, 0);
    assert.deepEqual(Array.from(pixels(dst).slice(0, 4)), [255, 0, 0, 255]);
  });

  test("does not pin the source: releasing the source keeps the image", () => {
    let source = cpu(40, 40);
    source.getContext("2d").fillStyle = "#0f0";
    source.getContext("2d").fillRect(0, 0, 40, 40);
    let image = source.toImage();
    source.release();

    let dst = cpu(40, 40);
    dst.getContext("2d").drawImage(image, 0, 0);
    assert.deepEqual(Array.from(pixels(dst).slice(0, 4)), [0, 255, 0, 255]);
  });
});

describe("Canvas.release()", () => {
  test("throws on drawing, reading back and exporting; properties still read", () => {
    let canvas = cpu(40, 40),
      ctx = canvas.getContext("2d");
    ctx.fillRect(0, 0, 10, 10);
    canvas.release();

    assert.equal(canvas.released, true);
    assert.equal(ctx.released, true);
    assert.equal(canvas.width, 40);
    assert.equal(canvas.gpu, false);
    assert.throws(() => ctx.fillRect(0, 0, 10, 10), /released/);
    assert.throws(() => ctx.getImageData(0, 0, 1, 1), /released/);
    assert.throws(() => canvas.toBufferSync("png"), /released/);
    assert.throws(() => canvas.toImage(), /released/);
    assert.throws(() => cpu(10, 10).getContext("2d").drawImage(canvas, 0, 0), /released/);
    assert.throws(() => cpu(10, 10).getContext("2d").drawCanvas(canvas, 0, 0), /released/);
    assert.throws(() => cpu(10, 10).getContext("2d").createPattern(canvas, "repeat"), /released/);
    assert.throws(() => canvas.newPage(), /released/);
    assert.throws(() => (canvas.width = 20), /released/);
    assert.equal(canvas.width, 40);
    canvas.release(); // idempotent
  });

  test("toImage of a 0x0 canvas is a broken image, not a pending one", () => {
    let image = cpu(0, 0).toImage();
    assert.equal(image.complete, true);
    assert.equal(image.width, 0);
    // a broken image draws nothing, as a failed decode does
    cpu(10, 10).getContext("2d").drawImage(image, 0, 0);
  });

  test("Image.release() makes the image undrawable", () => {
    let source = cpu(40, 40);
    let image = source.toImage();
    image.release();
    assert.equal(image.released, true);
    assert.throws(() => cpu(10, 10).getContext("2d").drawImage(image, 0, 0), /released/);
    image.release(); // idempotent
  });

  test("frees the surface getImageData keeps and the export cache", () => {
    let canvas = cpu(1024, 1024, LINEAR), // 8 MB surface at F16
      ctx = canvas.getContext("2d");
    ctx.fillRect(0, 0, 1024, 1024);
    gc();
    let before = external();
    ctx.getImageData(0, 0, 1, 1);
    // the raw buffer is a 4 MB ArrayBuffer on the same counter: keep it alive
    // so it is a known quantity
    let raw = canvas.toBufferSync("raw");
    let held = external() - before - raw.length;
    assert(held >= 1024 * 1024 * 8, `expected >= 8 MB reported, got ${held}`);

    canvas.release();
    let after = external() - before - raw.length;
    assert(after < 1024 * 1024, `expected the report back near zero, got ${after}`);
  });

  test("a per-sample release loop stays flat", async () => {
    const W = 1024, H = 512, N = 40;
    let accum = cpu(W, H, LINEAR),
      actx = accum.getContext("2d"),
      temp = cpu(W, H, LINEAR);
    paint(temp.getContext("2d"));
    temp.getContext("2d").getImageData(0, 0, 1, 1); // 4 MB surface
    gc();
    let start = rss(),
      startExternal = external();
    for (let i = 0; i < N; i++) {
      let sample = cpu(W, H, LINEAR);
      let sctx = sample.getContext("2d");
      paint(sctx, i);
      sctx.getImageData(0, 0, 1, 1); // allocates the 4 MB surface
      let image = sample.toImage(); // 4 MB
      sample.release();
      actx.globalAlpha = 1 / N;
      actx.globalCompositeOperation = "lighter";
      actx.drawImage(image, 0, 0);
      accum.toBufferSync("raw");
      image.release(); // the accumulator has been rasterized
      accum.width = W; // reset: drops the samples' images from the recording
    }
    // the raw export buffers are collected by GC and swept on the event loop
    for (let i = 0; i < 5; i++) {
      gc();
      await new Promise((res) => setImmediate(res));
    }
    let grownExternal = external() - startExternal,
      grown = rss() - start;
    // N x 4 MB (160 MB) if the images or surfaces leaked
    assert(grownExternal < 24 * 1024 * 1024, `external grew by ${grownExternal}`);
    // N x 4 MB would be 160 MB if anything leaked; allow allocator slack
    assert(grown < 64 * 1024 * 1024, `rss grew by ${grown}`);
  });
});

describe("External memory report", () => {
  test("getImageData reports the surface, GC of the canvas takes it back", async () => {
    gc();
    let before = external();
    (() => {
      let canvas = cpu(1024, 1024); // 4 MB surface at 8-bit
      canvas.getContext("2d").getImageData(0, 0, 1, 1);
      let held = external() - before;
      assert(held >= 4 * 1024 * 1024 - 512 * 1024, `reported ${held}`);
    })();
    // finalizers run after a GC, on the event loop
    for (let i = 0; i < 10 && external() - before >= 1024 * 1024; i++) {
      gc();
      await new Promise((res) => setTimeout(res, 20));
    }
    assert(external() - before < 1024 * 1024, `still reported: ${external() - before}`);
  });

  test("toImage images report their pixels and release them", () => {
    gc();
    let before = external();
    let image = cpu(512, 512, LINEAR).toImage(); // 2 MB at F16
    assert(external() - before >= 2 * 1024 * 1024 - 256 * 1024);
    image.release();
    assert(external() - before < 256 * 1024);
  });

  test("an eager drawImage copy (RGBAF32 source) is reported and reset frees it", () => {
    let source = cpu(512, 512, { colorType: "RGBAF32", colorSpace: "srgb-linear" });
    source.getContext("2d").fillRect(0, 0, 10, 10);
    let dst = cpu(512, 512);
    gc();
    let before = external();
    dst.getContext("2d").drawImage(source, 0, 0); // 4 MB F32 copy pinned
    assert(external() - before >= 4 * 1024 * 1024 - 256 * 1024);
    dst.width = 512;
    assert(external() - before < 256 * 1024);
  });

  test("worker threads allocate and free without an abort", async () => {
    const script = `
      const { Canvas } = require(${JSON.stringify(require.resolve("../../lib"))});
      const { parentPort } = require("node:worker_threads");
      let keep = [];
      for (let i = 0; i < 20; i++) {
        let c = new Canvas(256, 256);
        c.gpu = false;
        let ctx = c.getContext("2d");
        ctx.fillRect(0, 0, 100, 100);
        ctx.getImageData(0, 0, 1, 1);
        c.toBufferSync("raw");
        let img = c.toImage();
        if (i % 2) { img.release(); c.release(); } else keep.push(c, img);
      }
      // canvases and images alive at thread exit: the env teardown finalizes them
      parentPort.postMessage(keep.length);
    `;
    let workers = Array.from({ length: 6 }, () => new Worker(script, { eval: true }));
    let counts = await Promise.all(
      workers.map(
        (w) =>
          new Promise((res, rej) => {
            w.once("message", res);
            w.once("error", rej);
            w.once("exit", (code) => code && rej(new Error(`worker exit ${code}`)));
          }),
      ),
    );
    assert.deepEqual(counts, Array(6).fill(20));
    // and the main thread keeps working after the workers' envs are gone
    let c = cpu(64, 64);
    c.getContext("2d").getImageData(0, 0, 1, 1);
    c.release();
  });
});

describe("`gpu` constructor option", () => {
  test("`{ gpu: false }` selects the CPU engine and `.gpu` reads false", () => {
    let canvas = new Canvas(16, 16, { gpu: false });
    assert.equal(canvas.gpu, false);
    assert.equal(canvas.engine.renderer, "CPU");
    canvas.getContext("2d").fillRect(0, 0, 8, 8);
    assert.deepEqual(Array.from(pixels(canvas).slice(0, 4)), [0, 0, 0, 255]);
  });

  test("the setter still works both ways", () => {
    let canvas = new Canvas(16, 16, { gpu: false });
    canvas.gpu = true;
    assert.equal(canvas.gpu, canvas.engine.renderer == "GPU");
    canvas.gpu = false;
    assert.equal(canvas.gpu, false);
    assert.equal(canvas.engine.renderer, "CPU");
  });
});

describe("Linear blending survives", () => {
  test("white at alpha 0.5 plus-lighter over black reads 188 through 8-bit sRGB", () => {
    let a = cpu(4, 4, LINEAR),
      ax = a.getContext("2d");
    ax.globalCompositeOperation = "lighter";
    ax.globalAlpha = 0.5;
    ax.fillStyle = "#fff";
    ax.fillRect(0, 0, 4, 4);
    let b = cpu(4, 4, LINEAR),
      bx = b.getContext("2d");
    bx.fillStyle = "#000";
    bx.fillRect(0, 0, 4, 4);
    bx.drawImage(a, 0, 0);
    let out = cpu(4, 4);
    out.getContext("2d").drawImage(b, 0, 0);
    assert.deepEqual(Array.from(pixels(out).slice(0, 4)), [188, 188, 188, 255]);

    // and through toImage()
    let out2 = cpu(4, 4);
    out2.getContext("2d").drawImage(b.toImage(), 0, 0);
    assert.deepEqual(Array.from(pixels(out2).slice(0, 4)), [188, 188, 188, 255]);
  });
});
