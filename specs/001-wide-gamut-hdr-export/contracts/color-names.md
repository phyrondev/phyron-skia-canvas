# Contract: Colour Space and Colour Type Names

## Scope

This contract covers parsing and reporting of `colorSpace` and `colorType`
strings at every JS entry point: `new Canvas`, export options, `ImageData`,
`getImageData`, and the `colorSpace` getters.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| N1: unknown `colorSpace` (`rec2020-pqq`) throws `TypeError` naming the value and the accepted names, at canvas creation and at export | Open | None | None | `tests/suite/color.test.js` "rejects unknown colour spaces" |
| N2: every canonical name round-trips through a canvas `colorSpace` getter | Open | None | None | `tests/suite/color.test.js` "reports canonical colour space names" |
| N3: every alias is accepted and reads back canonical | Open | None | None | `tests/suite/color.test.js` "accepts colour space aliases" |
| N4: unknown `colorType` throws `TypeError` at canvas creation and at export | Open | None | None | `tests/suite/color.test.js` "rejects unknown colour types" |

## Invariants

- No name maps to a fallback space or type.
- The canonical set has exactly eight names.

## Failure Modes

- `new_cicp` returns `None` for a valid name: `TypeError` naming the value.

## Required Evidence Before Marking Complete

- Source evidence: `to_color_space`, `from_color_space`, `to_color_type` and
  their callers in `src/node/utils.rs`, `src/node/canvas.rs`,
  `src/node/image.rs`, `src/context/api.rs`.
- Executable evidence: the named tests, green in the `test.yml` CI run.
