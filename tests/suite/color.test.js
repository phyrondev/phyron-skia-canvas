// @ts-check

"use strict";

// Contract tests for specs/001-wide-gamut-hdr-export.

const { assert, describe, test } = require("../runner"),
  { Canvas, ImageData, backend } = require("../../lib");

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

// copy first: a Buffer's byteOffset need not be aligned for a typed array
const floats = (buf) => Array.from(new Float32Array(Uint8Array.from(buf).buffer)),
  u16s = (buf) => Array.from(new Uint16Array(Uint8Array.from(buf).buffer)),
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

//
// Primaries matrices from chromaticities (SMPTE RP 177), and HDR encoders
//

const D65 = [0.3127, 0.329],
  PRIMARIES = {
    rec709: [
      [0.64, 0.33],
      [0.3, 0.6],
      [0.15, 0.06],
    ],
    p3: [
      [0.68, 0.32],
      [0.265, 0.69],
      [0.15, 0.06],
    ],
    rec2020: [
      [0.708, 0.292],
      [0.17, 0.797],
      [0.131, 0.046],
    ],
  },
  xyz = ([x, y]) => [x / y, 1, (1 - x - y) / y],
  inverse3 = (m) => {
    const [[a, b, c], [d, e, f], [g, h, i]] = m,
      det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    return [
      [e * i - f * h, c * h - b * i, b * f - c * e],
      [f * g - d * i, a * i - c * g, c * d - a * f],
      [d * h - e * g, b * g - a * h, a * e - b * d],
    ].map((row) => row.map((v) => v / det));
  },
  transpose3 = (m) => m[0].map((_, col) => m.map((row) => row[col])),
  mulMat3 = (a, b) => a.map((row) => transpose3(b).map((col) => mul3([row], col)[0])),
  rgbToXyz = (primaries) => {
    const p = transpose3(primaries.map(xyz)),
      s = mul3(inverse3(p), xyz(D65));
    return p.map((row) => row.map((v, i) => v * s[i]));
  },
  rec2020To = (target) =>
    mulMat3(inverse3(rgbToXyz(PRIMARIES[target])), rgbToXyz(PRIMARIES.rec2020)),
  pqEncode = (nits) => {
    const m1 = 2610 / 16384,
      m2 = (2523 / 4096) * 128,
      c1 = 3424 / 4096,
      c2 = (2413 / 4096) * 32,
      c3 = (2392 / 4096) * 32,
      y = Math.pow(Math.min(Math.max(nits / 10000, 0), 1), m1);
    return Math.pow((c1 + c2 * y) / (1 + c3 * y), m2);
  },
  /** BT.2100 HLG, display-referred: 1000-nit display, system gamma 1.2. */
  hlgEncode = (rgb, white) => {
    const a = 0.17883277,
      b = 1 - 4 * a,
      c = 0.5 - a * Math.log(4 * a),
      oetf = (e) => (e <= 1 / 12 ? Math.sqrt(3 * e) : a * Math.log(12 * e - b) + c),
      display = rgb.map((v) => Math.max(v * white, 0) / 1000),
      luma = 0.2627 * display[0] + 0.678 * display[1] + 0.0593 * display[2],
      gain = luma > 0 ? Math.pow(luma, (1 - 1.2) / 1.2) : 0;
    return display.map((e) => oetf(Math.min(Math.max(e * gain, 0), 1)));
  },
  toCode = (signal) => Math.round(Math.min(Math.max(signal, 0), 1) * 65535);

describe("Raw export encoding (contracts/raw-export.md)", () => {
  const FILLS = [
      [0, 1, 0],
      [1, 1, 1],
      [2, 2, 2],
    ],
    REC2020 = { colorType: "RGBAF32", colorSpace: "rec2020-linear" },
    fixture = () => {
      let canvas = cpuCanvas(3, 1, REC2020),
        ctx = canvas.getContext("2d");
      FILLS.forEach((rgb, i) => {
        ctx.fillStyle = [...rgb, 1];
        ctx.fillRect(i, 0, 1, 1);
      });
      return canvas;
    },
    pixel = (values, i) => values.slice(i * 4, i * 4 + 3);

  test("primaries matrix matches BT.2087", () => {
    rec2020To("rec709").forEach((row, r) =>
      row.forEach((v, c) => near(v, REC2020_TO_709[r][c], 1e-3)),
    );
  });

  // E1
  test("raw export converts to the requested space", async () => {
    const expected = {
      "rec2020-linear": (rgb) => rgb,
      "srgb-linear": (rgb) => mul3(rec2020To("rec709"), rgb),
      "display-p3-linear": (rgb) => mul3(rec2020To("p3"), rgb),
      srgb: (rgb) => mul3(rec2020To("rec709"), rgb).map(srgbEncode),
    };
    for (const [colorSpace, convert] of Object.entries(expected)) {
      let values = floats(
        await fixture().toBuffer("raw", { colorType: "RGBAF32", colorSpace }),
      );
      FILLS.forEach((rgb, i) =>
        pixel(values, i).forEach((v, ch) =>
          near(v, convert(rgb)[ch], 1e-3, `${colorSpace} px${i} ch${ch}`),
        ),
      );
    }
  });

  // E2
  test("raw export encodes PQ", async () => {
    let codes = u16s(
      await fixture().toBuffer("raw", {
        colorType: "R16G16B16A16UNorm",
        colorSpace: "rec2020-pq",
      }),
    );
    FILLS.forEach((rgb, i) =>
      pixel(codes, i).forEach((code, ch) =>
        near(code, toCode(pqEncode(rgb[ch] * 203)), 2, `px${i} ch${ch}`),
      ),
    );
  });

  // E3
  test("raw export encodes HLG", async () => {
    let codes = u16s(
      await fixture().toBuffer("raw", {
        colorType: "R16G16B16A16UNorm",
        colorSpace: "rec2020-hlg",
      }),
    );
    FILLS.forEach((rgb, i) =>
      pixel(codes, i).forEach((code, ch) =>
        near(code, toCode(hlgEncode(rgb, 203)[ch]), 2, `px${i} ch${ch}`),
      ),
    );
    near(codes[4], Math.round(0.75 * 65535), 0.001 * 65535, "white is 75%");
  });

  // E4
  test("hdrReferenceWhite scales PQ and HLG", async () => {
    const opts = { colorType: "R16G16B16A16UNorm", hdrReferenceWhite: 100 };
    let pq = u16s(
        await fixture().toBuffer("raw", { ...opts, colorSpace: "rec2020-pq" }),
      ),
      hlg = u16s(
        await fixture().toBuffer("raw", { ...opts, colorSpace: "rec2020-hlg" }),
      );
    near(pq[4], toCode(pqEncode(100)), 2, "PQ white");
    near(hlg[4], toCode(hlgEncode([1, 1, 1], 100)[0]), 2, "HLG white");
  });

  // E5
  test("hdrReferenceWhite validates", async () => {
    for (const hdrReferenceWhite of [0, -1, NaN, Infinity]) {
      await assert.rejects(
        async () =>
          fixture().toBuffer("raw", {
            colorSpace: "rec2020-pq",
            hdrReferenceWhite,
          }),
        RangeError,
        String(hdrReferenceWhite),
      );
      assert.throws(
        () =>
          fixture()
            .getContext("2d")
            .getImageData(0, 0, 1, 1, { hdrReferenceWhite }),
        RangeError,
      );
    }
  });

  // E6
  test("premultiplied option", async () => {
    let canvas = cpuCanvas(1, 1, LINEAR_F32),
      ctx = canvas.getContext("2d");
    ctx.fillStyle = [1, 1, 1, 0.5];
    ctx.fillRect(0, 0, 1, 1);

    let [r, , , a] = floats(await canvas.toBuffer("raw", LINEAR_F32));
    near(r, 1, 1e-3, "unpremultiplied");
    near(a, 0.5, 1e-4);

    [r, , , a] = floats(
      await canvas.toBuffer("raw", { ...LINEAR_F32, premultiplied: true }),
    );
    near(r, 0.5, 1e-3, "premultiplied");
    near(a, 0.5, 1e-4);

    [r] = floats(
      await canvas.toBuffer("raw", {
        colorType: "RGBAF32",
        colorSpace: "rec2020-pq",
        premultiplied: true,
      }),
    );
    near(r, pqEncode(203) * 0.5, 1e-3, "premultiplied PQ");

    [r] = floats(
      ctx.getImageData(0, 0, 1, 1, { ...LINEAR_F32, premultiplied: true }).data,
    );
    near(r, 0.5, 1e-3, "premultiplied getImageData");
  });

  // E7
  test("getImageData converts to the requested space", () => {
    const opts = { colorType: "RGBAF32", colorSpace: "display-p3-linear" };
    let data = fixture().getContext("2d").getImageData(0, 0, 3, 1, opts),
      expected = mul3(rec2020To("p3"), [0, 1, 0]);
    assert.equal(data.colorSpace, "display-p3-linear");
    pixel(floats(data.data), 0).forEach((v, ch) =>
      near(v, expected[ch], 1e-3, `ch${ch}`),
    );
  });
});

describe("GPU fallback (contracts/gpu-fallback.md)", () => {
  // G1, G2
  test(
    "GPU engine falls back to CPU raster",
    {
      skip: backend().gpuAvailable
        ? false
        : "no GPU on this host; manual QA step 1 in quickstart.md is the evidence",
    },
    async () => {
      const REC2020 = { colorType: "RGBAF32", colorSpace: "rec2020-linear" };
      let canvas = new Canvas(1, 1, REC2020),
        ctx = canvas.getContext("2d");
      canvas.gpu = true;
      ctx.fillStyle = [2, 2, 2, 1];
      ctx.fillRect(0, 0, 1, 1);

      let [r, g, b, a] = floats(await canvas.toBuffer("raw", REC2020));
      [r, g, b].forEach((v) => near(v, 2, 1e-3));
      near(a, 1, 1e-6);

      let { renderer, fallback } = canvas.engine;
      assert.equal(renderer, "GPU");
      assert.ok(
        fallback === undefined || fallback === "RGBAF32",
        `fallback: ${fallback}`,
      );

      // an 8-bit export allocates on the GPU and clears the report
      let rgba = new Canvas(1, 1);
      rgba.gpu = true;
      await rgba.toBuffer("raw", {});
      assert.equal(rgba.engine.fallback, undefined);
    },
  );
});
