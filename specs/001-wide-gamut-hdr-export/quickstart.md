# Quickstart: Wide-Gamut and HDR Pixel Export

## Prerequisites

- `gh` authenticated for `phyrondev/phyron-skia-canvas`.
- For manual QA: a host with a real GPU (Vulkan or Metal) and Node 22.

## Validation Commands

Evidence runs locally (Linux, Vulkan GPU) and in CI for the OS matrix.

```bash
git push -u origin feat/wide-gamut-hdr-export
gh workflow run test.yml -R phyrondev/phyron-skia-canvas --ref feat/wide-gamut-hdr-export   # build + node --test, 3 OS x 2 Node
gh run list -R phyrondev/phyron-skia-canvas --branch feat/wide-gamut-hdr-export             # rust-ci.yml runs on the PR (fmt, clippy, cargo test)
gh run watch -R phyrondev/phyron-skia-canvas <run-id>
```

On a machine where builds are fine:

```bash
just ci
node --test tests/suite/color.test.js
```

## Manual QA

1. GPU fallback, on a GPU host:

   ```bash
   just build
   node -e '
   const { Canvas } = require("./lib");
   const c = new Canvas(3, 1, { colorType: "RGBAF32", colorSpace: "rec2020-linear" });
   const ctx = c.getContext("2d");
   ctx.fillStyle = [2, 2, 2, 1]; ctx.fillRect(0, 0, 3, 1);
   c.toBuffer("raw", { colorType: "RGBAF32", colorSpace: "rec2020-linear" }).then(b => {
     console.log(c.engine.renderer, c.engine.fallback, new Float32Array(b.buffer, b.byteOffset, 4));
   });'
   ```

   Expected: `GPU`, `RGBAF32` (or `undefined` if the GPU allocates float
   surfaces), and `[2, 2, 2, 1]`.

## Evidence Recording

After running validation, update contract rows with:

- Source evidence (file and symbol).
- Test name and CI run URL, or the manual QA output.
- Commit hash.
