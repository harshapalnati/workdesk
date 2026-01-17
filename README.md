# DeskWork (Tauri + React + Bun)

Open-source native desktop agent with a Rust/Tauri backend and React frontend, packaged with Bun + Vite. The app code lives in `deskwork/`; this root README is for GitHub visibility. For full details, see `deskwork/README.md`.

## Highlights
- Native shell: Rust commands for filesystem, shell, automation, system stats, screenshots.
- Guardrails: sensitive tools require explicit chat approval (`approve <action>` / `approve all`).
- Privacy by default: tool outputs/screenshots are redacted before sessions are saved; API keys live in the OS keyring.
- Fast tooling: Bun for install/build; Vite for the web layer.

## Quickstart (from repo root)
- Requirements: Bun (https://bun.sh), Rust toolchain.
- Install: `cd deskwork && bun install`
- Web dev: `bun run dev`
- Tauri dev: `bun run tauri dev`
- Build web: `bun run build`
- Build native: `bun run tauri build`

## Structure
- `deskwork/src/` – React UI.
- `deskwork/src-tauri/` – Rust backend (commands, agent, settings, session management).
- `deskwork/.deskwork/sessions/` – Session storage (tool output redacted).
- `deskwork/public/landing.html` – Static marketing page (open directly or deploy for GitHub Pages).

## Landing Page
- Preview: `python -m http.server 8000 -d deskwork/public` → visit `http://localhost:8000/landing.html`
- Deploy: point GitHub Pages to `deskwork/public/` or copy `landing.html`/`landing.css` to your published root.

## Contributing
1. Fork/branch from `main`.
2. Use conventional commits (e.g., `feat: add bun tooling`, `fix: gate sensitive tools`).
3. Run `bun run build` and `bun run tauri build` before PRs.
4. Open a PR with a clear summary and testing notes.

## License
TBD (choose a license, e.g., MIT/Apache-2.0).
