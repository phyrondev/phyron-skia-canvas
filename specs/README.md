# Specs

Feature specs live in this directory. The active feature directory is stored in
`.specify/feature.json`.

Rules: `.blueprints/domain/spec-driven-development.md`. Constitution:
`.specify/memory/constitution.md`.

## Index

| Number | Surface | Status |
| --- | --- | --- |
| 001 | [Wide-gamut and HDR pixel export](001-wide-gamut-hdr-export/spec.md) | Active |

## Definition Of Covered

A surface is covered only when all of these are present:

- A feature spec exists in `specs/<feature>/spec.md`.
- Contracts exist in `specs/<feature>/contracts/`.
- Implementation tasks exist in `specs/<feature>/tasks.md`.
- Every contract matrix row is `Covered`, `Partial`, or `Open`.
- Each `Covered` row cites source evidence and either executable test evidence
  or explicit manual QA evidence.
- Each contract file contains a `Required Evidence Before Marking Complete`
  section.

Docs, checkboxes, and TODOs are not evidence by themselves.

## Active Spec

Current active feature: `specs/001-wide-gamut-hdr-export/plan.md`.
