# Research: embedding split MiniJinja templates in the Zellij plugin

Date: 2026-07-10

## Question and current path

`pkgs/plugins/zellij-plugin/src/render/template.rs` currently has two distinct modes:

```text
RenderModel
  -> template_dir present
       -> MiniJinja path_loader -> named runtime template -> render
  -> template_dir absent
       -> model.template string -> template_from_str -> render
```

`RenderConfig` also permits an inline `template` override, so replacing every non-directory render with a hard-coded named root would silently remove an existing customization path. Splitting only the built-in default must preserve `template_from_str(&model.template)` while registering the built-in partials that default root references.

Zellij plugins are WebAssembly/WASI modules, not ordinary host executables ([Zellij Plugins](https://zellij.dev/documentation/plugins)). Zellij does expose mapped `/host`, `/data`, and `/tmp` paths to plugin filesystem calls ([Zellij filesystem API](https://zellij.dev/documentation/plugin-api-file-system)), but built-in templates should not depend on those runtime mounts.

## Finding

Use Rust's built-in `include_str!` once per known template file, then register every non-root file in the MiniJinja `Environment` with `add_template`. Keep the existing `path_loader` branch for user-supplied runtime templates. No embedding crate and no `build.rs` are needed.

`include_str!` reads a UTF-8 file and produces a string expression; its path is relative to the Rust source file containing the macro ([Rust standard library: `include_str!`](https://doc.rust-lang.org/std/macro.include_str.html)). This gives the WASM module its built-in template bytes during Rust compilation, so rendering needs no host path, Zellij filesystem mount, permission request, or deployment of loose template files.

Minimal layout:

```text
src/render/
  model.rs
  template.rs
  templates/
    main.j2
    agent-group.j2
    events.j2
```

Minimal registration pattern, shown as a design sketch rather than a repository change:

```rust
const DEFAULT_TEMPLATE: &str = include_str!("templates/main.j2");
const AGENT_GROUP_TEMPLATE: &str = include_str!("templates/agent-group.j2");
const EVENTS_TEMPLATE: &str = include_str!("templates/events.j2");

fn add_builtin_templates(env: &mut Environment<'_>) -> Result<(), minijinja::Error> {
    env.add_template("builtin/agent-group.j2", AGENT_GROUP_TEMPLATE)?;
    env.add_template("builtin/events.j2", EVENTS_TEMPLATE)?;
    Ok(())
}
```

Then preserve current mode selection:

```rust
let captured = if let Some(template_dir) = &model.template_dir {
    env.set_loader(path_loader(template_dir));
    env.get_template(&model.template_name)?.render_captured(model)?
} else {
    add_builtin_templates(&mut env)?;
    env.template_from_str(&model.template)?.render_captured(model)?
};
```

The extracted `templates/main.j2` can use:

```jinja
{% include "builtin/agent-group.j2" %}
{% include "builtin/events.j2" %}
```

This is the smallest compatible pattern. `DEFAULT_TEMPLATE` becomes `include_str!("templates/main.j2")`, but `RenderConfig::template` remains a `String` and the renderer still compiles `model.template`. Default content gains split files; inline user overrides continue to work. Registering built-ins only in the non-`template_dir` branch also leaves runtime-loader behavior unchanged and avoids built-in names taking precedence over similarly named files in a custom directory.

## MiniJinja API choice

`Environment::add_template` borrows both template name and source for the environment lifetime and rejects syntax errors when the template is added. `Environment::add_template_owned` accepts `Cow`-convertible owned or borrowed values and exists for sources whose `String` ownership must move into the environment ([MiniJinja `Environment` API](https://docs.rs/minijinja/latest/minijinja/struct.Environment.html), [MiniJinja `environment.rs` source](https://docs.rs/minijinja/latest/src/minijinja/environment.rs.html#174-204)).

For `include_str!`, `add_template` is the direct fit: included strings are static compile-time data, so there is no ownership problem and no allocation is required. `add_template_owned` is useful only if a later implementation generates names or template text as runtime `String`s. It is not required for embedding.

The root may also be registered and rendered by name:

```rust
env.add_template("builtin/main.j2", DEFAULT_TEMPLATE)?;
env.get_template("builtin/main.j2")?.render_captured(model)?;
```

That pattern gives the root a stable MiniJinja name and is suitable if the built-in root is always selected. It is not the minimal compatible change here because current `model.template` can contain an inline user override. Keeping `template_from_str` for the root and registering only dependencies retains that API.

`path_loader` is a dynamic filesystem loader rooted at the supplied directory; hidden path components are rejected ([MiniJinja `path_loader`](https://docs.rs/minijinja/latest/minijinja/fn.path_loader.html)). It remains appropriate for explicit `template_dir` customization, but it is the wrong mechanism for compiled defaults because it requires runtime files.

## Template composition semantics

Multi-template tags are under MiniJinja's `multi_template` feature, included by default ([MiniJinja syntax reference](https://docs.rs/minijinja/latest/minijinja/syntax/index.html)). The repository's plain `minijinja = "2.21"` dependency therefore needs no Cargo feature change.

- `{% include "name" %}` renders another registered or loader-resolved template at that point. Included templates see the active context. `ignore missing` suppresses a missing-template error ([MiniJinja syntax reference: include](https://docs.rs/minijinja/latest/minijinja/syntax/index.html#including-other-templates)). This is the best split mechanism for the current monolithic body.
- `{% extends "name" %}` selects a parent skeleton; child `{% block name %}` sections override parent blocks, and `super()` renders parent block content. MiniJinja says `extends` should be the first tag ([MiniJinja syntax reference: inheritance](https://docs.rs/minijinja/latest/minijinja/syntax/index.html#template-inheritance)). Use this only if the default UI genuinely has a stable shell plus variants; includes are less machinery for simple extraction.
- `{% import "name" as ns %}` and `{% from "name" import macro %}` expose exported variables and macros. Imported template body output is discarded; unlike Jinja2, MiniJinja imports are not cached and receive full template context ([MiniJinja syntax reference: import](https://docs.rs/minijinja/latest/minijinja/syntax/index.html#import), [MiniJinja compatibility notes](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md#import)). Use imports for reusable macros, not for ordinary rendered sections.

Template references such as `"builtin/events.j2"` are MiniJinja environment names. They must exactly match names passed to `add_template` or names resolvable by the active loader. They are separate from the Rust compile-time path in `include_str!("templates/events.j2")`.

## `include_str!` versus `include_bytes!`

`include_str!` is correct for MiniJinja source because MiniJinja accepts strings and templates must be UTF-8. Invalid UTF-8 fails at compilation by the macro's UTF-8 contract ([Rust standard library: `include_str!`](https://doc.rust-lang.org/std/macro.include_str.html)).

`include_bytes!` returns `&'static [u8; N]`; its path is also relative to the invoking source file and interpreted using compile-host path rules ([Rust standard library: `include_bytes!`](https://doc.rust-lang.org/std/macro.include_bytes.html)). Using it would add a byte-to-UTF-8 conversion before MiniJinja can consume the source and provides no benefit for text templates. Avoid compile-host-specific backslash paths; use repository-relative forward-slash paths as shown.

Neither macro accepts a directory glob. Explicit constants are the minimal option for a small known set. A generated registry, embedding crate, or `build.rs` becomes justified only if automatic discovery of an unbounded template directory is a real requirement. Cargo build scripts are separate programs run before package compilation and require explicit change-tracking directives such as `cargo::rerun-if-changed` when used ([Cargo Book: build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)). None of that is needed for explicit `include_str!` calls.

If this crate is later packaged for crates.io, template files must remain in the package source set; Cargo's `include`/`exclude` manifest fields control packaged files ([Cargo Book: manifest include/exclude](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields), [Cargo Book: `cargo package`](https://doc.rust-lang.org/cargo/commands/cargo-package.html)). Keeping templates under `src/render/templates/` avoids surprising placement and should be verified with `cargo package --list` if publishing is introduced.

## WASM and repository build constraints

Embedding removes runtime filesystem coupling but increases the compiled WASM by approximately the stored template text plus compiler/format overhead. It does not precompile MiniJinja bytecode: `add_template` and `template_from_str` still parse template source when each fresh `Environment` is constructed; `render_template` currently creates an environment per render. Changing environment lifetime or caching is a separate performance decision and is not required to split files. `add_template` can return syntax errors during registration, as documented by MiniJinja's API/source ([MiniJinja `environment.rs`](https://docs.rs/minijinja/latest/src/minijinja/environment.rs.html#174-204)).

Repository-specific build metadata needs one follow-up when application code is eventually changed: `pkgs/plugins/zellij-plugin/moon.yml` declares only `src/**/*.rs` as build/check/test inputs, and `dev-watch` watches extensions `rs,toml`. New `.j2` files would therefore be invisible to Moon cache invalidation and the current watcher even though Rust compilation consumes them. Expand Moon inputs to `src/**/*` (or add `src/**/*.j2`) and add `j2` to watcher extensions in the same implementation change. This is a Moon orchestration issue, not a need for Cargo `build.rs`.

## Recommendation

1. Put `main.j2` and extracted sections under `src/render/templates/`.
2. Change `DEFAULT_TEMPLATE` from a raw string to `include_str!("templates/main.j2")`.
3. Add one explicit `include_str!` constant and one `env.add_template(...)` call per partial.
4. Register built-in partials only in the no-`template_dir` branch; continue rendering `model.template` with `template_from_str`.
5. Use `{% include %}` for rendered sections; use `{% import %}` only for macro libraries and `{% extends %}` only after a real parent/child variant appears.
6. Update Moon inputs/watch extensions when implementing.
7. Add no crate, Cargo feature, or `build.rs`.

## Conclusion

Minimal viable equivalent to Go's explicit embedding is Rust `include_str!` plus MiniJinja `Environment::add_template`. It works for `wasm32-wasip1` because template content is part of the compiled module and requires no runtime host filesystem. Preserve the existing dynamic `path_loader` path for user templates and the existing `template_from_str(&model.template)` path for inline overrides. No application code was changed by this research task.
