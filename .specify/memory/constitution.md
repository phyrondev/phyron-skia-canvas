# Project Spec Constitution

## Core Principles

### I. Source Of Truth First

The active spec directory named in `.specify/feature.json`, or the
project-specific equivalent, governs feature behavior. Code, plans, TODOs,
and agent claims must point back to contract files. If implementation and spec
disagree, update the spec or reject the implementation before continuing.

### II. Contract Evidence Is Required

Every feature surface needs contracts with a matrix row for each important
behavior. Each row is `Covered`, `Partial`, or `Open`. `Covered` requires
source evidence plus executable test evidence or explicit manual QA evidence.
Untested documentation is not evidence.

### III. Small User Stories Beat Broad Architecture Docs

Slice specs by independently testable user stories. A single session should
target one user story or one contract gap. Architecture summaries may exist as
context, but they are not a substitute for acceptance criteria, contracts, and
tasks.

### IV. Tests Follow Contracts

Derive tests from contract preconditions, postconditions, invariants, and user
acceptance criteria. Deterministic model logic needs automated tests.
Interactive behavior needs automated tests when practical and explicit manual QA
when browser-only behavior cannot be covered cheaply.

### V. Persistence And Migration Are Product Behavior

Project files, clipboard payloads, local/session storage, server/client
adapters, and migration paths are contract surfaces. Any schema or wire-format
change must document load behavior, save behavior, compatibility, and failure
mode. Silent fallback on required identifiers is forbidden.

### VI. UI/UX Invariants Are Contracts

Selection, hit testing, drag semantics, viewport overlays, timeline state,
property-panel edits, and command dispatch must have explicit invariants. If a
UI control mutates state, a contract must name the target state path and the
undo/history expectation.

### VII. Shared Logic Has One Owner

Code needed by multiple runtimes, languages, packages, or services must have one
named owner. Document temporary duplication as `Partial`, with a removal task.

## Required Feature Artifacts

Each feature surface must contain:

- `spec.md`.
- `plan.md`.
- `research.md`.
- `data-model.md`.
- `contracts/*.md`.
- `quickstart.md`.
- `tasks.md`.
- `checklists/requirements.md`.

## Review Gates

- Project check/lint/test commands must pass before claiming readiness unless a
  known unrelated failure is documented with exact output.
- Relevant package tests must run for every changed surface.
- Visual baseline update commands require explicit human approval.
- Tick plans and TODO checkboxes only after their contract evidence is present.

## Governance

This constitution applies unless a branch-specific constitution explicitly
overrides it. Amendments require a commit changing this file, an explanation in
the PR or handoff, and updates to affected specs/templates.
