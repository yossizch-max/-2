# Lockfiles are intentionally not fabricated

The original alpha.16 QA found a Cargo.toml/Cargo.lock mismatch.

This reconstructed package therefore does not invent historical lockfiles.

Before any build can be called reproducible:

```powershell
npm install --package-lock-only
cd src-tauri
cargo generate-lockfile
```

Review the resulting dependency graph, then commit both lockfiles.

All CI/build commands after that must use locked resolution.
