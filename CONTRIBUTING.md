# Contributing to VitaForge

Thank you for your interest in contributing to **VitaForge** ❤️

VitaForge is an open-source application manager and ecosystem for the PlayStation Vita. Whether you want to fix bugs, improve the UI, optimize performance, write documentation, or suggest new features, your help is welcome.

---

## Before You Start

Please:

- Search existing issues before opening a new one.
- Keep pull requests focused on a single change.
- Be respectful and constructive in discussions.

---

## Ways to Contribute

### Report Bugs

Open an issue and include:

- Vita model (1000 / 2000 / PSTV)
- Firmware version
- HENkaku / Enso version
- VitaForge version
- Steps to reproduce
- Screenshots or logs if possible

---

### Suggest Features

Feature requests are welcome. Explain:

- What problem it solves
- Why it would be useful
- Any mockups or examples

---

### Improve the Code

Typical contributions:

- UI improvements
- Performance optimizations
- Memory usage reductions
- Network reliability fixes
- Rust refactoring
- Documentation updates

---

## Development Setup

### Requirements

- Rust (latest stable)
- vitaSDK
- Git

### Clone the repository

```bash
git clone https://github.com/josephinoo/vitaForge.git
cd vitaForge
```

### Build

```bash
cargo build
```

---

## Branching

Create a feature branch:

```bash
git checkout -b feat/my-feature
```

Examples:

- `feat/icon-cache`
- `fix/install-crash`
- `docs/update-readme`

---

## Commit Style

Use clear commit messages.

### Good

```text
feat: add icon cache for app list
fix: prevent crash when downloading VPK
docs: update installation guide
refactor: simplify network client
```

### Avoid

```text
update
fix stuff
changes
aaaa
```

---

## Pull Requests

Before submitting:

- [ ] Code builds successfully
- [ ] No unnecessary files are included
- [ ] Changes are tested on a real Vita or Vita3K when possible
- [ ] Documentation is updated if needed

PRs should include:

- A short summary
- Screenshots (for UI changes)
- Testing information

---

## Code Style

- Follow Rust formatting:

```bash
cargo fmt
```

- Run clippy if available:

```bash
cargo clippy
```

---

## Documentation

Documentation improvements are highly appreciated. This includes:

- README updates
- Setup guides
- Troubleshooting
- API or architecture notes

---

## Questions

If you are unsure about a change, open a discussion or draft pull request first.

---

## Contributors

Thanks to everyone who contributes to VitaForge.

<a href="https://github.com/josephinoo/vitaForge/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=josephinoo/vitaForge" />
</a>

---

By contributing to VitaForge, you agree that your contributions will be licensed under the same license as the project.
