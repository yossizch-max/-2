# Canonical Architecture

## Product sentence

TAHRIR is a calm, local-first legal workspace that turns incoming Matter material into verified context and the next safe action, without forcing file migration, repeated data entry or trust in a black box.

## Canonical graph

```text
OFFICE FILESYSTEM
   │
   ├─ Stage A metadata scan
   ├─ Stage B stable content identity
   └─ text extraction / OCR
          │
          ▼
SOURCE GRAPH
Matter → Document → DocumentVersion → Page / Source Block
          │
          ├───────────────────────┐
          ▼                       ▼
LEGAL LEDGER                  ACTION GRAPH
Verified Facts               Tasks
Fact conflicts               Waiting For
Legal Deadlines              Calendar
Damage snapshots             Reviews
Authorities                  Stage state
Legal document versions
          │                       │
          └───────────┬───────────┘
                      ▼
                  DOMAIN EVENTS
                      │
          ┌───────────┼─────────────┐
          ▼           ▼             ▼
        Today      Timeline    Action Center
                      │
                      ▼
       Deterministic services / AI capabilities
                      │
                      ▼
               Human review → commit
```

## Sources of truth

### Files
The existing office folder tree is authoritative for original files.

TAHRIR does not silently move, rename, delete or overwrite existing source files.

### Legal knowledge
Local encrypted SQLite is authoritative for knowledge created by TAHRIR:
- tasks
- deadlines
- verified facts
- reviews
- calculations
- authorities
- approved legal documents
- export audit
- stage state

## Matter boundary

Matter is an integrity boundary.

The same SHA256 appearing in two Matters does not merge their:
- Document
- DocumentVersion
- facts
- calculations
- authorities
- legal document sources

Cross-Matter legal references must fail at the database boundary.

## File identity

`Document`
is logical identity inside one Matter.

`DocumentVersion`
represents content identity and records SHA256.

`FileOccurrence`
represents a physical locator and local/cloud state.

Path is a locator, not the only future identity. The schema reserves:
- `path_display`
- `path_key`
- `volume_serial`
- `file_id_128`

## Scanner

Stage A:
- metadata only
- no intentional file-content read
- batched writes
- existing index remains available
- no cloud-placeholder hydration

Stage B:
- stable local file only
- size/mtime checked around hash
- SHA256 materializes DocumentVersion

## Extraction

Extraction operates on an immutable private source snapshot.

Flow:

```text
indexed SHA
→ copy exact source bytes to private temp snapshot
→ SHA snapshot
→ compare to indexed SHA
→ extract from snapshot only
→ verify snapshot unchanged
→ persist extracted blocks
```

A mismatch persists zero extracted pages.

PDF:
1. native text via Poppler
2. if insufficient text, rasterize with Poppler
3. OCR with Tesseract using heb + ara + eng

DOCX:
- local XML extraction
- document-level provenance
- no fake page number
- stable page citations require future rendering to a controlled PDF snapshot

## AI

AI is capability-based, not a general autonomous agent.

Examples:
- classify_document
- extract_decision_actions
- extract_fact_candidates
- extract_medical_events
- extract_wage_data
- analyze_authority
- build_demand_outline
- draft_grounded_paragraphs
- prepare_hearing_brief

Two separate gates:
1. technical provider connection
2. client-data authorization

Provider rules:
- OpenAI endpoint fixed to `https://api.openai.com/v1`
- local compatible provider must be loopback
- redirects disabled
- local provider bypasses system proxy
- external request uses `store:false`
- external request uses `background:false`
- no AI tools
- provider secret stored in OS keyring
- provider refusal/error prose is not persisted
- source IDs are validated locally
- one unsupported source reference rejects the proposal

## Deterministic legal core

AI never owns:
- legal-deadline calculation
- money calculation
- damage formulas
- authority verification
- final fact verification
- final draft approval

Money is stored in integer cents.

LegalDeadline stores:
- trigger source
- rule/ruleset
- calculation snapshot
- due date
- commit state
- supersession

## Reset semantics

There is no future `reset everything`.

Rebuildable:
- source occurrence index
- extraction cache

Durable:
- tasks
- deadlines
- verified facts
- review history
- locked calculations
- verified authorities
- approved legal documents
- export audit

## Desktop UI

RTL three-pane structure:
- Inspector on left
- Main workspace center
- Navigation rail right

Top-level:
1. Today
2. Matters
3. Action Center
4. Calendar
5. Search
6. Templates
7. AI
8. Settings

AI appears inside legal work surfaces, not as the center of the product.
