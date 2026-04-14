# Clean-Room Rules

## Allowed inputs
- Public README/docs
- Public screenshots/videos
- Public API/OpenAPI contracts
- Public config schemas
- Black-box behavior from a running deployment
- User workflows and acceptance criteria written in this repo

## Forbidden inputs for implementers
- Upstream source files
- Upstream database schema internals
- Upstream prompts / hidden system prompts
- Upstream tests
- Upstream UI assets, copywriting, icons, or code snippets
- Any direct copy-paste from the upstream repository

## Process split
1. Spec/design track creates parity specs, acceptance criteria, and user-visible behavior notes.
2. Implementation track works only from this repository's specs/plans.

## Goal
Match functionality, not code expression.
