# dyson-common-ui (ui)

## Overview

Shared React 19 UI primitives and design tokens for the `dyson` and
`dyson-swarm` web apps, published as the `dyson-common-ui` npm package. The
`build` script bundles `ui/index.js` into `dist/index.js` (ESM, with `react`
and `react-dom` as externals).

The package exports:

- `.`: `dist/index.js`
- `./tokens.css`
- `./components.css`
- `./vite`: the Node-side `ui/vite.js`

Consumers import the CSS files themselves. The Rust crate embeds `tokens.css`
through `dyson_common::ui::DESIGN_TOKENS_CSS`.

## Interface

### Exports re-exported by `index.js`

- Modals [Modal.jsx:Modal] [ConfirmModal.jsx:ConfirmModal] [ConfirmModal.jsx:useConfirm] [SubscriptionConnectModal.jsx:SubscriptionConnectModal]:
  - `Modal`
  - `ConfirmModal` and `useConfirm`
  - `SubscriptionConnectModal` and `SUBSCRIPTION_CONNECT_PROVIDERS` (`codex` device flow; `claude` and `deepseek` code flow)
- Basic components: `EmptyState`, `Combobox`, `SegmentedControl` [EmptyState.jsx:EmptyState] [Combobox.jsx:Combobox] [SegmentedControl.jsx:SegmentedControl]
- State [createStore.js:createStore] [store.js:createUseAppState]:
  - `createStore` and `deepFreeze`
  - `createUseAppState`
- Browser helpers:
  - `base64url`, `randomString`, `pkceChallenge` [pkce.js:pkceChallenge]
  - `downloadBlob` [download.js:downloadBlob]
  - `useEscapeKey` [useEscapeKey.js:useEscapeKey]
  - `useStored` and `useDebounced` [useStored.js:useStored]
  - `copyToClipboard`, `copyText`, `copyFetched` [clipboard.js:copyText]
  - `createBuildWatcher` [buildWatch.js:createBuildWatcher]
- Brand marks [DysonMark.jsx:DysonMark] [GlitchMark.jsx:GlitchMark] [marks.js:MARK_VARIANTS]:
  - `DysonMark`, `ComputerMark`, `DysonGlyph` (the `Glyph` component, re-exported under that name), `DYSON_BLUE`
  - `setBrandMarkVariant` and `useBrandMarkVariant`
  - `GlitchMark`
  - `MARK_VARIANTS`, `MARK_VARIANT_LABELS`, `MARK_VARIANT_NAMES`, `DEFAULT_MARK_VARIANT` (`orbits`)
- Formatting: `formatUsd`, `formatBalance`, `formatTokens`, `formatCount`, `formatBytes`, `formatDuration` [format.js:formatUsd]
- Paging and sorting [Pager.jsx:Pager] [sort.js:sortRows] [Sorting.jsx:useSorter]:
  - `Pager`, `PAGE_SIZES` (`[25, 50, 100, 200]`), `usePageSize`
  - `SORT_NATURAL`, `SORT_DIRECTION`, `firstDesc`, `parseSort`, `sortParam`, `sortWays`, `sortRows`
  - `useSorter`, `SortTh`, `SortPill`, `SortPicker`, `SortGlyph`
- Theme: `createThemeController`, and `THEME_MODES` (`['system', 'light', 'dark']`) [theme.js:createThemeController] [theme.js:MODES]
- Report views: `SecurityReportView`, `EvalResultsView`, `EvalReportView` [SecurityReportView.jsx:SecurityReportView] [EvalResultsView.jsx:EvalResultsView] [EvalReportView.jsx:EvalReportView]
- Gateway layout: `PageHeader`, `MetricGrid`, `MetricCard`, `StatusBadge`, `Toolbar`, `DataTable`, `DetailList`, `JsonCodeBlock` [GatewayPrimitives.jsx:PageHeader] [GatewayPrimitives.jsx:JsonCodeBlock]

### Node-side (`./vite`)

- `buildIdPlugin({ fileName = 'version.json', global = '__BUILD__', build })`: returns `{ build, define, plugin }`. The plugin emits `version.json` at build time [vite.js:buildIdPlugin]

## Invariants

- `createStore` deep-freezes its initial state; updates go through `dispatch` [createStore.js:createStore] [createStore.js:deepFreeze]
- `setBrandMarkVariant` falls back to `DEFAULT_MARK_VARIANT` for unknown variants [DysonMark.jsx:setBrandMarkVariant]
- `createBuildWatcher` never reports a newer build when the compiled build id is missing or `dev` [buildWatch.js:createBuildWatcher]
- On `vite:preloadError`, `createBuildWatcher` reloads at most once per build. A `sessionStorage` key `<reloadKey>-<build>` prevents reload loops [buildWatch.js:createBuildWatcher]
- `usePageSize` only accepts stored sizes listed in `sizes`, otherwise uses `fallback` (50) [Pager.jsx:usePageSize]
- Theme storage [theme.js:createThemeController]:
  - the `dyson-theme` cookie wins over `localStorage`
  - only values in `MODES` are accepted
  - the cookie is domain-scoped to the apex, except on bare or IP hosts
  - the cookie is `SameSite=Lax`, with `Secure` on https
- `SubscriptionConnectModal` documents that OAuth credentials never pass through the component (component doc comment) [SubscriptionConnectModal.jsx:SubscriptionConnectModal]
- Unknown severities, and `info`/`informational`, fold into `low` in the security report view [SecurityReportView.jsx]

## Side Effects

- `fetch` calls:
  - `createBuildWatcher` polls `<url>?t=<now>` (default `/version.json`, every 5 minutes) with `cache: 'no-store'`, and also checks on `visibilitychange` and `focus` [buildWatch.js:createBuildWatcher]
  - `EvalReportView` loads `/api/mind/file?path=...` (same-origin credentials) by default, polling every 2.5 s [EvalReportView.jsx:loadFromMindRoute] [EvalReportView.jsx:POLL_MS]
  - `SecurityReportView` loads `/api/mind/file?path=...` (same-origin credentials) by default [SecurityReportView.jsx:loadFromMindRoute]
- Polling timers: `SubscriptionConnectModal` polls once per second while open [SubscriptionConnectModal.jsx:SubscriptionConnectModal]
- Storage:
  - `localStorage` reads and writes in `useStored`, `usePageSize`, and `createThemeController` [useStored.js:useStored] [Pager.jsx:usePageSize] [theme.js:createThemeController]
  - `document.cookie` writes in `createThemeController` [theme.js:createThemeController]
- Clipboard: `navigator.clipboard.writeText` and `write`, with a hidden `<textarea>` fallback [clipboard.js:copyText] [clipboard.js:copyFetched] [clipboard.js:copyToClipboard]
- Downloads: `downloadBlob` creates an object URL and a temporary `<a>`, and revokes the URL after 5 s [download.js:downloadBlob]
- Crypto: `crypto.getRandomValues` and `crypto.subtle.digest('SHA-256')` [pkce.js:randomString] [pkce.js:pkceChallenge]
- Global listeners:
  - `keydown` on `window` [useEscapeKey.js:useEscapeKey]
  - `scroll` and `resize` on `window` [Combobox.jsx:Combobox] [Sorting.jsx:SortMenu]
  - `mousedown` and `keydown` on `document` [Sorting.jsx:SortMenu]
  - `prefers-color-scheme` change [theme.js:createThemeController]
  - `Combobox` and the sort menu render through `document.body` portals [Combobox.jsx:Combobox] [Sorting.jsx:SortMenu]

## Verification

- Run: `npm test` (from `dyson-common/`; this runs `vitest run` over `ui/**/*.test.{js,jsx}` in jsdom)
