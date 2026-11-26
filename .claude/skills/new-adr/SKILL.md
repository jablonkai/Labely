---
name: new-adr
description: Write a new Architecture Decision Record for Labely in docs/adr/ — next free number, the 0000 template, the house style of recent ADRs, the README index row and the links from code, roadmap and feature matrix; also phase start and phase close-out ADRs and "as built" amendments to older ADRs. Use whenever a change makes a significant decision (new driver, vendor, transport, file format, command schema change, packaging, mobile surface, dropping or deferring a feature), whenever the user asks to record, document or justify a decision, or when another skill such as add-printer says an ADR is needed — even if they just say "write this up" or "document why". Hungarian triggers: "írj egy ADR-t", "új ADR", "rögzítsd a döntést", "dokumentáld a döntést", "zárd le a fázist ADR-rel", "fázis lezáró ADR".
---

# Write a Labely ADR

ADR-0001 says every significant decision is an ADR in `docs/adr/NNNN-slug.md`, using
`0000-template.md`. The ADRs are the project's memory: later work (and later agents) read them
to learn why the code is the way it is, and the code, roadmap and feature matrix cite them by
number. So an ADR must be accurate about what was actually built, and findable.

## 1. Pick the number and slug

```sh
ls docs/adr/[0-9]*.md | tail -1
```

Take the highest `NNNN` + 1, zero-padded to four digits. Check open branches too
(`git branch -a`, `git log --all --oneline -- docs/adr | head`) so two pull requests do not
claim the same number. Slug: short, lowercase, hyphenated topic (`0037-other-vendors.md`,
`0044-zpl-import.md`, `0040-phase-6-close-out.md`).

## 2. Write it from the template

`docs/adr/0000-template.md` is:

```markdown
# ADR-NNNN: Title

- Status: proposed | accepted | superseded by ADR-XXXX
- Date: YYYY-MM-DD

## Context
## Decision
## Consequences
```

House conventions, taken from ADR-0035 to ADR-0044:

- **Status** is `proposed` — every ADR in the repo is, so do not invent `accepted`. Use
  `superseded by ADR-XXXX` only when a later ADR fully replaces this one.
- **Date** is today, ISO format.
- **Title** is a short noun phrase. Add the roadmap slice or phase in brackets when the ADR
  belongs to one: `Other vendors (Phase 6.3)`, `NIIMBOT print tasks (Phase 6 start)`,
  `iOS build (Phase 5 start)`.
- **Context**: what the repo lacked or got wrong, in facts (counts, file names, issue numbers
  like `(issue #46)`), and constraints such as "Labely's only test printer is a B21 Pro". Name
  rejected alternatives and why (ADR-0044 rejected the Labelary web API).
- **Decision**: a bullet list, each bullet led by a bold summary (`- **Spec first.** …`), with
  sub-bullets or `*italic:*` lead-ins for variants. Name real code: crates, modules, types,
  commands, file paths, spec sections (`protocol.md §7.5`). Cite the sources and the clean-room
  basis when protocol facts are involved (ADR-0001).
- **Consequences**: what is still open and why, what has *not* run on hardware or a phone, the
  schema impact ("No command schema changed, so `API_VERSION` stays 3" — or the bump; see
  AGENTS.md "Schema changes"), and changed counts (models, commands).
- **Style**: short plain sentences, present tense, no marketing, no "will". Say UNCERTAIN or
  *unverified* where it is. Keep it to roughly 40–70 lines; ADR-0037 is a good length.

Write the ADR from the code you actually changed, not from the plan. Re-read the diff before
finishing and fix any Decision bullet the code contradicts.

## 3. Index it

Append a row to the table in `docs/adr/README.md`, in number order, same format:

```markdown
| [0045](0045-slug.md) | Short title | proposed |
```

The index title may be shorter than the ADR heading (ADR-0001's index row is
"Record decisions; clean-room policy").

## 4. Link it where it is relevant

Other files cite ADRs by number in brackets, e.g. `(ADR-0037)`. Add the links the change calls
for:

- **Code**: the module doc comment of a new or reworked module, e.g.
  `//! Data sources, column binding and serial numbers: \`data.*\` (ADR-0014).`
- **`docs/roadmap.md`**: the slice row in the phase progress table
  (`done (ADR-0037); … are open`), the phase intro list, and items moved to "Later".
- **`docs/feature-matrix.md`**: the "Labely plan" cell or the phase exit-status table.
- **Specs and architecture**: `docs/architecture.md`, `docs/protocol*.md`, `docs/printers.md`
  when the decision shapes them.
- **Older ADRs it changes**: do not rewrite history. Add a note to the old ADR instead:
  - an `**As built.**` paragraph when the result differs from what it said (ADR-0011);
  - a bracketed pointer in the changed bullet, e.g. `(ADR-0040 adds two digits, …)` (ADR-0039);
  - an `Update (#45): …` line under Consequences when later work resolves an open point
    (ADR-0036);
  - status `superseded by ADR-XXXX` only for a full replacement.

The ADR ships in the same pull request as the change it records (see `git show --stat 134e60e`:
code, `docs/adr/0037-other-vendors.md` and the README row together).

## 5. Phase start and close-out ADRs

Phases in `docs/roadmap.md` open with a "Phase N start" ADR and end with a close-out ADR
(ADR-0024, ADR-0029, ADR-0034, ADR-0040). If the user is finishing a phase, write a close-out
ADR with this shape — read ADR-0034 and ADR-0040 first:

- **Context**: what the phase added (the ADR range), its exit criteria, and the exit audit:
  which ADRs were checked against which crates/projects/docs, and what it found ("no open
  TODOs, one gap in the tests, two faults…"). Do the audit for real before writing it.
- **Decision** bullets, in this order:
  - **Hardware checks** that were not run, listed concretely, and where they move.
  - One bullet per fault fixed (what was wrong, what it does now).
  - **Tests** added. **Docs** brought in line. Older ADRs amended "as built".
  - **Left as it is, with reasons** / **Deferred**, each with its reason and target.
- **Consequences**: schema/API version, the roadmap slice marked audited, and that the
  hardware results are added to this ADR when they run.

Also add the phase exit-status table to `docs/feature-matrix.md` (see "Phase 6 exit status")
and update the roadmap progress.

## 6. Check

- `cargo xtask ci` if code changed alongside.
- `ls docs/adr` has no duplicate number; the README row links to an existing file.
- Every ADR number you cited exists and says what you claim.

Report the new file, the index row and every file you linked from.
