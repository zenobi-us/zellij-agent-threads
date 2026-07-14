# Research: MiniJinja grid/flex layout template API design

Access date: 2026-07-06

## Thinking

Goal: choose a template-author API for grid/flex layout in `zellij-agent-threads`, using MiniJinja without wrecking compatibility, errors, escaping, or click markers.

Bias: prefer boring MiniJinja-native APIs. Custom syntax is likely overkill.

State machine:

```text
template author need
  -> arbitrary body layout
      -> MiniJinja call block -> Rust function reads caller -> render layout -> collect click hitboxes
  -> data-only layout
      -> function/filter over array -> render cells from data
  -> reusable author snippets
      -> macro/include -> no Rust parser change
  -> new language syntax
      -> parser/ext risk -> avoid unless all above fail
```

## Skills discovered and used

- `lynx-web-search`: relevant to terminal-only web research. Used methodology, but `lynx` binary missing; fallback used `curl` to fetch official docs into `/tmp`.
- `code-library-docs`: relevant to MiniJinja/Jinja library understanding. Used docs-first/cache-first principle: official docs before inference.
- No dedicated "template-language-design" skill found in available skill list. Used authoritative docs directly.

## Research

### Sources consulted

| Source | URL | Type | Publisher/author | Relevant evidence | Confidence | Contradictions |
|---|---|---:|---|---|---:|---|
| MiniJinja syntax docs | https://docs.rs/minijinja/latest/minijinja/syntax/index.html | official API docs | MiniJinja / docs.rs | macros, `{% call %}`, filters, autoescape blocks | high | MiniJinja says macro support is "limited" |
| MiniJinja Environment | https://docs.rs/minijinja/latest/minijinja/struct.Environment.html | official API docs | MiniJinja / docs.rs | `add_function`, `set_syntax`, syntax errors on template load | high | none |
| MiniJinja SyntaxConfig | https://docs.rs/minijinja/latest/minijinja/syntax/struct.SyntaxConfig.html | official API docs | MiniJinja / docs.rs | "custom syntax" means delimiters/prefixes; not custom tags | high | name "custom_syntax" misleading |
| MiniJinja Value | https://docs.rs/minijinja/latest/minijinja/value/struct.Value.html | official API docs | MiniJinja / docs.rs | `from_safe_string` bypasses autoescaping | high | dangerous if misused |
| MiniJinja Error / ErrorKind | https://docs.rs/minijinja/latest/minijinja/struct.Error.html and https://docs.rs/minijinja/latest/minijinja/enum.ErrorKind.html | official API docs | MiniJinja / docs.rs | errors expose kind/name/line/range/debug info; `SyntaxError`, `TooManyArguments` | high | range not guaranteed in all cases |
| Jinja template docs | https://jinja.palletsprojects.com/en/stable/templates/ | official docs | Pallets | call blocks, filters, macros, autoescaping | high | richer Python ecosystem than MiniJinja |
| Jinja extensions docs | https://jinja.palletsprojects.com/en/stable/extensions/ | official docs | Pallets | custom parser extensions exist in Jinja | high | MiniJinja docs do not expose same extension API |
| Nunjucks templating docs | https://mozilla.github.io/nunjucks/templating.html | official docs | Mozilla/Nunjucks | Jinja-like call/filter/macro patterns | medium-high | JS async caveats irrelevant to MiniJinja |
| Handlebars block helpers | https://handlebarsjs.com/guide/block-helpers.html | official docs | Handlebars | block helper shape, `options.fn`, hash args | medium | different language, useful only as API analogy |
| Project click markers | `pkgs/plugins/zellij-plugin/src/render/click.rs` | repo source | zellij-agent-threads | PUA markers wrap clickable labels; post-render hitbox collection strips markers | high | project-specific, not external |
| Project template setup | `pkgs/plugins/zellij-plugin/src/render/template.rs`, `filters.rs` | repo source | zellij-agent-threads | helpers registered via `Environment::add_function`/`add_filter`; `render_captured` then `collect_hitboxes` | high | none |

### Major claim 1: `{% call Grid(...) %}...{% endcall %}` is best default for arbitrary author-written layout bodies

Evidence:

- MiniJinja call blocks pass an anonymous macro as hidden `caller`; docs show `caller()` rendering body and `call(user)` for callback args. Source: MiniJinja syntax docs. Confidence: high. Contradictions: macro support described as limited.
- Jinja official docs describe same call-block model and say it passes macro content, including argument callbacks. Source: Pallets Jinja docs. Confidence: high. Contradictions: Jinja has broader extension ecosystem.
- Nunjucks official docs clone Jinja call-block ergonomics: content inside tag available as `caller()`. Source: Mozilla/Nunjucks docs. Confidence: medium-high. Contradictions: async macro caveats not applicable.
- Project already implements call-compatible Rust functions (`PaneButton`, `TabButton`) by reading `caller` from `Kwargs` and formatting it via `State`. Source: `click.rs:76-129`. Confidence: high. Contradictions: none.

Assessment:

```jinja
{% call Grid(columns=3, gap=1) %}
  {{ PaneButton(pane="1") }}
  {{ TabButton(tab=2) }}
{% endcall %}
```

Better actual style if `PaneButton` requires call body:

```jinja
{% call Grid(columns=3, gap=1) %}
  {% call PaneButton(pane="1") %}Logs{% endcall %}
  {% call TabButton(tab=2) %}Tests{% endcall %}
{% endcall %}
```

Pros:
- Vanilla MiniJinja syntax. No parser fork.
- Natural wrapper for arbitrary body content.
- Existing implementation pattern already in repo.
- Good docs transfer from Jinja/Nunjucks.

Risks:
- Single body only. Named slots/rows need conventions (`Cell`, line splitting, separators), not language syntax.
- Body must be formatted before layout; layout code must ignore ANSI/click markers for width.

### Major claim 2: `{{ grid([...]) }}` and filters over arrays fit data-driven layout, not arbitrary rich blocks

Evidence:

- MiniJinja exposes `Environment::add_function` and functions are callable with positional/keyword args. Source: MiniJinja Environment + syntax docs. Confidence: high.
- Jinja and Nunjucks filters are pipe-applied functions with optional args; good for transforming existing values. Sources: Jinja templates, Nunjucks templating. Confidence: high.
- Current repo uses filters for scalar transforms (`fg`, `bg`, `bold`, `remap`) and functions for interactive markup (`PaneButton`, `TabButton`). Source: `filters.rs`, `click.rs`. Confidence: high.

API shapes:

```jinja
{{ grid(items, columns=3, gap=1) }}
{{ items | grid(columns=3, gap=1) }}
```

Pros:
- Short.
- Easy to validate args/types in Rust.
- Good for lists of plain strings or pre-rendered strings.

Risks:
- Bad ergonomics for mixed rich content; authors must build arrays in template.
- Filters hide structure. `items|grid` reads like text transform, not layout container.
- Escaping/safety unclear when input values include markup/click markers.

Verdict: keep as optional convenience later. Not primary API.

### Major claim 3: macros/includes are good documentation escape hatches, not enough if layout needs Rust width/click handling

Evidence:

- MiniJinja macros/imports support reusable template functions, but docs call support "limited" and note import behavior differs from Jinja2. Source: MiniJinja syntax docs. Confidence: high.
- Jinja macros/imports are established for reusable template idioms. Source: Jinja templates. Confidence: high.
- Nunjucks macros/imports mirror Jinja but have context/import caveats and async restrictions. Source: Nunjucks docs. Confidence: medium-high.

API shape:

```jinja
{% import "layout.jinja" as layout %}
{{ layout.grid(items, columns=3) }}
```

Pros:
- No Rust change if pure text layout is enough.
- Authors can override local style.

Risks:
- Terminal width, ANSI, Unicode width, and click marker handling probably need Rust helpers anyway.
- Macro-only layout duplicates logic and weakens error messages.

Verdict: useful docs pattern for simple userland snippets, not core layout engine.

### Major claim 4: custom MiniJinja syntax is compatibility debt and likely unsupported for new tags through public API

Evidence:

- MiniJinja `SyntaxConfig` docs say custom syntax config changes delimiters and line prefixes; start markers must be distinct. It does not document custom tags/AST parser hooks. Source: MiniJinja SyntaxConfig. Confidence: high.
- MiniJinja `Environment::set_syntax` applies syntax config only to future templates. Source: MiniJinja Environment. Confidence: high.
- Jinja official docs have an extensions system for parser-level changes, showing custom syntax is a real concept in Jinja proper. Source: Jinja extensions docs. Confidence: high.
- Nunjucks has extension mechanisms, but this only proves other engines can support parser extensions; MiniJinja docs consulted do not expose equivalent new-tag API. Source: Nunjucks docs. Confidence: medium. Contradiction: MiniJinja internals may have non-public machinery; using it would be brittle.

Bad API shape:

```jinja
{% grid columns=3 gap=1 %}
  ...
{% endgrid %}
```

Risks:
- Requires parser/compiler work or preprocessing.
- Breaks syntax tooling expectations.
- New errors become ours, not MiniJinja's.
- Docs burden high: custom tag syntax, nesting rules, migration rules, escaping rules.

Verdict: do not build custom syntax unless call-block API is proven impossible.

### Major claim 5: escaping and click marker preservation are core correctness risks

Evidence:

- MiniJinja `Value::from_safe_string` bypasses autoescaping; docs explicitly say use it when engine should render HTML without requiring `|safe`. Source: MiniJinja Value docs. Confidence: high.
- MiniJinja `AutoEscape::Html` escapes `<`, `>`, `&`, quotes, slash; custom formats need custom formatter. Source: MiniJinja AutoEscape docs. Confidence: high.
- Project click actions are encoded as private-use markers around labels, then `collect_hitboxes` strips markers post-render while tracking visible columns. Source: `click.rs:6-73`, `131-197`. Confidence: high.
- Current formatter counts ANSI escape sequences as zero-width during click marker stripping. Source: `click.rs:163-168`. Confidence: high. Contradictions: only handles SGR ending in `m`; full ANSI parser absent.

Implications for Grid/Flex:

- If Grid lays out content after buttons are rendered, it must measure visible width while ignoring:
  - click marker spans (`\u{E000}B...\u{E001}`, `\u{E000}E...\u{E001}`),
  - ANSI SGR sequences,
  - probably Unicode display width.
- It must preserve markers byte-for-byte in output. If it trims, escapes, wraps, or reorders marker pairs incorrectly, hitboxes break.
- If autoescape is ever enabled for templates, layout functions returning generated terminal markup should return safe strings intentionally, or the docs must say autoescape is unsupported for terminal templates.

### Major claim 6: best error reporting comes from staying inside MiniJinja functions/filters, not preprocessing/custom syntax

Evidence:

- MiniJinja `Error` supports kind, name, line, range, template source, debug display; range requires debug feature and is not guaranteed. Source: MiniJinja Error docs. Confidence: high.
- `ErrorKind` includes `InvalidOperation`, `SyntaxError`, `TooManyArguments`, etc. Source: MiniJinja ErrorKind docs. Confidence: high.
- Current helpers use `Kwargs::get`, `assert_all_used`, and `Error::new(ErrorKind::InvalidOperation, ...)` for template-facing validation. Source: `click.rs:76-123`, `filters.rs:36-106`. Confidence: high.

Implications:

- `Grid` as Rust function can report bad args like `Grid expects columns >= 1` at render time with MiniJinja error context.
- Custom syntax/preprocessor would need its own span mapping and diagnostics. Bad trade.

## API comparison

| API | Example | Compatibility | Errors | Escaping/markers | Docs burden | Verdict |
|---|---|---|---|---|---|---|
| Call block function | `{% call Grid(columns=3) %}...{% endcall %}` | high: vanilla MiniJinja/Jinja style | good: `Kwargs`, `ErrorKind` | good if function preserves raw body/zero-width markers | medium | **primary** |
| Function over array | `{{ grid(items, columns=3) }}` | high | good | okay for pre-rendered strings, risky for rich blocks | low | optional later |
| Filter over array | `{{ items|grid(columns=3) }}` | high | good but less discoverable | same as function | low | optional, not primary |
| Macro/include | `{% import "layout" as l %}{{ l.grid(...) }}` | high | template-level | weak for marker-aware widths | medium | docs escape hatch |
| Custom syntax | `{% grid columns=3 %}...{% endgrid %}` | low in MiniJinja | expensive | risky | high | reject now |

## Recommended API

Minimum useful surface:

```jinja
{% call Grid(columns=3, gap=1) %}
  {% call PaneButton(pane="1") %}Logs{% endcall %}
  {% call PaneButton(pane="2") %}Shell{% endcall %}
  {% call TabButton(tab=3) %}Tests{% endcall %}
{% endcall %}
```

If flex needed:

```jinja
{% call Flex(direction="row", gap=1, wrap=true) %}
  ...
{% endcall %}
```

Keep names capitalized if matching existing `PaneButton`/`TabButton`; otherwise lowercase all helpers in one cleanup later. Do not mix new naming styles casually.

Validation rules:

- `columns` MUST be integer `>= 1`.
- `gap` SHOULD default to `1` and MUST be non-negative.
- Unknown kwargs MUST error via `assert_all_used()`.
- Layout MUST preserve click marker bytes.
- Layout width measurement MUST ignore click markers and ANSI SGR sequences.
- Custom syntax MUST NOT be added for first version.

## Verification

Checked project reality:

- `render_template` creates `Environment::new()`, registers helpers, renders captured output, then calls `collect_hitboxes`. Path: `pkgs/plugins/zellij-plugin/src/render/template.rs`.
- Existing helper registration already uses `add_function`/`add_filter`. Path: `pkgs/plugins/zellij-plugin/src/render/filters.rs`, `click.rs`.
- Existing click API already proves call-block-to-Rust-function pattern works: `caller` kwarg is extracted and rendered via `State::format(caller.call(...))`.
- Click marker stripping happens after full template render; any layout helper inserted before that step must preserve markers.

## Insights

1. Lazy answer: implement `Grid`/`Flex` as call-block Rust functions. No syntax extension. No macro DSL.
2. Biggest hidden bug risk is width measurement, not MiniJinja syntax. Current click stripper already has zero-width marker logic; layout needs same model or shared helper.
3. Function/filter array APIs are useful only after real data-driven templates appear. Building them first is speculative.
4. Custom MiniJinja syntax is bad direction: more code, worse diagnostics, more docs, unclear public support.
5. Docs should teach one pattern: call block container + existing button call blocks. Extra APIs later only if templates become noisy.

## Summary

Recommended first API: `{% call Grid(columns=..., gap=...) %}...{% endcall %}` implemented as a normal MiniJinja Rust function accepting `State` + `Kwargs` + hidden `caller`.

Do not implement custom `{% grid %}` syntax. MiniJinja public docs only support delimiter-level syntax config, not new parser tags. Use boring Jinja-compatible call blocks.

Most important implementation requirement: preserve click markers and measure visible width ignoring markers/ANSI. Syntax is easy. Width correctness is where bugs live.
