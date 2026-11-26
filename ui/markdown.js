// dyson-common-ui/markdown — the markdown renderer, a separate entry so the
// parser is only loaded by apps (and chunks) that render markdown. Needs
// the peer dependencies react-markdown, remark-gfm and remark-breaks.
export {
  MarkdownBody,
  MD_PLUGINS,
  MD_COMPONENTS,
  MD_BRIEF_COMPONENTS,
} from './MarkdownBody.jsx';
export { isPlainParagraph, safeMarkdownHref } from './markdownSafety.js';
