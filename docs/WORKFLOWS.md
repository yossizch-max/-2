# Workflow Catalog

Risk classes:
- `SAFE_AUTO`
- `ONE_CLICK`
- `APPROVAL_REQUIRED`

## Canonical workflows

1. Bind Existing Matter
2. Create New Matter
3. Resume Matter
4. New File Triage
5. Court Decision → Action
6. Record Request
7. Missing Client Document
8. Medical Document → Chronology
9. Wage Document → Wage Ledger
10. Demand Readiness
11. Demand Draft
12. Negotiation Entry
13. Litigation Readiness
14. Pleading Draft
15. Hearing Prep
16. Legal Research
17. Outgoing Letter Follow-up
18. Source Changed
19. Close Matter
20. AI Connector Test

## Court Decision → Action

```text
decision source
→ extraction/source blocks
→ AI proposes explicit instructions
→ exact source validation
→ propose task/event/deadline
→ individual lawyer review
→ deterministic deadline ruleset if calculation is required
→ transaction commit
→ reminders
→ timeline event
```

No bulk deadline approval.

## Medical → chronology

AI may propose:
- provider
- visit
- diagnosis written in source
- test
- surgery
- hospitalization
- dates

AI may not determine:
- causation
- disability percentage
- legal entitlement

A missing-document period is a `gap proposal`, not a conclusion that no treatment occurred.

## Damage / demand

```text
collecting
→ completeness review
→ approved facts
→ approved deterministic calculation
→ approved outline
→ grounded draft
→ paragraph-source review
→ lawyer edit
→ approved immutable version
→ DOCX
→ explicit send outside/integration layer
→ follow-up
```

No `collecting → AI draft` shortcut.

## Legal research

No scraping of Nevo/Takdin.

Lawyer obtains the authorized source and imports/saves it to the Matter.

TAHRIR may then:
- analyze issue/rule/application
- identify passages
- propose research notes

Only lawyer-verified authority/passages become drafting sources.
