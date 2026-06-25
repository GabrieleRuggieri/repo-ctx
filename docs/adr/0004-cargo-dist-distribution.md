# ADR-0004: Distribuzione con cargo-dist

- **Stato:** Accettato
- **Data:** 2026-06-23

## Contesto

Becket deve raggiungere sviluppatori Rust, macOS/Linux nativi e ecosistema JS (`npx becket`) con binari firmati e installer ripetibili.

## Decisione

Usare **[cargo-dist](https://axodotdev.github.io/cargo-dist/)** (`dist-workspace.toml`) per:

- Build cross-platform su tag `v*.*.*` → GitHub Releases
- Installer shell e **npm** (`becket`, `becket-mcp`) — canale prebuilt principale, cross-platform
- CI generata in `.github/workflows/release.yml`

Canale complementare: build da sorgente (contributor, repo clonato).

## Conseguenze

- Release = bump versione workspace + tag `v0.x.y` + push.
- Per npm serve il secret `NPM_TOKEN` nel repo GitHub.
- Homebrew rimosso (2026-06): npm copre macOS/Linux/Windows senza tap separato.
