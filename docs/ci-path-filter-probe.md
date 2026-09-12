# CI path-filter probe

This file exists only to prove that a docs-only change skips the six-target
build matrix. It is deleted with its throwaway branch.

- No `src/**`, `crates/**`, `Cargo.toml`, `Cargo.lock`, `web/**` or `assets/**`
  path is touched.
- The required `Multi-platform build` gate must still report success.
