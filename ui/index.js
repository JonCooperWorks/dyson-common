// dyson-common-ui — shared React UI primitives for dyson and dyson-swarm.
// Pair the components with ./tokens.css (design tokens) and
// ./components.css (their styles), both imported by the consuming app.
export { Modal } from './Modal.jsx';
export { ConfirmModal, useConfirm } from './ConfirmModal.jsx';
export { EmptyState } from './EmptyState.jsx';
export { Combobox } from './Combobox.jsx';
export { deepFreeze, createStore } from './createStore.js';
export { createUseAppState } from './store.js';
export { base64url, randomString, pkceChallenge } from './pkce.js';
export { downloadBlob } from './download.js';
export { useEscapeKey } from './useEscapeKey.js';
export {
  DysonMark,
  Glyph as DysonGlyph,
  DYSON_BLUE,
  setBrandMarkVariant,
  useBrandMarkVariant,
} from './DysonMark.jsx';
export { GlitchMark } from './GlitchMark.jsx';
export {
  MARK_VARIANTS,
  MARK_VARIANT_LABELS,
  MARK_VARIANT_NAMES,
  DEFAULT_MARK_VARIANT,
} from './marks.js';
export {
  formatUsd,
  formatBalance,
  formatTokens,
  formatBytes,
} from './format.js';
export { copyToClipboard, copyText, copyFetched } from './clipboard.js';
export { createBuildWatcher } from './buildWatch.js';
// MarkdownBody itself is in the separate dyson-common-ui/markdown entry.
export { isPlainParagraph, safeMarkdownHref } from './markdownSafety.js';
export { createSseParser, readSse, SSE_DEFAULT_MAX_BUFFER } from './sse.js';
export { useStored, useDebounced } from './useStored.js';
export { SegmentedControl } from './SegmentedControl.jsx';
export { Pager, PAGE_SIZES, usePageSize } from './Pager.jsx';
export {
  SORT_NATURAL,
  SORT_DIRECTION,
  firstDesc,
  parseSort,
  sortParam,
  sortWays,
  sortRows,
} from './sort.js';
export { useSorter, SortTh, SortPill, SortPicker, SortGlyph } from './Sorting.jsx';
export { createThemeController, MODES as THEME_MODES } from './theme.js';
export { SecurityReportView } from './SecurityReportView.jsx';
export { EvalResultsView } from './EvalResultsView.jsx';
export { EvalReportView } from './EvalReportView.jsx';
export {
  SubscriptionConnectModal,
  SUBSCRIPTION_CONNECT_PROVIDERS,
} from './SubscriptionConnectModal.jsx';
export {
  PageHeader,
  MetricGrid,
  MetricCard,
  StatusBadge,
  Toolbar,
  DataTable,
  DetailList,
  JsonCodeBlock,
} from './GatewayPrimitives.jsx';
export * as fmt from './fmt.js';
export { StatusDot, STATUS_DOT_STATES } from './StatusDot.jsx';
export { SeverityTag, severityLevel } from './SeverityTag.jsx';
export { SwarmBar, useSwarmContext } from './SwarmBar.jsx';
export {
  SWARM_CONTEXT_PATH,
  parseSwarmContext,
  loadSwarmContext,
  getSwarmContext,
  swarmAgentHref,
  swarmHref,
  safeName,
  isAgentId,
} from './swarmContext.js';
