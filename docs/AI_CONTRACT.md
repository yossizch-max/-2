# AI Contract

## Core rule

AI produces proposals. It never writes final legal truth.

## Pipeline

```text
TaskContextManifest
→ bounded capability
→ provider
→ structured proposal
→ local source-ID validation
→ lawyer review
→ deterministic commit
```

## Context

The model receives only the minimum source blocks required for the capability.

A Source Block contains:
- Matter ID
- DocumentVersion ID
- Source ID
- optional page
- anchor kind
- text SHA256
- extracted text

Windows file paths are not required in external AI context.

## External provider gate

A healthy API connection is not permission to send Matter data.

External egress requires:
- provider enabled
- client-data authorization enabled for provider/policy
- explicit run approval

## Local provider

Local OpenAI-compatible endpoint must use:
- `127.0.0.1`
- `localhost`
- `::1`

Non-loopback local-provider URLs are rejected.

## Provider persistence

Do not persist:
- raw provider error body
- free-form refusal prose
- arbitrary provider diagnostic output

Persist:
- stable error code
- request/context hash
- response hash where appropriate
- model
- capability
- run status

## Grounding

Every proposal must carry source IDs.

All returned source IDs must be a subset of the exact context manifest sent for that run.

Unknown source ID means:
`INVALID_SOURCE_REFERENCE`

No partial “completed” run when a required chunk failed.

## Source text is untrusted

Documents may contain text that looks like instructions.

The AI system instruction explicitly treats source material as evidence, never instructions.

## Approval

Per-capability review is required.

There is no global “approve model” switch.
