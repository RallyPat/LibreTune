# libretune-app — Notes for AI Agents

App-level companion to the root [AGENTS.md](../../AGENTS.md). See that file
for project-wide conventions (backend-first, legal distinction, git rules).
This file tracks app-crate specifics.

## Crate layout

- `src/` — React + Vite frontend (TypeScript)
  - `components/` — UI: `common/` (shared Dialog/Button/FormField/EmptyState
    primitives), `dialogs/` (INI-driven `DialogRenderer` + per-feature
    dialogs), `dashboards/`, `gauges/`, `tables/`, `curves/`, `tuner-ui/`
    (layout chrome), `hardware/` (port editor), `agent/` (AI panel)
  - `contexts/` — cross-cutting providers (loading, toast, unit prefs)
  - `hooks/`, `stores/` (Zustand realtime store), `services/`, `menus/`,
    `i18n/` (en + pt-BR), `utils/`, `types/`, `themes/`
- `src-tauri/` — Tauri host
  - `src/lib.rs` — glue only: AppState + `invoke_handler!` manifest
  - `src/commands/` — one module per topic (~80 files); add new commands
    there and register them in `lib.rs`

## Commands (npm)

- `npm run dev` — Vite dev server (docs sync runs first)
- `npm run tauri dev` — full Tauri app
- `npm run build` — production bundle (docs sync runs first)
- `npm run test:run` — Vitest
- `npm run typecheck` — `tsc --noEmit`
- `npm run lint` — ESLint (typescript-eslint recommended + react-hooks); CI fails on errors, warnings are advisory

## Docs sync

`public/manual/` is a generated copy of the mdBook manual (`docs/src/`),
refreshed by `npm run docs:sync` (runs automatically on `dev`/`build`).
Never edit `public/manual/` by hand — edit `docs/src/` and re-run the sync.

## Frontend conventions

- Realtime data arrives via `realtime:update` Tauri events →
  `useRealtimeStream` → `stores/realtimeStore.ts`; components subscribe
  per-channel (`useChannelValue` / `useChannels`) instead of polling.
- TypeScript interfaces mirror the Rust command payloads.
- All dialogs use the shared `Dialog` / `Button` / `FormField` primitives
  from `components/common/`; avoid re-creating per-dialog overlays.
- UI chrome text (menus, toolbar tooltips) goes through `t()` from
  `src/i18n/`; INI-derived labels and channel names pass through verbatim.

## Recent app-side work (Aug 2026)

- Per-table `.table` import/export toolbar buttons (`TableEditor2D.tsx`,
  `tuner-ui/TableEditor.tsx` → `commands/table_file_io.rs`)
- Signature-mismatch dialog auto-searches online INI sources
  (`dialogs/SignatureMismatchDialog.tsx`)
- Dialog fidelity pass: enable-condition fields disabled not hidden,
  command-button/indicatorPanel labels + dynamic units evaluated
  (`dialogs/fields/*`, `dialogs/PanelComponents.tsx`)
- AFR Delay Test dialog + trace overlay (`dialogs/AfrDelayTestDialog.tsx`,
  `dialogs/DelayTraceOverlay.tsx`)
- HotkeyEditor re-render loop fix + regression test
  (`dialogs/__tests__/HotkeyEditor.test.tsx`)

Detailed history: [CHANGELOG.md](../../CHANGELOG.md).

