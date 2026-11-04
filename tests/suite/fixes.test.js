// @ts-check

"use strict";

// Regressions reported in issues #32, #33, #35, #36 and #38.

const { assert, describe, test } = require("../runner"),
  { Canvas, ParagraphBuilder } = require("../../lib");

const cpu = (w, h, opts) => {
    let canvas = new Canvas(w, h, opts);
    canvas.gpu = false;
    return canvas;
  },
  px = (ctx, x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data),
  // median of `runs` timings of `f`
  timed = (f, runs = 5) => {
    let times = [];
    for (let i = 0; i < runs; i++) {
      let t = performance.now();
      f();
      times.push(performance.now() - t);
    }
    return times.sort((a, b) => a - b)[Math.floor(runs / 2)];
  };

describe("Path regressions (#32, #33)", () => {
  test("arc continues the current subpath", () => {
    let canvas = cpu(120, 120),
      ctx = canvas.getContext("2d");
    ctx.beginPath();
    ctx.moveTo(20, 20);
    ctx.lineTo(100, 20);
    ctx.arc(60, 60, 40, 0, Math.PI);
    ctx.closePath();
    // a point inside the region the line and the arc enclose together
    assert.ok(ctx.isPointInPath(60, 40), "arc must extend the contour");
    ctx.fillStyle = "black";
    ctx.fill();
    assert.deepEqual(px(ctx, 60, 40), [0, 0, 0, 255]);
  });

  test("ellipse continues the current subpath", () => {
    let ctx = cpu(120, 120).getContext("2d");
    ctx.beginPath();
    ctx.moveTo(20, 20);
    ctx.lineTo(100, 20);
    ctx.ellipse(60, 60, 40, 40, 0, 0, Math.PI);
    ctx.closePath();
    assert.ok(ctx.isPointInPath(60, 40), "ellipse must extend the contour");
  });

  test("roundRect starts at the top-left radius", () => {
    let { Path2D } = require("../../lib"),
      path = new Path2D();
    path.roundRect(10, 10, 100, 50, 10);
    let d = path.d;
    // the HTML spec starts the subpath at (x + top-left radius, y)
    assert.match(d, /^M20[, ]10/, `start point in ${d}`);
  });

  test("a non-positive conic weight draws a line", () => {
    let { Path2D } = require("../../lib");
    for (const weight of [0, -1]) {
      let straight = new Path2D();
      straight.moveTo(10, 90);
      straight.conicCurveTo(50, 10, 90, 90, weight);
      let line = new Path2D();
      line.moveTo(10, 90);
      line.lineTo(90, 90);
      assert.deepEqual(
        straight.bounds,
        line.bounds,
        `weight ${weight} must degenerate to a line`,
      );
    }
  });

  test("path construction is linear in segment count", () => {
    const build = (n) => () => {
      let ctx = cpu(10, 10).getContext("2d");
      ctx.beginPath();
      for (let i = 0; i < n; i++) ctx.lineTo(i % 10, (i * 7) % 10);
    };
    let small = timed(build(4000)),
      large = timed(build(16000));
    // 4x the segments: linear is ~4x, quadratic ~16x
    assert.ok(
      large < small * 8,
      `16k lineTo took ${large.toFixed(1)}ms against ${small.toFixed(1)}ms for 4k`,
    );
  });
});

describe("saveLayer survives transforms and clips (#35)", () => {
  const layered = (inside) => {
    let canvas = cpu(40, 40),
      ctx = canvas.getContext("2d");
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, 40, 40);
    ctx.fillStyle = "black";
    ctx.saveLayer(0.5);
    inside(ctx);
    ctx.fillRect(0, 0, 40, 40);
    ctx.restore();
    return px(ctx, 20, 20);
  };

  test("composites at the layer alpha", () => {
    let [r, , , a] = layered(() => {});
    assert.ok(Math.abs(r - 127) <= 1, `expected ~127, got ${r}`);
    assert.equal(a, 255);
  });

  for (const [name, inside] of Object.entries({
    translate: (ctx) => ctx.translate(1, 1),
    scale: (ctx) => ctx.scale(2, 2),
    "no-op rotate": (ctx) => ctx.rotate(0),
    clip: (ctx) => {
      ctx.beginPath();
      ctx.rect(0, 0, 40, 40);
      ctx.clip();
    },
    "nested save": (ctx) => {
      ctx.save();
      ctx.restore();
    },
  })) {
    test(`survives ${name}`, () => {
      let [r] = layered(inside);
      assert.ok(Math.abs(r - 127) <= 1, `expected ~127, got ${r}`);
    });
  }

  test("group opacity applies to the group, not each draw", () => {
    let ctx = cpu(40, 40).getContext("2d");
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, 40, 40);
    ctx.fillStyle = "black";
    ctx.saveLayer(0.5);
    ctx.translate(0, 0);
    ctx.fillRect(0, 0, 30, 30);
    ctx.fillRect(10, 10, 30, 30); // overlaps the first
    ctx.restore();
    let [overlap] = px(ctx, 20, 20),
      [single] = px(ctx, 5, 5);
    assert.ok(
      Math.abs(overlap - single) <= 1,
      `overlap ${overlap} must match single ${single}`,
    );
  });
});

describe("Paragraph compositing (#36)", () => {
  const paragraph = (style) => {
    let builder = ParagraphBuilder.Make({
      textStyle: { fontSize: 24, fontFamilies: ["sans-serif"], ...style },
    });
    builder.addText("Hg");
    let para = builder.build();
    para.layout(120);
    return para;
  };

  test("a decoration takes the text colour", () => {
    let canvas = cpu(120, 60),
      ctx = canvas.getContext("2d");
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, 120, 60);
    // 1 = underline, the TextDecoration bitmask
    let para = paragraph({ color: "red", decoration: 1 });
    ctx.drawParagraph(para, 0, 0);
    let data = ctx.getImageData(0, 0, 120, 60).data,
      seen = new Set();
    for (let i = 0; i < data.length; i += 4) {
      if (data[i + 3] === 255 && !(data[i] === 255 && data[i + 1] === 255))
        seen.add(`${data[i]},${data[i + 1]},${data[i + 2]}`);
    }
    // every inked pixel is a shade of red; a white underline would show up here
    for (const colour of seen) {
      let [r, g, b] = colour.split(",").map(Number);
      assert.ok(r >= g && r >= b, `unexpected ink ${colour}`);
    }
  });

  test("drawParagraph honours globalAlpha", () => {
    const ink = (alpha) => {
      let ctx = cpu(120, 60).getContext("2d");
      let para = paragraph({ color: "black" });
      ctx.globalAlpha = alpha;
      ctx.drawParagraph(para, 0, 30);
      let data = ctx.getImageData(0, 0, 120, 60).data,
        total = 0;
      for (let i = 3; i < data.length; i += 4) total += data[i];
      return total;
    };
    let full = ink(1),
      half = ink(0.5);
    assert.ok(full > 0, "the paragraph drew nothing");
    assert.ok(
      Math.abs(half / full - 0.5) < 0.1,
      `half-alpha ink ${half} against ${full}`,
    );
  });

  test("drawParagraph honours globalCompositeOperation", () => {
    let ctx = cpu(120, 60).getContext("2d");
    ctx.fillStyle = "red";
    ctx.fillRect(0, 0, 120, 60);
    let para = paragraph({ color: "black" });
    ctx.globalCompositeOperation = "destination-out";
    ctx.drawParagraph(para, 0, 30);
    let data = ctx.getImageData(0, 0, 120, 60).data,
      cleared = 0;
    for (let i = 3; i < data.length; i += 4) if (data[i] < 255) cleared++;
    assert.ok(cleared > 0, "destination-out cleared nothing");
  });
});

describe("imageSmoothingQuality high (#38)", () => {
  // a zone plate aliases visibly when minified with a cubic resampler
  const source = () => {
    let canvas = cpu(256, 256),
      ctx = canvas.getContext("2d"),
      image = ctx.createImageData(256, 256);
    for (let y = 0; y < 256; y++) {
      for (let x = 0; x < 256; x++) {
        let v = 127 * (1 + Math.cos((x * x + y * y) / 200)),
          i = (y * 256 + x) * 4;
        image.data[i] = image.data[i + 1] = image.data[i + 2] = v;
        image.data[i + 3] = 255;
      }
    }
    ctx.putImageData(image, 0, 0);
    return canvas;
  };

  const drawn = (src, quality, size) => {
    let ctx = cpu(size, size).getContext("2d");
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = quality;
    ctx.drawImage(src, 0, 0, size, size);
    return ctx.getImageData(0, 0, size, size).data;
  };

  const roughness = (data, size) => {
    let total = 0;
    for (let y = 0; y < size; y++)
      for (let x = 1; x < size; x++)
        total += Math.abs(data[(y * size + x) * 4] - data[(y * size + x - 1) * 4]);
    return total / (size * (size - 1));
  };

  test("minifying does not alias more than medium", () => {
    let src = source(),
      high = roughness(drawn(src, "high", 32), 32),
      medium = roughness(drawn(src, "medium", 32), 32);
    assert.ok(
      high <= medium + 0.5,
      `high ${high.toFixed(2)} must not alias more than medium ${medium.toFixed(2)}`,
    );
  });

  test("magnifying still uses the cubic resampler", () => {
    let src = cpu(8, 8),
      sctx = src.getContext("2d");
    sctx.fillStyle = "black";
    sctx.fillRect(0, 0, 8, 8);
    sctx.fillStyle = "white";
    sctx.fillRect(0, 0, 4, 4);
    let high = drawn(src, "high", 64),
      medium = drawn(src, "medium", 64);
    assert.notDeepEqual(
      Array.from(high),
      Array.from(medium),
      "an upscale must differ from trilinear",
    );
  });
});
