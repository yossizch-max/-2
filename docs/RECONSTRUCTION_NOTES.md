# Reconstruction Notes

## Why this package exists

The preserved alpha.16 source checksum exists, but the actual source ZIP became unavailable/empty.

The surviving material is sufficient to reconstruct the architecture and product contract, but not to reproduce every historical byte.

## High-confidence preserved facts

The surviving record establishes:
- Tauri 2 + Rust + React/TypeScript
- local-first architecture
- SQLCipher and OS credential-store key
- Stage A metadata scan
- Stage B stable SHA
- Matter / Document / DocumentVersion / FileOccurrence model
- OneDrive placeholder safety boundary
- OCR
- AI providers
- Verified Facts
- damage calculator
- death-damage separation
- authorities
- legal documents
- paragraph provenance
- export audit
- 31-table alpha.16 scale
- 61-command alpha.16 frontend/backend parity
- Professional three-pane UI
- deep QA blockers and hardening order

## What this package does not claim

It does not claim that:
- these 61 reconstructed command names are the exact original names
- the file tree equals the original 113-file tree
- dependency versions equal every original pin
- Rust source is byte-identical
- lockfiles are recovered
- this package has passed Windows compilation

Where exact historical behavior could not be reconstructed safely, the command fails closed with:

`RECONSTRUCTED_COMMAND_NOT_YET_WIRED:<command>`

That is intentional.

## Design improvement over the lost UI source

The alpha.16 QA noted a very large `App.tsx`.

This reconstruction splits:
- shell
- command palette
- inspector
- global pages
- Matter workspace
- documents
- facts/AI
- damage
- tasks/calendar
- legal documents
- authorities

The design system is also split into tokens and application layout CSS.
