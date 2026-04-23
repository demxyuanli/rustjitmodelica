# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Diagram / structure editor

- Rust payload shapes: `src-tauri/src/diagram/types.rs` (and related `diagram` modules).
- TypeScript session and sync: `src/structureEditor/` (keep JSON and field names aligned with the Tauri layer when extending the wire format).
- Frontend checks: `npm run test` (Vitest).
