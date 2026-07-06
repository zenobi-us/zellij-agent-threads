# Research: MiniJinja extension mechanisms for layout/grid helper in Rust

Access date: 2026-07-06  
Repo context: `zellij-agent-threads` / Rust Zellij plugin

## Thinking

Question: can this repo create a MiniJinja layout block for flex/grid-style terminal layout?

Short answer: yes, but **not as a new custom `{% grid %}` tag** unless MiniJinja itself is patched or templates are preprocessed. The practical path is a **call-block function**:

```jinja
{% call Grid(cols=3, gap=1) %}
item A
item B
item C
{% endcall %}
```

State machine:

```text
Need layout helper
  -> Need new syntax? (`{% grid %}`)
       -> MiniJinja public API: no custom statement registration
       -> parser source: statement names hardcoded
       -> reject unless upstream patch/preprocessor
  -> Need process rendered block body?
       -> use `{% call Grid(...) %}` function/object
  -> Need process rendered string only?
       -> use `{% filter grid(...) %}`
  -> Need one expression helper?
       -> use `{{ grid(items, cols=3) }}` function
```

Skills used:

- `lynx-web-search`: selected because task required internet research. `lynx` binary missing, so I used `curl` fallback and stored page dumps in `/tmp/minijinja-research/`.
- `code-library-docs`: selected because MiniJinja is an unfamiliar library/API; used cache-first/docs-first method, then cloned official source for parser/API verification.
- `codemapper`: selected for repo/code structure approach; actual structural need was small, so I used source reads and exact parser/API locations rather than broad symbol maps.
- `ponytail`: active; conclusion favors smallest working API (`call` function) over new syntax/preprocessor.

## Research

### Sources inspected

1. MiniJinja docs.rs crate docs — official Rust API docs, publisher: docs.rs / MiniJinja crate authors, URL: https://docs.rs/minijinja/latest/minijinja/
2. MiniJinja `Environment` docs — official API docs, URL: https://docs.rs/minijinja/latest/minijinja/struct.Environment.html
3. MiniJinja `functions` docs — official API docs, URL: https://docs.rs/minijinja/latest/minijinja/functions/index.html
4. MiniJinja `Function` trait docs — official API docs, URL: https://docs.rs/minijinja/latest/minijinja/functions/trait.Function.html
5. MiniJinja `Object` trait docs — official API docs, URL: https://docs.rs/minijinja/latest/minijinja/value/trait.Object.html
6. MiniJinja `SyntaxConfig` docs — official API docs, URL: https://docs.rs/minijinja/latest/minijinja/syntax/struct.SyntaxConfig.html
7. MiniJinja official source clone, commit `1d3e73d39a7fd4a31c369209dbcb036fcf1a322c`, URL: https://github.com/mitsuhiko/minijinja
8. MiniJinja call-block function example, URL: https://github.com/mitsuhiko/minijinja/tree/main/examples/call-block-function
9. MiniJinja syntax-highlighting call-block object example, URL: https://github.com/mitsuhiko/minijinja/tree/main/examples/syntax-highlighting
10. MiniJinja GitHub issue #200 “Custom block tags”, URL: https://github.com/mitsuhiko/minijinja/issues/200
11. MiniJinja GitHub discussion #178 “Custom Blocks Proposal”, URL: https://github.com/mitsuhiko/minijinja/discussions/178
12. Jinja official extensions docs for contrast, URL: https://jinja.palletsprojects.com/en/stable/extensions/

### Claim 1 — MiniJinja supports extension via filters, tests, globals, functions, objects, syntax delimiter config; not via public custom block-tag API

Evidence:

- MiniJinja `Environment` exposes `add_filter`, `add_test`, `add_function`, `add_global`, plus `set_syntax`. Source type: official API docs. Publisher: docs.rs / MiniJinja. Confidence: HIGH. Contradictions: none in public docs.
  - https://docs.rs/minijinja/latest/minijinja/struct.Environment.html
- `SyntaxConfig` only documents delimiter/prefix customization: block, variable, comment delimiters, line statement/comment prefixes. Source type: official API docs. Publisher: docs.rs / MiniJinja. Confidence: HIGH. Contradictions: none.
  - https://docs.rs/minijinja/latest/minijinja/syntax/struct.SyntaxConfig.html
- Parser source has a hardcoded statement match list: `for`, `if`, `with`, `set`, `autoescape`, `filter`, `block`, `extends`, `include`, `import`, `from`, `macro`, `call`, `continue`, `break`, `do`; unknown names error with `unknown statement`. Source type: official source. Publisher: MiniJinja GitHub. Confidence: HIGH. Contradictions: GitHub issue proposes custom tags, but it is open/proposal, not implemented.
  - https://github.com/mitsuhiko/minijinja/blob/main/minijinja/src/compiler/parser.rs

Interpretation: MiniJinja has extensibility hooks for values and callable behavior. It does **not** currently have a public “register custom tag/block parser” API like Python Jinja.

### Claim 2 — Python Jinja has custom extension parser support; MiniJinja intentionally does not expose equivalent parser internals today

Evidence:

- Jinja official docs say extensions can add filters/tests/globals and “extend the parser”; “Writing Extensions” says custom tags are possible but non-trivial and usually not needed. Source type: official docs. Publisher: Pallets/Jinja. Confidence: HIGH. Contradictions: applies to Python Jinja, not MiniJinja.
  - https://jinja.palletsprojects.com/en/stable/extensions/
- MiniJinja issue #200 is explicitly titled “Custom block tags” and remains open. Source type: official project issue. Publisher: MiniJinja GitHub. Confidence: HIGH. Contradictions: open issue means desired by users, not available as stable API.
  - https://github.com/mitsuhiko/minijinja/issues/200
- In issue #200 comments, Armin Ronacher (`mitsuhiko`, owner) says exposing lexer/parser/codegen internals would “blow up the complexity of the API surface”; later: exposing the entire parser would be “pretty crazy” and references hidden `unstable-machinery`. Source type: maintainer comment. Publisher: MiniJinja GitHub. Confidence: HIGH. Contradictions: maintainer mentions preprocessing as possible future path, but skeptical/blockers remain.
  - https://github.com/mitsuhiko/minijinja/issues/200

Interpretation: If you require literal `{% grid %}...{% endgrid %}`, current MiniJinja pushes you toward preprocessing or forking/upstream changes. Bad trade for this repo.

### Claim 3 — `{% call %}` is the best mechanism for a layout/grid block helper

Evidence:

- MiniJinja syntax docs describe `{% call %}` as declaring an anonymous macro and passing it as `caller`; supports arguments back into the call block. Source type: official docs/source comments. Publisher: MiniJinja. Confidence: HIGH. Contradictions: requires `macros` feature, included by default per docs.
  - https://github.com/mitsuhiko/minijinja/blob/main/minijinja/src/syntax.rs
- Official `examples/call-block-function` implements `custom_loop(state, num, kwargs)`, extracts `caller` from `Kwargs`, calls `caller.call(state, args!(...))`, and registers it with `env.add_function`. Source type: official example. Publisher: MiniJinja. Confidence: HIGH. Contradictions: example is loop, not grid, but same block-body capture mechanism.
  - https://github.com/mitsuhiko/minijinja/tree/main/examples/call-block-function
- Official `examples/syntax-highlighting` implements a callable `Object` whose `call` extracts `caller`, renders content, transforms it, returns safe HTML. Source type: official example. Publisher: MiniJinja. Confidence: HIGH. Contradictions: object route is heavier than plain function unless stateful object needed.
  - https://github.com/mitsuhiko/minijinja/tree/main/examples/syntax-highlighting
- Issue #200: maintainer replies to syntax-highlighting custom block request with “why can you not use the `{% call %}` block” and later says “for as long as you can get away with `call` that is the way to go.” Source type: maintainer guidance. Publisher: MiniJinja GitHub. Confidence: HIGH. Contradictions: some uses like external AST annotations or cache semantics may still want custom tags.
  - https://github.com/mitsuhiko/minijinja/issues/200

Interpretation: For grid/flex layout, `{% call Grid(...) %}` is exactly the intended MiniJinja escape hatch: block-ish template syntax without custom parser work.

### Claim 4 — A filter block is viable only when the helper needs a rendered string, not caller-state/control semantics

Evidence:

- MiniJinja syntax docs: `{% filter %}` applies regular filters to a block of template data. Source type: official docs/source comments. Publisher: MiniJinja. Confidence: HIGH. Contradictions: none.
  - https://github.com/mitsuhiko/minijinja/blob/main/minijinja/src/syntax.rs
- `Environment::add_filter` registers filter functions applied to values in templates. Source type: official API docs. Publisher: docs.rs / MiniJinja. Confidence: HIGH. Contradictions: none.
  - https://docs.rs/minijinja/latest/minijinja/struct.Environment.html#method.add_filter
- Issue #200 maintainer asks whether `{% spaceless %}` is much improvement over `{% filter spaceless %}`, implying rendered-block transforms fit filters. Source type: maintainer comment. Publisher: MiniJinja GitHub. Confidence: MEDIUM. Contradictions: commenter notes filters are insufficient for block caching / AST annotation use cases.
  - https://github.com/mitsuhiko/minijinja/issues/200

Interpretation: `{% filter grid(cols=3) %}` could work if grid layout receives raw rendered text and returns text. But call-block function is more flexible and matches existing repo pattern.

### Claim 5 — This repo already uses MiniJinja call-block functions; minimum integration path is small

Evidence:

- `pkgs/plugins/zellij-plugin/src/render/template.rs` creates `Environment::new()` and calls `add_template_helpers(&mut env)`. Source type: repo source. Publisher: local repo. Confidence: HIGH. Contradictions: none.
- `pkgs/plugins/zellij-plugin/src/render/filters.rs` registers filters and button helpers via `add_template_helpers`. Source type: repo source. Publisher: local repo. Confidence: HIGH. Contradictions: none.
- `pkgs/plugins/zellij-plugin/src/render/click.rs` registers `PaneButton`/`TabButton` with `env.add_function`, extracts `caller` from `Kwargs`, and renders call-block content via `caller.call(state, &[])`. Source type: repo source. Publisher: local repo. Confidence: HIGH. Contradictions: none.
- Verification command `moon run zellij-plugin:test` passed: 30 tests. Source type: local test run. Publisher: repo tooling. Confidence: HIGH.

Interpretation: Add `Grid` beside `PaneButton`/`TabButton`, or one new `layout.rs` if code grows. No new dependency. No parser work.

## Feasibility matrix

| Mechanism | Template shape | Feasible? | Best use | Verdict |
|---|---|---:|---|---|
| Custom block tag | `{% grid cols=3 %}...{% endgrid %}` | No public API | Exact DSL syntax | Reject. Needs MiniJinja fork/preprocessor/upstream API. |
| Call-block function | `{% call Grid(cols=3) %}...{% endcall %}` | Yes | Process body with args/state | Best path. |
| Filter block | `{% filter grid(cols=3) %}...{% endfilter %}` | Yes | Pure rendered string transform | Good fallback, less flexible. |
| Global function | `{{ grid(items, cols=3) }}` | Yes | Data-driven list layout | Good if content is data, not template body. |
| Custom object | `{% call grid.render(cols=3) %}` or `{% call Grid(...) %}` | Yes | Stateful/configured helper | Use only if plain function too small. |
| SyntaxConfig | custom delimiters/prefixes | Yes | Change `{{` / `{%` markers | Not relevant to new tag semantics. |
| Preprocessor | rewrite `{% grid %}` to `{% call Grid(...) %}` | Possible | Preserve nicer DSL | Avoid unless user insists. Source maps/errors get worse. |

## Minimum integration path in this repo

Lazy path:

1. Add `env.add_function("Grid", grid)` inside existing template helper registration. Probably in `render/filters.rs` if tiny; split `render/layout.rs` only if code exceeds trivial size.
2. Implement `fn grid(state: &State<'_, '_>, kwargs: Kwargs) -> Result<String, Error>`.
3. Extract options: `cols`, `gap`, maybe `width`. Extract `caller: Value = kwargs.get("caller")?`; `kwargs.assert_all_used()?`.
4. Render block body: `let body = state.format(caller.call(state, &[])?)?;`.
5. Parse items by newline or explicit separator. Keep dumb first. Terminal flexbox is a trap.
6. Pad/truncate with existing terminal-width assumptions if present; add one unit test.

Likely first template API:

```jinja
{% call Grid(cols=3, gap=2) %}
{{ pane.title }}
{{ pane.status }}
{{ pane.cwd }}
{% endcall %}
```

Avoid for now:

- New `{% grid %}` syntax.
- Preprocessor.
- CSS-like flex model.
- Object abstraction.
- New crate.

## Verification

Commands/actions:

- Cloned official MiniJinja source: `git clone --depth 1 https://github.com/mitsuhiko/minijinja.git /tmp/minijinja-src`; commit verified: `1d3e73d39a7fd4a31c369209dbcb036fcf1a322c`.
- Downloaded docs/API pages to `/tmp/minijinja-research/` via `curl` because `lynx` was not installed.
- Inspected parser source showing hardcoded statement names and unknown-statement error.
- Inspected official examples for `call` function/object usage.
- Inspected local repo MiniJinja helper registration and call-block button implementation.
- Ran `moon run zellij-plugin:test`; result: 30 passed.

Reality check: evidence supports call-block helper. Evidence does not support public custom MiniJinja block-tag registration.

## Insights

1. MiniJinja’s “extension API” is value/callable registration, not parser plugins.
2. Maintainer bias is clear: avoid custom syntax; use `call` while possible.
3. This repo already solved the hard part with `PaneButton`: call-block body capture, state access, temp state, tests.
4. For grid layout, custom tag syntax gives little value and adds parser/source-map/error complexity.
5. A filter block is tempting but weaker. It transforms output after render; a call function can still do that while keeping access to state and kwargs.
6. [bias: ponytail] Start with newline-separated items and fixed columns. Add flex semantics only after templates prove need.

## Summary

Feasible: **yes**, as a MiniJinja call-block function/helper.

Not feasible cleanly: **custom `{% grid %}` block tag** using public MiniJinja API.

Recommended implementation:

```text
render/template.rs -> Environment::new()
  -> add_template_helpers(env)
    -> add_function("Grid", grid)
      -> grid(state, kwargs)
        -> caller.call(state, &[])
        -> layout rendered lines/items
        -> return String
```

Best first issue/slice: implement `Grid(cols, gap)` call-block helper with newline-separated children and tests. Skip preprocessor/custom tag until someone proves `{% call Grid %}` is unacceptable.
