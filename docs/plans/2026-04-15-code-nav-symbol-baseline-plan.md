# Code Navigation Symbol Baseline Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Deliver Phase 3 / Task 15 by adding a minimal symbol extraction baseline that can enumerate definition candidates from supported source files and expose a clean internal API for later definitions/references endpoints.

**Architecture:** Keep the first slice intentionally narrow. Add a lightweight symbol extraction module in `crates/search` that scans file contents for top-level symbol definitions using language-specific heuristics. Start with Rust support and explicit unsupported-language handling so later API work can build on stable internal contracts.

**Tech Stack:** Rust, serde, anyhow, existing `sourcebot-search` crate.

---

## Scope for Task 15

### In scope
- Internal symbol model with path + range metadata.
- Baseline extractor for Rust source files.
- Capability result for unsupported file types.
- Unit tests proving supported extraction and graceful unsupported behavior.

### Out of scope
- HTTP definitions endpoint.
- References lookup.
- Cross-repo navigation.
- Frontend navigation UI.
- Full tree-sitter indexing pipeline.

---

## Task 1: Add symbol extraction contracts
**Objective:** Define reusable data structures for symbol extraction results.

**Files:**
- Modify: `crates/search/src/lib.rs`

**Steps:**
1. Add `SymbolKind`, `SymbolRange`, `SymbolDefinition`, and `SymbolExtraction` types.
2. Represent extraction outcomes as either `supported` with symbols or `unsupported` with a capability message.
3. Keep field names JSON-friendly for future API use.

## Task 2: Write failing Rust extraction tests
**Objective:** Lock behavior before implementation.

**Files:**
- Modify: `crates/search/src/lib.rs`

**Steps:**
1. Add a test for extracting Rust `fn`, `struct`, `enum`, and `trait` definitions from a sample file.
2. Add a test proving unsupported extensions return a non-fatal capability response with no symbols.
3. Run the targeted cargo test and observe failure.

## Task 3: Implement minimal Rust extractor
**Objective:** Make the tests pass with the smallest useful implementation.

**Files:**
- Modify: `crates/search/src/lib.rs`

**Steps:**
1. Add a public extraction entrypoint that accepts `path` + `content`.
2. Detect Rust source files by extension.
3. Parse line-by-line using simple anchored heuristics for top-level `fn`, `struct`, `enum`, and `trait` declarations.
4. Return 1-based line ranges and discovered symbol names.

## Task 4: Refactor and verify
**Objective:** Clean the implementation without expanding scope.

**Files:**
- Modify: `crates/search/src/lib.rs`

**Steps:**
1. Refactor repetitive matching into helpers.
2. Run `cargo fmt --all`.
3. Run `cargo test`.
4. Run `cargo check`.

## Task 5: Hand-off for Task 16
**Objective:** Leave a clear foundation for the next roadmap task.

**Files:**
- No new files required if code is self-explanatory.

**Steps:**
1. Ensure types and function names are generic enough for later definitions/references APIs.
2. Keep unsupported-language behavior explicit so future endpoints can surface a capability response instead of a server error.
