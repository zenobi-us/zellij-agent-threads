# Research: MiniJinja grid/flex layout fit in zellij-agent-threads

## Thinking

Question: can this repo support a MiniJinja layout block for grid/flex-style terminal layout with small change?

Bias: [bias: prefer one MiniJinja function/caller block over new renderer subsystem]. Existing renderer already treats template output as final text. Replacing painter with layout engine is overkill unless dynamic coordinates become required.

Skills discovered/used:

- `zellij-plugin-dev`: used because target is Rust/WASM Zellij plugin rendering. Relevant guidance: render phase draws UI, host tests matter for Zellij side effects.
- `codemapper`: discovered for AST tracing, attempted `cm stats` / `cm map`; unavailable in environment (`cm: command not found`). Fell back to `read`, `grep`, file maps, and tests.
- `rust-engineer`: used for Rust dependency/config review and host/WASM verification expectations.
- `ponytail`: used to force smallest viable change, avoid new layout subsystem.

State machine of render path:

```text
Zellij render(rows, cols)
  -> RenderModel::from_runtime(runtime, config)
  -> Renderer::render(model, rows, cols)
  -> render_template(model)
  -> MiniJinja Environment + helpers
  -> render_captured(model)
  -> collect_hitboxes(state, output)
  -> Renderer prints each line at x=0, clipped to cols
```

## Research

### Current rendering architecture

Render is deliberately split into model/template/painter layers. `render/mod.rs` says `RenderModel` prepares testable plain data while `Renderer` performs Zellij terminal calls (`pkgs/plugins/zellij-plugin/src/render/mod.rs:1-6`). `Renderer::render` clears rows, renders template, then prints each rendered line at column zero (`pkgs/plugins/zellij-plugin/src/render/mod.rs:33-52`).

Template rendering is isolated in `render_template`: it creates a fresh `minijinja::Environment`, registers helpers, then either loads disk template via `path_loader` or inline template string, using `render_captured(model)` (`pkgs/plugins/zellij-plugin/src/render/template.rs:7-22`). Captured state is required because click helpers store button actions in MiniJinja temp state before `collect_hitboxes` strips markers (`pkgs/plugins/zellij-plugin/src/render/template.rs:22`, `pkgs/plugins/zellij-plugin/src/render/click.rs:47-59`).

Helpers already exist in two forms:

- Filters: `remap`, `fg`, `bg`, `dim`, `bold`, `italic` are registered with `env.add_filter` (`pkgs/plugins/zellij-plugin/src/render/filters.rs:4-11`).
- Caller-block functions: `PaneButton` and `TabButton` are registered with `env.add_function` and used via `{% call ... %}` (`pkgs/plugins/zellij-plugin/src/render/click.rs:32-37`, `pkgs/plugins/zellij-plugin/src/render/model.rs:18-28`).

MiniJinja version is pinned by Cargo deps: `minijinja = "2.21"` (`pkgs/plugins/zellij-plugin/Cargo.toml:8-12`) and lock resolves `2.21.0` (`pkgs/plugins/zellij-plugin/Cargo.lock:1158-1165`). External docs confirm `Environment` supports both `add_filter` and `add_function` in MiniJinja 2.21.0: <https://docs.rs/minijinja/2.21.0/minijinja/struct.Environment.html>. MiniJinja function docs also document `Kwargs` handling for extra args: <https://docs.rs/minijinja/2.21.0/minijinja/functions/index.html>.

### Template model and docs fit

Default template is already handwritten MiniJinja with macros, loops, filters, and call blocks (`pkgs/plugins/zellij-plugin/src/render/model.rs:8-36`). Disk templates are supported for include/import through `template_dir` and `template_name`, documented in plugin README (`pkgs/plugins/zellij-plugin/README.md:32-43`) and tested with include/import (`pkgs/plugins/zellij-plugin/src/render/mod.rs:164-193`).

`RenderModel` contains runtime data and template config, but not terminal `rows`/`cols` (`pkgs/plugins/zellij-plugin/src/render/model.rs:44-57`). `Renderer::render` has `rows`/`cols` and applies truncation after template render (`pkgs/plugins/zellij-plugin/src/render/mod.rs:33-52`, `pkgs/plugins/zellij-plugin/src/render/mod.rs:80-88`). This matters: a flex helper cannot know actual pane width unless width is passed manually or `render_template` receives `cols` and exposes it.

### Click/hitbox interaction

Buttons work by injecting private-use markers into template output, then stripping them while tracking visible columns (`pkgs/plugins/zellij-plugin/src/render/click.rs:6-8`, `pkgs/plugins/zellij-plugin/src/render/click.rs:131-190`). ANSI escapes are skipped for column counting (`pkgs/plugins/zellij-plugin/src/render/click.rs:163-168`) and tested (`pkgs/plugins/zellij-plugin/src/render/click.rs:224-236`). Multi-line button hitboxes are supported (`pkgs/plugins/zellij-plugin/src/render/click.rs:239-269`).

A grid/flex helper that reflows text after `PaneButton`/`TabButton` markers can corrupt hitboxes if it counts marker bytes as visible columns or splits marker sequences. Safest constraint: layout helper should format child text after nested caller is rendered, preserving marker substrings and treating markers/ANSI as zero-width. Existing marker parsing logic is private in `click.rs`; duplicating it in a helper is bad unless refactored into shared visible-width utilities.

### Smallest code change decision

Best fit: MiniJinja caller-block function, not filter.

Why:

- Block layout needs child body: existing pattern is function with `caller` kwarg (`caller_label` gets `caller` and calls it via `state.format(caller.call(...))`, `pkgs/plugins/zellij-plugin/src/render/click.rs:126-129`).
- Existing template syntax already uses `{% call PaneButton(...) %}...{% endcall %}` (`pkgs/plugins/zellij-plugin/src/render/model.rs:21-28`). A layout block would feel native: `{% call Grid(cols=2, gap=2) %}...{% endcall %}`.
- A filter can only transform a value after template authors manually build a string. Worse UX, no need.
- Custom MiniJinja tag/block extension is likely unnecessary. MiniJinja caller functions already cover it.

Smallest viable implementation:

1. Add `env.add_function("Grid", grid)` in `add_template_helpers` or `add_filters` area (`pkgs/plugins/zellij-plugin/src/render/filters.rs:31-34`).
2. Implement `grid(state: &State, kwargs: Kwargs) -> Result<String, Error>` in `filters.rs` first. If it grows beyond small helper, split later into `render/layout.rs`.
3. Read `cols`, `gap`, optional `width` from kwargs. Use caller body as raw string. Split rows by newline. Pack into fixed column widths.
4. Add tests beside render/helper tests, like current filter/button tests (`pkgs/plugins/zellij-plugin/src/render/filters.rs:108-175`, `pkgs/plugins/zellij-plugin/src/render/mod.rs:98-161`).

If true flex needs pane width, one extra seam change is needed:

- Change `render_template(model)` to `render_template(model, cols)` and add a MiniJinja global/function arg for `cols` from `Renderer::render` (`pkgs/plugins/zellij-plugin/src/render/mod.rs:33-41`, `pkgs/plugins/zellij-plugin/src/render/template.rs:7-12`).
- Caveat: first line has less usable width because collapse button reserves `button.len() + 1` only after render (`pkgs/plugins/zellij-plugin/src/render/mod.rs:40-49`). A helper using full `cols` can still collide/truncate on row 0.

## Verification

Commands run:

```text
moon run zellij-plugin:test
moon run zellij-plugin:check
```

Observed:

- Host tests: 30 passed.
- WASM check: passed for `wasm32-wasip1`.
- `cm` unavailable, so CodeMapper-based call graph verification could not run.

Static evidence checked:

- Rendering entry and line clipping: `pkgs/plugins/zellij-plugin/src/render/mod.rs:33-60`, `pkgs/plugins/zellij-plugin/src/render/mod.rs:80-88`.
- Template environment/helper registration: `pkgs/plugins/zellij-plugin/src/render/template.rs:10-22`, `pkgs/plugins/zellij-plugin/src/render/filters.rs:31-34`.
- Existing block helper implementation: `pkgs/plugins/zellij-plugin/src/render/click.rs:32-37`, `pkgs/plugins/zellij-plugin/src/render/click.rs:76-88`, `pkgs/plugins/zellij-plugin/src/render/click.rs:126-129`.
- Template docs/tests: `pkgs/plugins/zellij-plugin/README.md:32-43`, `pkgs/plugins/zellij-plugin/src/render/mod.rs:164-193`.

## Insights

Confidence: high that a MiniJinja function/caller block is the right integration point. Evidence: existing `PaneButton`/`TabButton` are exactly that pattern.

Confidence: medium that smallest first patch can live in `filters.rs`. It is currently the general helper registry despite name mismatch (`add_template_helpers` lives there). Contradiction: a layout helper is not a filter. If helper grows, `render/layout.rs` is cleaner.

Confidence: medium-low for full flex behavior without touching render signature. Current `RenderModel` lacks pane width, while `Renderer::render` owns `cols`. Manual width arg works; automatic flex needs passing `cols` into MiniJinja.

Contradictions / risks:

- Existing painter prints only full lines at x=0. No cell-level coordinate rendering exists (`pkgs/plugins/zellij-plugin/src/render/mod.rs:44-52`). A helper must output preformatted text, not a structured layout tree.
- Existing truncation is by Unicode scalar count, not terminal display width (`pkgs/plugins/zellij-plugin/src/render/mod.rs:85-88`). Wide glyphs/icons can misalign grid columns. Default template already uses icons (`pkgs/plugins/zellij-plugin/src/render/model.rs:22-27`), so this is current debt, not new.
- Button marker/hitbox handling is sensitive. A layout helper that pads/truncates strings containing private markers must treat those as zero-width and indivisible.
- First rendered row has collapse button space removed after template render (`pkgs/plugins/zellij-plugin/src/render/mod.rs:40-49`). Auto-width layout can be correct for rows 1..N and still collide on row 0.

## Summary

Top finding: yes, this architecture fits a MiniJinja grid/flex layout helper, but only as a preformatted-text helper. Do not build a renderer subsystem.

Recommended smallest path:

```jinja
{% call Grid(cols=2, gap=2, width=cols) %}
{{ left }}
---
{{ right }}
{% endcall %}
```

Implementation shape: add one MiniJinja caller function registered beside existing helpers. Start fixed/manual-width. Add automatic `cols` only if needed by changing `render_template(model, cols)`.

Do not implement a custom MiniJinja tag, layout AST, or coordinate painter now. Too much machinery for current renderer.
