# RetroX

**Permanent infrastructure for websites that should not rot.**

RetroX is an experimental alternative web project focused on permanence, ownership, privacy, and stable publishing. It is not nostalgia for old technology. It is an attempt to build web infrastructure where a site created today can still render correctly decades later without being rewritten every time the modern web stack mutates.

This repository currently contains the first working piece of that idea: a Rust-based **RNMDL GN-Z11 parser, validator, and terminal renderer**.

## Status

> Early experimental prototype. Not production-ready.

Current implementation:

- RNMDL GN-Z11 document parsing
- strict validation
- terminal rendering
- zero external Rust dependencies
- basic CLI commands
- module declaration system
- image placeholder rendering
- source-context error messages

Planned larger ecosystem:

- ORNPS: Open RetroNet Protocol Stack
- ARNPS: Authenticated RetroNet Protocol Stack
- RetroBrowser
- RetroID / RetroPasskey
- RetroSE
- site packaging and certificate verification

## Philosophy

The modern web is built on moving targets:

- HTML, CSS, and JavaScript evolve constantly
- frameworks are replaced every few years
- websites depend on fragile dependency chains
- platforms pivot, shut down, or break old content
- maintaining a site for decades requires constant labor

RetroX takes the opposite approach:

1. **Stable specifications** — features are frozen into named module sets.
2. **Static-first design** — documents before applications.
3. **No tracking by design** — privacy is infrastructure, not a plugin.
4. **Creator ownership** — sites should belong to their creators.
5. **Permanence over novelty** — change is deliberate, versioned, and conservative.

RetroX is not trying to replace the modern internet. It is alternative infrastructure for permanent digital spaces.

## What is RNMDL?

**RNMDL** stands for **RetroNet Markdown Language**.

It is the content format used by RetroX. Instead of supporting the full complexity of modern HTML/CSS/JavaScript, RNMDL is intentionally small, strict, and versioned.

The current implemented version is:

```text
RNMDL GN-Z11 (v0.0.0)
```

GN-Z11 currently supports:

- text
- headings
- paragraphs
- containers
- sections
- image declarations
- comments
- module declarations
- bugfixset opt-in declaration

## Example RNMDL document

```rnmdl
<RNMDL>
from gn-z11 declare text, images
implement_bugfixsets = True

[container id="home"]
[h1]Welcome to RetroX[/h1]
[p]This is a permanent document rendered by the GN-Z11 terminal renderer.[/p]

[section id="about"]
[h2]About[/h2]
[p]RNMDL is designed to be small, stable, and easy to preserve.[/p]
[image path="assets/logo.png" alt="RetroX project logo"]
[/section]
[/container]
```

## Current RNMDL tags

| Tag | Purpose |
|---|---|
| `[container]...[/container]` | Groups content into a larger block |
| `[section]...[/section]` | Defines a document section |
| `[h1]...[/h1]` | Level 1 heading |
| `[h2]...[/h2]` | Level 2 heading |
| `[h3]...[/h3]` | Level 3 heading |
| `[heading level="1"]...[/heading]` | Explicit heading tag |
| `[paragraph]...[/paragraph]` | Paragraph text |
| `[p]...[/p]` | Short paragraph tag |
| `[image path="..." alt="..."]` | Self-closing image declaration |
| `<!-- comment -->` | Comment |

## Validation rules

The GN-Z11 validator is intentionally strict. Invalid documents are rejected instead of silently degraded.

Current validation includes:

- document must declare at least one module
- unknown module versions are rejected
- modules cannot be declared more than once
- heading levels must be 1, 2, or 3
- headings and paragraphs cannot be empty
- image paths are required
- image alt text is required and must be meaningful
- image formats are limited to `jpg`, `jpeg`, `png`, `gif`, and `webp`
- IDs must be unique within the document
- IDs must start with a letter
- IDs may only contain letters, numbers, and hyphens

This strictness is intentional. RetroX favors predictable documents over permissive chaos.

## Installation

### Requirements

- Rust `1.95.0`
- Cargo

Check your Rust version:

```bash
rustc --version
cargo --version
```

### Clone and build

```bash
git clone https://github.com/Disc0wd/retrox.git
cd retrox
cargo build
```

### Run

```bash
cargo run -- render path/to/file.rnmdl
```

Or build a release binary:

```bash
cargo build --release
./target/release/retrox render path/to/file.rnmdl
```

## CLI usage

```text
retrox <command> [file]
```

Commands:

```text
render   <file.rnmdl>   Parse, validate, and render a document
validate <file.rnmdl>   Validate without rendering
check    <file.rnmdl>   Render without meta information
version                 Show version information
help                    Show help message
```

Examples:

```bash
cargo run -- render site.rnmdl
cargo run -- validate site.rnmdl
cargo run -- check site.rnmdl
cargo run -- version
```

## Project structure

```text
src/
├── main.rs       # CLI entry point
├── lexer.rs      # RNMDL lexer/tokenizer
├── parser.rs     # token stream → AST parser
├── ast.rs        # abstract syntax tree definitions
├── validator.rs  # strict GN-Z11 validation rules
└── renderer.rs   # terminal renderer
```

## Design principles for this implementation

### Zero external dependencies

The current crate has no external Rust dependencies. This is deliberate: fewer dependencies means fewer long-term maintenance risks.

### Parser first, browser later

RetroX starts with the document format and renderer before attempting a full browser. The current terminal renderer proves the core loop:

```text
RNMDL source
↓
lexer
↓
parser
↓
AST
↓
validator
↓
terminal renderer
```

### Frozen modules, conservative evolution

RNMDL is intended to evolve through named, frozen module sets. A future renderer may support newer modules, but older documents should remain valid and renderable.

### Strict validation

The validator rejects invalid documents instead of guessing what the creator meant. This keeps documents predictable, portable, and easier to preserve.

## Roadmap

### GN-Z11 / v0.0.0

- [x] lexer
- [x] parser
- [x] AST
- [x] validator
- [x] terminal renderer
- [x] CLI commands
- [ ] formal RNMDL GN-Z11 specification document
- [ ] example RNMDL documents
- [ ] test suite
- [ ] cleaner error recovery

### Next technical milestones

- [ ] package format for complete sites
- [ ] local navigation between RNMDL pages
- [ ] basic asset loading strategy
- [ ] ORNPS / ARNPS protocol design draft
- [ ] cryptographic signing experiment
- [ ] RetroBrowser prototype renderer

## What RetroX is not

RetroX is not:

- a Chrome or Firefox replacement
- a full JavaScript application platform
- a Web3/blockchain project
- a social media platform
- a nostalgia skin over the modern web
- a lawless anonymous network

RetroX is focused on durable publishing, preserved websites, privacy-respecting communities, and human-scale digital spaces.

## License

This repository is currently licensed under **CC-BY-4.0**, as declared in `Cargo.toml`.

## Warning

RetroX is still a prototype. The RNMDL syntax, internal architecture, and command behavior may change while the project is in early development.

Once a module set is formally frozen, compatibility should become the priority.
