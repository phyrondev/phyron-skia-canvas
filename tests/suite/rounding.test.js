// @ts-check

"use strict";

// Quantization: float -> half when an `RGBAF16` canvas stores pixels, and
// float -> 8-bit sRGB on the read-out paths Studio uses (3.7.3).

const { assert, describe, test } = require("../runner"),
  { Canvas } = require("../../lib");

const cpu = (w, h, opts) => {
    let canvas = new Canvas(w, h, opts);
    canvas.gpu = false;
    return canvas;
  },
  LINEAR_F32 = { colorType: "RGBAF32", colorSpace: "srgb-linear" },
  LINEAR_F16 = { colorType: "RGBAF16", colorSpace: "srgb-linear" },
  SRGB_8 = { colorType: "rgba", colorSpace: "srgb" },
  // sRGB transfer function, exact (IEC 61966-2-1)
  encode = (l) =>
    l <= 0.0031308 ? 12.92 * l : 1.055 * Math.pow(l, 1 / 2.4) - 0.055,
  decode = (e) =>
    e <= 0.04045 ? e / 12.92 : Math.pow((e + 0.055) / 1.055, 2.4),
  // spacing of half floats around `v` (normal range)
  halfStep = (v) => Math.pow(2, Math.floor(Math.log2(v)) - 10),
  floats = (buffer) =>
    new Float32Array(
      buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.length),
    ),
  // one pixel per value, linear greys
  greys = (values, opts) => {
    let canvas = cpu(values.length, 1, opts),
      ctx = canvas.getContext("2d");
    values.forEach((v, i) => {
      ctx.fillStyle = [v, v, v, 1];
      ctx.fillRect(i, 0, 1, 1);
    });
    return canvas;
  };

describe("RGBAF16 canvases round to the nearest half float", () => {
  // values three quarters of a step above a representable half: nearest
  // rounds up, truncation rounds down
  const values = Array.from({ length: 200 }, (_, k) => {
    let base = 0.001 + k * 0.0048,
      step = halfStep(base);
    return Math.floor(base / step) * step + 0.75 * step;
  });

  // The inputs go in through float `ImageData`, not `fillStyle`: `SkPaint`
  // keeps its color in sRGB, and the encode and decode around that (skcms's
  // approximate `powf`) move a linear fill by up to about 5e-4 relative, close
  // to a whole half-float step, before anything is stored.
  const exact = () => {
      let ctx = cpu(values.length, 1, LINEAR_F32).getContext("2d"),
        data = ctx.getImageData(0, 0, values.length, 1, LINEAR_F32),
        pixels = floats(data.data);
      values.forEach((v, i) => pixels.set([v, v, v, 1], i * 4));
      // `floats` copies; write the values back into the `ImageData`
      data.data.set(new Uint8Array(pixels.buffer));
      return data;
    },
    f16 = () => cpu(values.length, 1, LINEAR_F16);

  for (const [label, store] of [
    [
      "putImageData of float data",
      () => {
        let dst = f16();
        dst.getContext("2d").putImageData(exact(), 0, 0);
        return dst;
      },
    ],
    [
      "drawImage of an F32 canvas",
      () => {
        let src = cpu(values.length, 1, LINEAR_F32),
          dst = f16();
        src.getContext("2d").putImageData(exact(), 0, 0);
        dst.getContext("2d").drawImage(src, 0, 0);
        return dst;
      },
    ],
  ]) {
    test(label, () => {
      let stored = floats(store().toBufferSync("raw", LINEAR_F32));
      values.forEach((v, i) => {
        let got = stored[i * 4],
          step = halfStep(v);
        assert.ok(
          Math.abs(got - v) <= step / 2,
          `${v} stored as ${got}: off by ${(Math.abs(got - v) / step).toFixed(2)} steps`,
        );
      });
    });
  }

  test("half denormals are kept, not flushed to zero", () => {
    let tiny = [3e-5, 1e-5, 2e-6],
      stored = floats(greys(tiny, LINEAR_F16).toBufferSync("raw", LINEAR_F32));
    tiny.forEach((v, i) => {
      let got = stored[i * 4];
      // denormal halfs are spaced 2^-24 apart
      assert.ok(Math.abs(got - v) <= Math.pow(2, -25), `${v} stored as ${got}`);
    });
  });
});

describe("8-bit sRGB read-out rounds to the nearest code", () => {
  // For every code boundary c + 0.5, linear values whose exact sRGB encoding
  // is 0.1 and 0.25 code units below and above it. Skia's sRGB encode uses an
  // approximate pow (about 0.02 code units of error), so values closer to a
  // boundary are not a rounding test.
  const values = [],
    expected = [];
  for (let c = 0; c < 255; c++) {
    for (const d of [-0.25, -0.1, 0.1, 0.25]) {
      let linear = decode((c + 0.5 + d) / 255);
      values.push(linear);
      expected.push(Math.floor(encode(linear) * 255 + 0.5));
    }
  }

  const check = (bytes, label) => {
    let wrong = expected.flatMap((want, i) =>
      bytes[i * 4] === want ? [] : [`${values[i]}: ${bytes[i * 4]} != ${want}`],
    );
    assert.deepEqual(wrong.slice(0, 5), [], `${label}: ${wrong.length} wrong`);
  };

  for (const opts of [LINEAR_F32, LINEAR_F16]) {
    const source = () => greys(values, opts),
      into8bit = () => {
        let out = cpu(values.length, 1, SRGB_8);
        out.getContext("2d").drawImage(source(), 0, 0);
        return out;
      };

    test(`${opts.colorType}: drawImage into 8-bit sRGB, getImageData`, () => {
      check(
        into8bit().getContext("2d").getImageData(0, 0, values.length, 1).data,
        "getImageData",
      );
    });

    test(`${opts.colorType}: drawImage into 8-bit sRGB, raw export`, () => {
      check(into8bit().toBufferSync("raw"), "raw");
    });

    test(`${opts.colorType}: 8-bit sRGB getImageData of the float canvas`, () => {
      check(
        source().getContext("2d").getImageData(0, 0, values.length, 1, SRGB_8)
          .data,
        "getImageData",
      );
    });

    test(`${opts.colorType}: 8-bit sRGB raw export of the float canvas`, () => {
      check(source().toBufferSync("raw", SRGB_8), "raw export");
    });
  }
});
