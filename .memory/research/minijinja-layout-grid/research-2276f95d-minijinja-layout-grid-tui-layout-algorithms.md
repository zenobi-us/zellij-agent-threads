# Research: Terminal/TUI grid and flex-style layout algorithms for string rendering

Date: 2026-07-06
Task hash: `2276f95d`

## Thinking

Question: can MiniJinja templates expose a `{% layout %}` / grid-ish block that renders terminal-safe strings using flex/grid ideas?

Blunt answer: yes, but only if the first version is **not CSS**. Terminal rendering is integer cell layout plus Unicode/ANSI width accounting. Full CSS Grid/Flexbox inside MiniJinja is overkill unless this becomes a real UI framework.

Ponytail path:

1. Start with rows/columns/fill/min/max/gap/wrap.
2. Use terminal cell widths, not byte lengths.
3. Treat ANSI as zero-width styling, or strip before measurement.
4. Add Taffy only if users need true CSS Flexbox/Grid semantics.

State sketch:

```text
Template block
  -> parse attrs + child blocks
  -> measure children min/preferred widths
  -> solve terminal rects
  -> wrap/truncate/pad each cell
  -> stitch lines
  -> output string
```

## Skills discovered and used

- `lynx-web-search`: relevant for terminal-only web research. Tried first; `lynx` missing on machine, so used `curl`/Python fetch fallback and saved evidence in `/tmp/minijinja-layout-research/`.
- `code-library-docs`: relevant because Ratatui, Taffy, MiniJinja, width crates are libraries; used docs-first/source-first method.
- `zellij-plugin-dev`: relevant because target repo includes Zellij plugin/TUI rendering; used to frame plugin constraints: render by rows/cols, keep protocol/string output small.
- `ponytail`: active. Used to bias recommendation toward minimal layout DSL, no dependency until needed.
- `caveman`: active communication style.

## Research

### Claim 1: Terminal/TUI layout should solve rectangles in character cells, not pixels or HTML boxes.

Evidence:

1. Ratatui layout concepts docs say widgets render into `Rect` areas with height/width in buffer cells; `Layout` divides terminal dynamically by constraints such as `Length`, `Min`, `Max`, `Ratio`, `Percentage`.
   - URL: https://ratatui.rs/concepts/layout/
   - Access date: 2026-07-06
   - Source type: official docs
   - Author/publisher: Ratatui project
   - Confidence: high
   - Contradictions: none for terminal UIs.

2. Ratatui API docs define coordinate system left-to-right/top-to-bottom, origin `(0,0)`, `x/y` as terminal coordinates, and `Rect` as position + dimensions.
   - URL: https://docs.rs/ratatui/latest/ratatui/layout/index.html
   - Access date: 2026-07-06
   - Source type: generated API docs
   - Author/publisher: Ratatui crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: docs.rs page appears generated from crate docs; reliable enough.

3. Zellij plugin docs describe plugins as terminal-first workspace panes that can render UI and react to application state.
   - URL: https://zellij.dev/documentation/plugins
   - Access date: 2026-07-06
   - Source type: official docs
   - Author/publisher: Zellij project
   - Confidence: medium-high
   - Contradictions: docs explain capability, not layout algorithm details.

Implication: MiniJinja layout block should output fixed-width lines by terminal cells. `len()` is wrong.

### Claim 2: Ratatui-style constraints map well to a minimal template layout DSL.

Good primitives:

- `length(n)` / fixed columns or rows.
- `percent(n)` / parent-relative split.
- `ratio(a,b)` / proportional split.
- `min(n)` / floor.
- `max(n)` / cap.
- `fill(weight)` / remaining space by weight.
- `gap(n)` / spacer columns/rows.
- nested `row` / `col` sections.

Evidence:

1. Ratatui concepts docs list `Constraint::Length`, `Percentage`, `Ratio`, `Min`, `Max`, `Fill`; `Fill` expands into excess available space proportionally.
   - URL: https://ratatui.rs/concepts/layout/
   - Access date: 2026-07-06
   - Source type: official docs
   - Author/publisher: Ratatui project
   - Confidence: high
   - Contradictions: Ratatui warns mixed percentage/ratio/fixed constraints may surprise.

2. Ratatui API docs say `Layout` divides available screen space using constraints and can be nested; examples show header/content/footer and sidebar/main.
   - URL: https://docs.rs/ratatui/latest/ratatui/layout/index.html
   - Access date: 2026-07-06
   - Source type: API docs
   - Author/publisher: Ratatui crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: API is Rust widget-oriented, not string-template-oriented.

3. Ratatui `Layout::split` docs say it splits areas based on preferred widths/heights and direction; results are cached by layout+area.
   - URL: https://docs.rs/ratatui/latest/ratatui/layout/struct.Layout.html
   - Access date: 2026-07-06
   - Source type: API docs
   - Author/publisher: Ratatui crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: uses Cassowary/kasuari; a string renderer may not need that much machinery.

Contradiction worth keeping: Ratatui docs warn impossible constraints can yield arbitrary/non-deterministic near-solutions. A MiniJinja block should avoid exposing arbitrary constraint mixes at first; deterministic simple arithmetic is better.

### Claim 3: True CSS Flexbox/Grid is available in Rust via Taffy, but likely too heavy for first pass.

Evidence:

1. Taffy README says it implements CSS Block, Flexbox, and CSS Grid layout algorithms; it is a Rust UI layout library used by GUI/UI frameworks.
   - URL: https://raw.githubusercontent.com/DioxusLabs/taffy/main/README.md
   - Access date: 2026-07-06
   - Source type: upstream README/source repo
   - Author/publisher: DioxusLabs/Taffy maintainers
   - Confidence: high
   - Contradictions: Taffy computes layout boxes; it does not handle terminal string wrapping by itself.

2. Taffy docs.rs documents `TaffyTree`, `Style`, and layout computation model.
   - URL: https://docs.rs/taffy/latest/taffy/
   - Access date: 2026-07-06
   - Source type: API docs
   - Author/publisher: Taffy crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: current docs fetched as compact one-line HTML text; still official docs.

3. Ratatui concepts docs explicitly say Taffy PoCs exist for flexbox/grid layout rects, but Taffy is not built into Ratatui yet.
   - URL: https://ratatui.rs/concepts/layout/
   - Access date: 2026-07-06
   - Source type: official docs
   - Author/publisher: Ratatui project
   - Confidence: high
   - Contradictions: “can work nicely” but “not built in” means integration risk belongs to us.

4. Yoga docs say Yoga only determines size/position of boxes and supports a familiar subset of CSS mostly focused on Flexbox.
   - URL: https://www.yogalayout.dev/docs/about-yoga
   - Access date: 2026-07-06
   - Source type: official docs
   - Author/publisher: Meta/Yoga project
   - Confidence: medium-high
   - Contradictions: Yoga is not Rust-native in same way; useful as model, not first choice.

Recommendation: first version should not embed Taffy. Add optional Taffy backend only after simple solver fails real cases.

### Claim 4: Width measurement must use displayed terminal width, including Unicode quirks; byte length is broken.

Evidence:

1. `unicode-width` docs say it determines displayed width of `char` and `str` according to Unicode Standard Annex #11 and other Unicode rules.
   - URL: https://docs.rs/unicode-width/latest/unicode_width/
   - Access date: 2026-07-06
   - Source type: crate API docs
   - Author/publisher: unicode-width maintainers/docs.rs
   - Confidence: high
   - Contradictions: terminal emulator behavior can still differ, especially ambiguous/CJK width.

2. `textwrap` docs say wrapping needs word displayed width, not byte size; default measures displayed width with `unicode-width`.
   - URL: https://docs.rs/textwrap/latest/textwrap/
   - Access date: 2026-07-06
   - Source type: crate API docs
   - Author/publisher: textwrap maintainers/docs.rs
   - Confidence: high
   - Contradictions: default optional features matter; check Cargo features.

3. Ratatui `Line::width` docs say it returns Unicode width of content; Line implements `UnicodeWidthStr`; long lines are truncated/alignment ignored when exceeding space.
   - URL: https://docs.rs/ratatui/latest/ratatui/text/struct.Line.html
   - Access date: 2026-07-06
   - Source type: API docs
   - Author/publisher: Ratatui crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: Ratatui `Line` handles styled spans, not arbitrary raw ANSI strings.

Practical rule: every padding/truncation/wrap operation must use display width. Anything else corrupts columns with emoji/CJK/accented text.

### Claim 5: ANSI escape handling is separate from Unicode width; raw ANSI must be ignored or stripped during measurement.

Evidence:

1. `ansi-width` docs say terminal width differs from byte length because Unicode can span columns and ANSI escape codes should be ignored; crate extends `unicode-width` by ignoring ANSI escapes.
   - URL: https://docs.rs/ansi-width/latest/ansi_width/
   - Access date: 2026-07-06
   - Source type: crate API docs
   - Author/publisher: ansi-width crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: docs state limitations: tab width cannot be known; parser skips only supported escape subsets.

2. `strip-ansi-escapes` docs say it strips ANSI escape sequences from byte sequences and provides `strip`/`strip_str`.
   - URL: https://docs.rs/strip-ansi-escapes/latest/strip_ansi_escapes/
   - Access date: 2026-07-06
   - Source type: crate API docs
   - Author/publisher: strip-ansi-escapes maintainers/docs.rs
   - Confidence: high
   - Contradictions: stripping loses styling; suitable for measurement fallback/logging, not styled output preservation.

3. `textwrap` docs note `textwrap::core::display_width` supports hyperlinks since 0.16.1; `ansi-width` docs list it as alternative.
   - URL: https://docs.rs/textwrap/latest/textwrap/ and https://docs.rs/ansi-width/latest/ansi_width/
   - Access date: 2026-07-06
   - Source type: crate API docs
   - Author/publisher: textwrap / ansi-width maintainers/docs.rs
   - Confidence: medium-high
   - Contradictions: feature/version specific; verify in Cargo before relying.

Practical rule: internal representation should be styled spans, if possible. Raw ANSI strings make truncation dangerous because cutting inside escape codes corrupts output.

### Claim 6: Wrapping should happen after column widths are known, and should use terminal-aware wrap.

Evidence:

1. `textwrap` docs provide `wrap`, `fill`, `wrap_columns`; docs target CLI formatting and terminals.
   - URL: https://docs.rs/textwrap/latest/textwrap/
   - Access date: 2026-07-06
   - Source type: crate API docs
   - Author/publisher: textwrap maintainers/docs.rs
   - Confidence: high
   - Contradictions: `textwrap` works on strings; preserving styles/ANSI while wrapping may require custom fragment handling.

2. Ratatui `Paragraph` supports `.wrap(Wrap { trim: true })`; `Line` rendered directly is single-line and can truncate.
   - URL: https://docs.rs/ratatui/latest/ratatui/text/struct.Line.html
   - Access date: 2026-07-06
   - Source type: API docs
   - Author/publisher: Ratatui crate maintainers/docs.rs
   - Confidence: medium-high
   - Contradictions: `Paragraph` docs would be stronger; Line docs still show normal rendering path.

3. `unicode-width` and `textwrap` together establish that wrapping by bytes is wrong for non-ASCII terminal content.
   - URLs: https://docs.rs/unicode-width/latest/unicode_width/ , https://docs.rs/textwrap/latest/textwrap/
   - Access date: 2026-07-06
   - Source type: API docs
   - Author/publisher: crate maintainers/docs.rs
   - Confidence: high
   - Contradictions: terminal-specific ambiguous width remains.

Practical rule: layout engine should first assign cell widths, then wrap each cell to its assigned width, then row height is max child line count.

## Verification

What I verified:

- Fetched official Ratatui concepts/API docs, Taffy README/API docs, unicode/textwrap/ANSI width docs, Zellij plugin docs, MiniJinja docs.
- Checked Ratatui constraint model and warnings around Cassowary/non-determinism.
- Checked Unicode/ANSI width docs enough to reject byte length.
- Confirmed Taffy supports Flexbox/Grid but does not solve text rendering itself.

Local evidence cache:

- `/tmp/minijinja-layout-research/9fac4e0129.txt` Ratatui layout concepts.
- `/tmp/minijinja-layout-research/b45d624311.txt` Ratatui layout API module.
- `/tmp/minijinja-layout-research/755442450f.txt` Ratatui `Layout` API.
- `/tmp/minijinja-layout-research/67fb4dc274.txt` Taffy README.
- `/tmp/minijinja-layout-research/a60952e176.txt` unicode-width docs.
- `/tmp/minijinja-layout-research/67e0be4d4a.txt` textwrap docs.
- `/tmp/minijinja-layout-research/79e8249fa2.txt` ansi-width docs.
- `/tmp/minijinja-layout-research/9fa2181260.txt` strip-ansi-escapes docs.

Limitations:

- `lynx` unavailable; used Python URL fetch + rough HTML stripping.
- Did not inspect MiniJinja parser internals deeply. Need separate implementation research before claiming custom block syntax is easy.
- Did not run prototype.

## Insights

### Minimal viable layout block

Proposed DSL shape:

```jinja
{% layout width=80 gap=2 %}
  {% col width="20" %}{{ left }}{% endcol %}
  {% col fill=1 wrap=true %}{{ body }}{% endcol %}
{% endlayout %}
```

Better internal model:

```text
Node::Row { gap, children }
Node::Col { width: Fixed|Min|Max|Fill|Percent, wrap, align, text }
```

Algorithm:

1. Parse layout tree.
2. Resolve fixed/gap widths.
3. Resolve percentages against parent width.
4. Give leftover to `fill(weight)`.
5. Clamp by min/max.
6. Wrap cell text to final width.
7. Pad/truncate by displayed width.
8. Join columns line-by-line.

Avoid first pass:

- CSS `flex-basis`, `justify-content`, `align-items`, `grid-template-areas`.
- Constraint solver.
- Taffy dependency.
- Arbitrary nesting beyond row/col until tests require it.

### When to use Taffy

Use Taffy when users need:

- real CSS Flexbox/Grid behavior,
- nested mixed axes with min-content/max-content semantics,
- parity with web layout mental model,
- many layout nodes where hand-rolled edge cases pile up.

Even then, Taffy only gives boxes. You still need Unicode/ANSI-aware text wrapping/truncation.

### ANSI stance

Best: store style spans, not raw ANSI. If MiniJinja output is already raw strings, measurement can use `ansi-width`, but truncation must preserve escape integrity. Cheap first version may strip ANSI before measuring and document that raw ANSI inside layout is unsupported or best-effort.

## Summary

Top findings:

1. Feasible: yes, for terminal-safe grid/flex-lite layout.
2. First implementation should copy Ratatui primitives (`Length`, `Percent`, `Ratio`, `Min`, `Max`, `Fill`, `gap`) but use deterministic arithmetic, not Cassowary.
3. Unicode display width is mandatory; byte length breaks CJK/emoji/accented output.
4. ANSI escapes must be zero-width for measurement; raw ANSI truncation is dangerous.
5. Taffy is credible for real Flexbox/Grid, but likely too much for MiniJinja layout block v1.
6. Wrapping belongs after width resolution; row height comes from max wrapped child height.

Recommended next step: build a tiny standalone Rust prototype: `layout_row(width, specs, cells) -> String`, with tests for ASCII, CJK, emoji, ANSI color, long wrap, and fill distribution.
