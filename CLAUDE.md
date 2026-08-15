# Llika

A schematic metro map generator. Takes a real metro network and draws it in the
straight-line, mostly-45-degree style of a transit poster.

## Development flow

This repo is developed spec-driven. Two artifacts, one job each:

- **`specs/<name>_spec.md`** — why we decided something, and the plan. Append-only once
  `accepted`; it does not track the code and may drift from it.
- **`rules/<subsystem>.md`** — what is true right now. It **does** track the code, and is
  corrected against its own sources rather than rewritten from scratch — freely, with no
  dated note. Each one declares its own `sources`, `covers` and `max_lines`.

The methodology is `/Users/ivapo/dev/main/spec-driven-dev/spec-authoring.md` — the
frontmatter schema, the phase gate, the review loop. Section references below (`§6.1`,
`§3`) resolve against that file. Spec ids are `llk-NNN`, allocated as `max(existing) + 1`
and never reused.

**The observable this project produces is: a schematic metro map — the network drawn as
an octilinear, straight-line, mostly-45-degree transit diagram, delivered in v1 as one
static SVG file a person opens and reads as a transit poster.** Every phase says whether
it produces one; a phase that does not is argued for explicitly.

**Before drafting or changing a spec, read `specs/INDEX.md`. Before changing a
subsystem, read `rules/INDEX.md`.** Both are generated from frontmatter by
`spec-lint --write-index` — never hand-edit them.

**A phase is not cleared to build until its own review round has converged** — that is
`reviewed` on the phase, not `status` on the document. Run `/review-spec <spec> --phase N`.

**When a conversation settles on a feature, work §6.1's ordered test before assuming a new
document.** Step 0 asks whether a decision changed at all; step 2 — append a phase to the
spec that owns the subject — is the commonest real answer, and the one a fresh context is
least likely to reach for.

**"Implement Phase N of `specs/X`" carries two standing plan steps and a close-out** (§3).
The plan states a **commit plan** — a phase is one plan, one push, and as many commits as
the work wants — and a **reconciliation step** naming which `rules/` files, which
user-facing documentation and which stanza the phase changes, or "none needed" with a
reason. When the exit gate passes, **write that phase's `shipped` date** into `phases[]`:
`/review-spec` owns `reviewed`, and nothing else owns `shipped`.
