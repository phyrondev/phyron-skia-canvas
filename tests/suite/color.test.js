// @ts-check

"use strict";

// Contract tests for specs/001-wide-gamut-hdr-export.

const { assert, describe, test } = require("../runner"),
  { Canvas, ImageData } = require("../../lib");

const CANONICAL = [
    "srgb",
    "srgb-linear",
    "display-p3",
    "display-p3-linear",
    "rec2020",
    "rec2020-linear",
    "rec2020-pq",
    "rec2020-hlg",
  ],
  ALIASES = {
    linear: "srgb-linear",
    p3: "display-p3",
    "p3-linear": "display-p3-linear",
    bt2020: "rec2020",
    "bt2020-linear": "rec2020-linear",
    hdr10: "rec2020-pq",
    hlg: "rec2020-hlg",
  };

describe("Colour names (contracts/color-names.md)", () => {
  // N1
  test("rejects unknown colour spaces", async () => {
    assert.throws(
      () => new Canvas(2, 2, { colorSpace: "rec2020-pqq" }),
      (err) =>
        err instanceof TypeError &&
        err.message.includes("rec2020-pqq") &&
        CANONICAL.every((name) => err.message.includes(name)),
    );

    let canvas = new Canvas(2, 2);
    canvas.gpu = false;
    await assert.rejects(
      async () => canvas.toBuffer("raw", { colorSpace: "rec2020-pqq" }),
      TypeError,
    );
    assert.throws(
      () => canvas.toBufferSync("raw", { colorSpace: "rec2020-pqq" }),
      TypeError,
    );
    assert.throws(
      () => new ImageData(2, 2, { colorSpace: "rec2020-pqq" }),
      TypeError,
    );
  });

  // N2
  test("reports canonical colour space names", () => {
    for (const colorSpace of CANONICAL) {
      let canvas = new Canvas(2, 2, { colorSpace });
      assert.equal(canvas.colorSpace, colorSpace);
    }
    assert.equal(new Canvas(2, 2).colorSpace, "srgb");
  });

  // N3
  test("accepts colour space aliases", () => {
    for (const [alias, canonical] of Object.entries(ALIASES)) {
      let canvas = new Canvas(2, 2, { colorSpace: alias });
      assert.equal(canvas.colorSpace, canonical);
    }
  });

  // N4
  test("rejects unknown colour types", async () => {
    assert.throws(
      () => new Canvas(2, 2, { colorType: "RGBAF33" }),
      (err) =>
        err instanceof TypeError &&
        err.message.includes("RGBAF33") &&
        err.message.includes("RGBAF32"),
    );

    let canvas = new Canvas(2, 2, { colorType: "RGBAF32" });
    assert.equal(canvas.colorType, "RGBAF32");
    assert.equal(new Canvas(2, 2).colorType, "RGBA8888");

    canvas.gpu = false;
    await assert.rejects(
      async () => canvas.toBuffer("raw", { colorType: "RGBAF33" }),
      TypeError,
    );
  });
});

//
// Independent colour maths (never a second call into Skia)
//

/** sRGB OETF, sign-preserving for extended values. */
const srgbEncode = (v) =>
    Math.sign(v) *
    (Math.abs(v) <= 0.0031308
      ? 12.92 * Math.abs(v)
      : 1.055 * Math.pow(Math.abs(v), 1 / 2.4) - 0.055),
  srgbDecode = (v) =>
    Math.sign(v) *
    (Math.abs(v) <= 0.04045
      ? Math.abs(v) / 12.92
      : Math.pow((Math.abs(v) + 0.055) / 1.055, 2.4)),
  /** BT.2087: linear Rec.2020 RGB to linear BT.709 RGB. */
  REC2020_TO_709 = [
    [1.6605, -0.5876, -0.0728],
    [-0.1246, 1.1329, -0.0083],
    [-0.0182, -0.1006, 1.1187],
  ],
  mul3 = (m, [r, g, b]) => m.map(([x, y, z]) => x * r + y * g + z * b),
  /** ST 2084 EOTF: PQ signal to nits. */
  pqToNits = (e) => {
    const m1 = 2610 / 16384,
      m2 = (2523 / 4096) * 128,
      c1 = 3424 / 4096,
      c2 = (2413 / 4096) * 32,
      c3 = (2392 / 4096) * 32,
      p = Math.pow(e, 1 / m2);
    return (
      10000 * Math.pow(Math.max(p - c1, 0) / (c2 - c3 * p), 1 / m1)
    );
  };

const floats = (buf) =>
    Array.from(new Float32Array(buf.buffer, buf.byteOffset, buf.length / 4)),
  near = (actual, expected, tolerance, label = "") =>
    assert.ok(
      Math.abs(actual - expected) <= tolerance,
      `${label} ${actual} != ${expected} (tolerance ${tolerance})`,
    ),
  cpuCanvas = (width, height, opts) => {
    let canvas = new Canvas(width, height, opts);
    canvas.gpu = false;
    return canvas;
  },
  /** One opaque grey pixel per value, in the canvas working space. */
  greys = (values, opts) => {
    let canvas = cpuCanvas(values.length, 1, opts),
      ctx = canvas.getContext("2d");
    values.forEach((v, i) => {
      ctx.fillStyle = [v, v, v, 1];
      ctx.fillRect(i, 0, 1, 1);
    });
    return canvas;
  },
  LINEAR_F32 = { colorType: "RGBAF32", colorSpace: "srgb-linear" },
  PRECISION = [0.002, 0.2001, 0.2003, 1.5];

describe("Working space compositing (contracts/working-space.md)", () => {
  const halfWhiteOverBlack = () => {
    let canvas = cpuCanvas(1, 1, LINEAR_F32),
      ctx = canvas.getContext("2d");
    ctx.fillStyle = [0, 0, 0, 1];
    ctx.fillRect(0, 0, 1, 1);
    ctx.fillStyle = [1, 1, 1, 0.5];
    ctx.fillRect(0, 0, 1, 1);
    return canvas;
  };

  // W1
  test("composites in the working space", async () => {
    const decode = {
      "srgb-linear": (v) => v,
      "rec2020-linear": (v) => v,
      srgb: srgbDecode,
      "rec2020-pq": (v) => pqToNits(v) / 203,
    };
    for (const [colorSpace, toLinear] of Object.entries(decode)) {
      let [r, g, b, a] = floats(
        await halfWhiteOverBlack().toBuffer("raw", {
          colorType: "RGBAF32",
          colorSpace,
        }),
      );
      [r, g, b].forEach((v) => near(toLinear(v), 0.5, 1e-3, colorSpace));
      near(a, 1, 1e-6, colorSpace);
    }
  });

  // W2
  test("does not clip HDR before blending", async () => {
    let canvas = cpuCanvas(1, 1, LINEAR_F32),
      ctx = canvas.getContext("2d");
    ctx.fillStyle = [2, 2, 2, 1];
    ctx.fillRect(0, 0, 1, 1);
    ctx.fillStyle = [0, 0, 0, 0.5];
    ctx.fillRect(0, 0, 1, 1);
    let [r, g, b] = floats(await canvas.toBuffer("raw", LINEAR_F32));
    [r, g, b].forEach((v) => near(v, 1, 1e-3));
  });

  // W3
  test("repeated exports ignore stale caches", async () => {
    const outputs = [
      { colorType: "RGBAF32", colorSpace: "srgb" },
      LINEAR_F32,
      { colorType: "RGBA8888", colorSpace: "srgb" },
      { colorType: "RGBAF32", colorSpace: "srgb" },
    ];
    let shared = halfWhiteOverBlack();
    for (const opts of outputs) {
      let fresh = await halfWhiteOverBlack().toBuffer("raw", opts),
        reused = await shared.toBuffer("raw", opts);
      assert.equal(reused.length, fresh.length, JSON.stringify(opts));
      assert.deepEqual(Array.from(reused), Array.from(fresh));
    }
  });

  // W4
  test("getImageData composites in the working space", () => {
    let ctx = halfWhiteOverBlack().getContext("2d"),
      data = ctx.getImageData(0, 0, 1, 1, LINEAR_F32).data,
      [r, g, b] = floats(data);
    [r, g, b].forEach((v) => near(v, 0.5, 1e-3));
  });

  // W5
  test("drawImage of a canvas keeps precision and range", async () => {
    for (const colorType of ["RGBAF32", "RGBAF16"]) {
      let src = greys(PRECISION, LINEAR_F32),
        dst = cpuCanvas(PRECISION.length, 1, {
          colorType,
          colorSpace: "srgb-linear",
        });
      dst.getContext("2d").drawImage(src, 0, 0);
      let values = floats(await dst.toBuffer("raw", LINEAR_F32));
      PRECISION.forEach((v, i) => {
        // a half float has 10 mantissa bits
        let tolerance =
          colorType == "RGBAF16"
            ? Math.max(1e-4, Math.pow(2, Math.floor(Math.log2(v))) / 1024)
            : 1e-4;
        near(values[i * 4], v, tolerance, colorType);
      });
    }
  });

  // W7
  test("drawImage of a canvas keeps gamut", async () => {
    const REC2020 = { colorType: "RGBAF32", colorSpace: "rec2020-linear" },
      expected = mul3(REC2020_TO_709, [0, 1, 0]);
    for (const colorSpace of ["rec2020-linear", "srgb-linear"]) {
      let src = cpuCanvas(1, 1, REC2020),
        sctx = src.getContext("2d");
      sctx.fillStyle = [0, 1, 0, 1];
      sctx.fillRect(0, 0, 1, 1);
      let dst = cpuCanvas(1, 1, { colorType: "RGBAF32", colorSpace });
      dst.getContext("2d").drawImage(src, 0, 0);
      let [r, g, b] = floats(await dst.toBuffer("raw", LINEAR_F32));
      [r, g, b].forEach((v, i) => near(v, expected[i], 1e-3, colorSpace));
    }
  });

  // W8
  test("drawImage of a canvas applies globalAlpha", async () => {
    let src = greys([1], LINEAR_F32),
      dst = cpuCanvas(1, 1, LINEAR_F32),
      ctx = dst.getContext("2d");
    ctx.globalAlpha = 0.5;
    ctx.drawImage(src, 0, 0);
    let [r, , , a] = floats(await dst.toBuffer("raw", LINEAR_F32));
    near(a, 0.5, 1e-4, "alpha");
    near(r, 1, 1e-3, "unpremultiplied red");

    let dst8 = cpuCanvas(1, 1);
    dst8.getContext("2d").drawImage(greys([0.2001], LINEAR_F32), 0, 0);
    let [r8] = Array.from(await dst8.toBuffer("raw", {}));
    assert.equal(r8, Math.round(srgbEncode(0.2001) * 255));
  });

  // W9
  test("float ImageData round trip", async () => {
    for (const colorSpace of ["srgb-linear", "rec2020-linear"]) {
      const opts = { colorType: "RGBAF32", colorSpace };
      let src = greys(PRECISION, opts),
        data = src
          .getContext("2d")
          .getImageData(0, 0, PRECISION.length, 1, opts);
      assert.equal(data.colorSpace, colorSpace);
      let dst = cpuCanvas(PRECISION.length, 1, opts);
      dst.getContext("2d").putImageData(data, 0, 0);
      let values = floats(await dst.toBuffer("raw", opts));
      PRECISION.forEach((v, i) =>
        near(values[i * 4], v, 1e-5, `${colorSpace} ${i}`),
      );
    }
  });
});
