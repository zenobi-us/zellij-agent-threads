# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

Layout: multi-context.

## Before exploring, read these

- **`CONTEXT-MAP.md`** at the repo root. It points at the relevant context docs for the area being changed.
- **Relevant `CONTEXT.md` files** named by `CONTEXT-MAP.md`.
- **`.memory/docs/adr/`** for system-wide decisions.
- **`.memory/docs/adr/<relativepath>/`** for context-scoped decisions.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The producer skill (`/eng-grill-with-docs`) creates them lazily when terms or decisions actually get resolved.

## File structure

```text
/
├── CONTEXT-MAP.md
├── .memory/docs/adr/                  ← system-wide decisions
└── <context>/
    ├── CONTEXT.md
    └── .memory/docs/adr/<context>/    ← context-specific decisions
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in the relevant `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/eng-grill-with-docs`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_
