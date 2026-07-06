# Research: MiniJinja grid/flex layout block synthesis

Access date: 2026-07-06

## Thinking

Question: can `zellij-agent-threads` create a MiniJinja layout block for grid/flex-style layout?

Answer: **yes**, but use MiniJinja-native call blocks, not a new custom `{% grid %}` tag.

```text
Need layout in template
  -> Need arbitrary body content?
      -> use `{% call Grid(...) %}...{% endcall %}`
  -> Need data-only list layout?
      -> optional `{{ grid(items, ...) }}` later
  -> Need literal `{% grid %}` syntax?
      -> MiniJinja public API does not expose custom tag parser hooks
      -> avoid fork/preprocessor
  -> Need real CSS flex/grid?
      -> maybe Taffy later; v1 should be fixed grid/flex-lite
```

[bias: ponytail] Smallest useful feature wins: caller-block helper + fixed/manual width + tests. CSS-grade layout engine is speculative debt.

## Research

Subtopic files:

- `.memory/minijinja-layout-grid/research-4d8e2b1a-minijinja-layout-grid-minijinja-extension-mechanisms.md`
- `.memory/minijinja-layout-grid/research-2276f95d-minijinja-layout-grid-tui-layout-algorithms.md`
- `.memory/minijinja-layout-grid/research-9fcb775-minijinja-layout-grid-repo-rendering-fit.md`
- `.memory/minijinja-layout-grid/research-9f3a2c-minijinja-layout-grid-template-api-design.md`

### Major claim 1: MiniJinja can support this as a call-block function

Evidence:

- MiniJinja `Environment` exposes `add_function`, `add_filter`, `add_global`, `set_syntax`. Source: official docs.rs API docs, MiniJinja/docs.rs. URL: https://docs.rs/minijinja/latest/minijinja/struct.Environment.html. Confidence: HIGH. Contradictions: none.
- MiniJinja official examples include call-block functions/objects using hidden `caller`. Source: MiniJinja GitHub examples. URL: https://github.com/mitsuhiko/minijinja/tree/main/examples/call-block-function. Confidence: HIGH. Contradictions: example is loop/highlighting, not layout, but mechanism matches.
- This repo already registers `PaneButton`/`TabButton` as MiniJinja functions in `pkgs/plugins/zellij-plugin/src/render/click.rs` and captures template output with `render_captured` in `pkgs/plugins/zellij-plugin/src/render/template.rs`. Source: local repo. Confidence: HIGH.

Recommended template shape:

```jinja
{% call Grid(cols=3, gap=2, width=80) %}
{% call PaneButton(pane="1") %}Logs{% endcall %}
{% call TabButton(tab=2) %}Tests{% endcall %}
Plain cell
{% endcall %}
```

### Major claim 2: custom `{% grid %}` syntax is the wrong path

Evidence:

- MiniJinja `SyntaxConfig` customizes delimiters/prefixes, not new statement tags. Source: official docs.rs API docs, MiniJinja/docs.rs. URL: https://docs.rs/minijinja/latest/minijinja/syntax/struct.SyntaxConfig.html. Confidence: HIGH.
- MiniJinja parser source has hardcoded statement names; unknown names error as unknown statements. Source: official MiniJinja GitHub source. URL: https://github.com/mitsuhiko/minijinja/blob/main/minijinja/src/compiler/parser.rs. Confidence: HIGH.
- MiniJinja issue #200 / discussion #178 show custom blocks are proposed/requested, not stable public API. Maintainer recommends `{% call %}` where possible. Source: MiniJinja GitHub. URLs: https://github.com/mitsuhiko/minijinja/issues/200 and https://github.com/mitsuhiko/minijinja/discussions/178. Confidence: HIGH. Contradictions: hidden/unstable machinery may exist; using it would be brittle.
- Python Jinja supports parser extensions, but that evidence applies to Jinja, not MiniJinja. Source: Pallets Jinja docs. URL: https://jinja.palletsprojects.com/en/stable/extensions/. Confidence: HIGH for Jinja, LOW as MiniJinja implementation evidence.

### Major claim 3: terminal layout must use cell/display width, not byte/string length

Evidence:

- Ratatui layout docs model UI as `Rect` in terminal cells with constraints (`Length`, `Percentage`, `Ratio`, `Min`, `Max`, `Fill`) and gaps/nesting patterns. Source: official Ratatui docs. URL: https://ratatui.rs/concepts/layout/. Confidence: HIGH.
- `unicode-width` exists to compute displayed width of `char`/`str` according to Unicode width rules. Source: docs.rs crate docs. URL: https://docs.rs/unicode-width/latest/unicode_width/. Confidence: HIGH. Contradiction: terminal emulators can differ for ambiguous/CJK width.
- `textwrap` docs explicitly wrap by displayed width, using `unicode-width` by default. Source: docs.rs crate docs. URL: https://docs.rs/textwrap/latest/textwrap/. Confidence: HIGH.
- Current repo click-hitbox code already treats ANSI SGR sequences as zero-width when computing columns. Source: `pkgs/plugins/zellij-plugin/src/render/click.rs:163-168`. Confidence: HIGH.

### Major claim 4: first version should be grid/flex-lite, not CSS Flexbox/Grid

Evidence:

- Taffy implements CSS Block/Flexbox/Grid layout in Rust. Source: upstream README/docs.rs. URLs: https://raw.githubusercontent.com/DioxusLabs/taffy/main/README.md and https://docs.rs/taffy/latest/taffy/. Confidence: HIGH.
- Taffy computes boxes; it does not render wrapped terminal strings or preserve this repo’s click markers. Source: Taffy docs + repo rendering constraints. Confidence: HIGH.
- Ratatui itself uses constraint-based terminal layout, with warnings around complex constraint interactions. Source: Ratatui docs. URL: https://ratatui.rs/concepts/layout/. Confidence: HIGH.

Lazy v1 primitives:

- `cols` / fixed number of columns
- `gap`
- optional `width`
- newline-separated cells or explicit separator
- wrap/pad by display width
- no dependency unless existing crates already cover width/wrap

### Major claim 5: repo fit is good, with two real risks

Evidence:

- `render_template` creates a fresh MiniJinja `Environment` and calls `add_template_helpers`. Source: `pkgs/plugins/zellij-plugin/src/render/template.rs:7-22`. Confidence: HIGH.
- Helpers currently live in `filters.rs` and click helpers in `click.rs`; registering another function is a small diff. Source: `pkgs/plugins/zellij-plugin/src/render/filters.rs:31-34`, `click.rs:32-37`. Confidence: HIGH.
- `RenderModel` lacks terminal `cols`; `Renderer::render` owns `rows`/`cols` then truncates lines after template render. Source: `pkgs/plugins/zellij-plugin/src/render/mod.rs:33-52`, `model.rs:44-57`. Confidence: HIGH.
- Click buttons use private-use markers that are stripped after rendering while producing hitboxes. Source: `pkgs/plugins/zellij-plugin/src/render/click.rs:6-73`, `131-197`. Confidence: HIGH.

Risks:

1. **Width source**: true auto-flex needs actual pane `cols`; v1 can require `width=` manually or pass `cols` into `render_template` later.
2. **Marker safety**: layout must preserve button markers and ANSI escapes byte-for-byte while counting them as zero-width.

## Verification

Subagents reported:

- `moon run zellij-plugin:test` passed: 30 tests.
- `moon run zellij-plugin:check` passed WASM `wasm32-wasip1` check.
- `cm`/CodeMapper unavailable locally (`cm: command not found`), so static inspection used `read`, `grep`, file maps, and local tests.
- `lynx` unavailable in subagent environments; research used official docs via fallback fetch methods.

Confidence summary:

| Claim | Confidence | Why |
|---|---:|---|
| Call-block helper feasible | HIGH | MiniJinja docs/examples + existing repo call-block helpers agree |
| Custom `{% grid %}` unsupported by public MiniJinja API | HIGH | docs/source/issues align |
| Terminal layout must use display width | HIGH | Ratatui/unicode-width/textwrap agree |
| Avoid Taffy first pass | MEDIUM-HIGH | Taffy valid but overpowered for current renderer |
| Auto-width requires render signature change | MEDIUM-HIGH | local architecture evidence clear |

## Insights

- Best API: `{% call Grid(cols=3, gap=2, width=cols) %}...{% endcall %}`.
- Do not create custom MiniJinja syntax. It buys prettier tags and sells maintainability.
- Do not add full CSS Flexbox now. Terminal string layout needs different correctness work first: Unicode width, ANSI zero-width, marker preservation.
- Refactor visible-width logic out of `click.rs` only if Grid needs it. Otherwise keep v1 conservative.
- If template authors want data layout later, add `{{ items | grid(cols=3) }}` as convenience after call-block helper proves useful.

## Summary

Yes. Build it as a MiniJinja caller-block function registered with existing template helpers.

Minimum implementation plan:

1. Add `Grid` helper registration beside `PaneButton`/`TabButton`.
2. Implement `grid(state, kwargs)` extracting `cols`, `gap`, optional `width`, and `caller`.
3. Render caller body, split into cells, format fixed columns.
4. Treat ANSI/click markers as zero-width; do not split marker sequences.
5. Add one host unit test covering columns + one test with button markers if nested buttons are supported.

Skipped: custom `{% grid %}` tag, Taffy, full flexbox, new renderer subsystem. Add only when real templates prove fixed grid insufficient.
