const assert = require("assert");
const { describe, test } = require("node:test");
const { PLATFORM_PACKAGES, loadSkiaNode } = require("../../lib/binary");
const manifest = require("../../package.json");

// Targets are named in three places that must agree: the map in lib/binary.js, the
// `optionalDependencies` it probes for, and the `prebuild` hashes. Adding a platform to one and
// forgetting another fails silently — resolution finds nothing and falls back to the install
// script, which is the behaviour all of this exists to replace.
//
// The loader map is the source of truth. `prebuild` is empty in a source checkout, and
// `optionalDependencies` is absent until the platform packages for this version are published —
// declaring it earlier leaves `npm ci` with an unresolvable lockfile entry. Both are therefore
// checked only once present.
const declared = Object.values(PLATFORM_PACKAGES)
  .flatMap((byArch) => Object.values(byArch).flat())
  .sort();

const prebuilt = Object.keys(manifest.prebuild || {})
  .filter((asset) => asset.endsWith(".gz"))
  .map((asset) => `phyron-skia-canvas-${asset.replace(/\.gz$/, "")}`)
  .sort();

const optional = Object.keys(manifest.optionalDependencies || {}).sort();

describe("native binary resolution", () => {
  test("every target the loader probes has an optional dependency", { skip: optional.length === 0 && "platform packages not published yet" }, () => {
    assert.deepStrictEqual(optional, declared);
  });

  test("every released binary has a platform package", { skip: prebuilt.length === 0 && "no release snapshotted yet" }, () => {
    assert.deepStrictEqual(prebuilt, declared);
  });

  // Exact pins, as sharp and esbuild do: a range would let a consumer resolve a binary built from
  // different sources than the JavaScript wrapping it.
  test("optional dependencies pin the current version", () => {
    for (const [name, range] of Object.entries(manifest.optionalDependencies || {})) {
      assert.strictEqual(range, manifest.version, `${name} must pin ${manifest.version}`);
    }
  });

  test("resolves a usable binary on this host", () => {
    const skiaNode = loadSkiaNode();
    assert.strictEqual(typeof skiaNode.backend, "function");
    assert.ok(Object.keys(skiaNode).length > 0);
  });
});
