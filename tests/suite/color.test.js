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
