# RetroX

**Permanent infrastructure for websites that should not rot.**

RetroX is an experimental alternative web platform focused on permanence, ownership, privacy, and stable publishing. It is not nostalgia for old technology. It is an attempt to build web infrastructure where a site created today can still render correctly decades later — without being rewritten every time the modern web stack mutates.

---

## Repository Structure

This is the RetroX monorepo. It contains all components of the RetroX ecosystem as separate Cargo workspace members.

```
retrox/
├── rnmdl/          # RNMDL format library and CLI
├── retrobrowser/   # Graphical RNMDL browser
├── retronet/       # ORNPS / ARNPS / RetroDNS (planned)
└── retroid/        # RetroID / RetroPasskey / RetroHub (planned)
```

---

## Components

### rnmdl
The RNMDL document format — parser, validator, AST, and terminal renderer. This is the core of the RetroX ecosystem. It exposes a Rust library used by other components and a standalone CLI.

### retrobrowser
A native graphical browser for RNMDL documents. Renders directly to the screen via Wayland shared memory buffers with no GPU requirement. GPU acceleration is planned for a future release.

### retronet *(planned)*
The RetroX network stack. Will include ORNPS (Open RetroNet Protocol Stack), ARNPS (Authenticated RetroNet Protocol Stack), and RetroDNS.

### retroid *(planned)*
Identity and social infrastructure. Will include RetroID, RetroPasskey, and RetroHub — a permanent, privacy-respecting social space.

---

## Philosophy

The modern web is built on moving targets:

- HTML, CSS, and JavaScript evolve constantly
- Frameworks are replaced every few years
- Websites depend on fragile dependency chains
- Platforms pivot, shut down, or break old content
- Maintaining a site for decades requires constant labor

RetroX takes the opposite approach:

1. **Stable specifications** — features are frozen into named module sets
2. **Static-first design** — documents before applications
3. **No tracking by design** — privacy is infrastructure, not a plugin
4. **Creator ownership** — sites should belong to their creators
5. **Permanence over novelty** — change is deliberate, versioned, and conservative

RetroX is not trying to replace the modern internet. It is alternative infrastructure for permanent digital spaces.

---

## What is RNMDL?

**RNMDL** stands for **RetroNet Markdown Language**.

It is the content format used by RetroX. Instead of supporting the full complexity of modern HTML/CSS/JavaScript, RNMDL is intentionally small, strict, and versioned. Features are frozen into named module sets. A document written today remains valid and renderable indefinitely.

### Current version: GN-Z11 (v0.0.0)

Supported features:

- Text, headings, paragraphs
- Containers and sections
- Image declarations
- Comments
- Module declaration system
- Bugfixset opt-in

### Example document

```rnmdl
<RNMDL>
from gn-z11 declare text, images
implement_bugfixsets = True

[container id="home"]
[h1]Welcome to RetroX[/h1]
[p]This is a permanent document rendered by the GN-Z11 renderer.[/p]

[section id="about"]
[h2]About[/h2]
[p]RNMDL is designed to be small, stable, and easy to preserve.[/p]
[image path="assets/logo.png" alt="RetroX project logo"]
[/section]
[/container]
```

### Planned versions

| Version | Name | Status |
|---|---|---|
| v0.0.0 | GN-Z11 | Current |
| v1.0.0 | Milky Way | Planned — links, lists, tables, more fonts |
| v1.1.0 | Sol | Planned — styling, layout |
| v1.1.1 | Luna | Planned — forms |
| v2.0.0 | Andromeda | Planned — embeds, video |

---

## Installation

### Requirements

- Rust `1.95.0`
- Cargo
- Linux with Wayland (for retrobrowser)

### Clone and build

```bash
git clone https://github.com/Disc0wd/retrox.git
cd retrox
cargo build
```

### Run the CLI

```bash
cargo run -p rnmdl -- render path/to/file.rnmdl
cargo run -p rnmdl -- validate path/to/file.rnmdl
cargo run -p rnmdl -- check path/to/file.rnmdl
cargo run -p rnmdl -- version
```

### Run the browser

```bash
cargo run -p retrobrowser -- gui path/to/file.rnmdl
```

---

## CLI Reference

### retrox (rnmdl)

```text
retrox <command> [file]

Commands:
  render   <file.rnmdl>   Parse, validate and render a document
  validate <file.rnmdl>   Validate without rendering
  check    <file.rnmdl>   Render without meta information
  version                 Show version information
  help                    Show help message
```

### retrobrowser

```text
retrobrowser <command> [file]

Commands:
  gui <file.rnmdl>   Open document in graphical browser
  version            Show version
  help               Show help message
```

---

## Validation Rules

The GN-Z11 validator is strict by design. Invalid documents are rejected, not silently degraded.

- Document must declare at least one module
- Unknown module versions are rejected
- Modules cannot be declared more than once
- Heading levels must be 1, 2, or 3
- Headings and paragraphs cannot be empty
- Image paths are required
- Image alt text is required and must be meaningful (minimum 5 characters)
- Image formats limited to: `jpg`, `jpeg`, `png`, `gif`, `webp`
- IDs must be unique within the document
- IDs must start with a letter
- IDs may only contain letters, numbers, and hyphens

---

## Architecture

### RNMDL pipeline

```
RNMDL source
↓
Lexer (tokenizer)
↓
Parser (token stream → AST)
↓
Validator (strict GN-Z11 rules)
↓
Renderer (terminal or graphical)
```

### RetroBrowser rendering pipeline

```
RNMDL document
↓
Layout engine (AST → element list, parallel image decode)
↓
Page buffer (full page pre-rendered to pixel buffer)
↓
Composite (viewport slice copied from page buffer)
↓
Wayland wl_shm (shared memory, zero-copy present)
↓
Display
```

### Design principles

**Zero external Rust dependencies in rnmdl** — the format library has no runtime dependencies. Fewer dependencies means fewer long-term maintenance risks.

**Frozen module sets** — RNMDL evolves through named, frozen versions. A future renderer may support newer modules, but older documents remain valid and renderable.

**Strict validation** — the validator rejects invalid documents instead of guessing what the creator meant. This keeps documents predictable, portable, and easier to preserve.

**Native Wayland rendering** — RetroBrowser communicates directly with the Wayland compositor via `wl_shm` shared memory buffers. No XWayland, no GPU requirement, no socket overhead.

---

## What RetroX is not

- A Chrome or Firefox replacement
- A full JavaScript application platform
- A Web3 or blockchain project
- A social media platform built on the modern web
- A nostalgia skin over existing technology
- A lawless anonymous network

RetroX is focused on durable publishing, preserved websites, privacy-respecting communities, and human-scale digital spaces.

---

## Status

> Early experimental prototype. Not production-ready.

### GN-Z11 / v0.0.0 checklist

- [x] Lexer
- [x] Parser
- [x] AST
- [x] Validator
- [x] Terminal renderer
- [x] CLI commands
- [x] Native Wayland graphical browser
- [x] PNG and JPEG image rendering
- [x] Parallel image loading
- [x] Momentum scrolling
- [ ] Formal GN-Z11 specification document
- [ ] Test suite
- [x] Example RNMDL documents

### Next milestones

- [x] Repo split into rnmdl / retrobrowser / retronet / retroid
- [ ] GPU acceleration backend (Vulkan, software fallback)
- [ ] Site package format
- [ ] Local navigation between RNMDL pages
- [ ] ORNPS / ARNPS protocol design
- [ ] RetroDNS
- [ ] RetroID and RetroPasskey
- [ ] RetroHub

---

## Acknowledgements

**[stb](https://github.com/nothings/stb)** by Sean Barrett — public domain single-header C libraries. RetroBrowser uses `stb_image` and `stb_image_resize2` for image decoding and resizing. Exceptionally well-tested, zero-dependency, and public domain — a perfect fit for RetroX's values.

**vlad2305m** — for technical consultation during the development of RetroBrowser's rendering pipeline.

---

## License

This repository is licensed under **CC-BY-4.0**.

---

> RetroX is still a prototype. The RNMDL syntax, internal architecture, and command behavior may change during early development. Once a module set is formally frozen, compatibility becomes the priority.