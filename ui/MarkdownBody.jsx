/* Markdown from agents and users, rendered without raw HTML (react-markdown
 * escapes it; no rehype-raw), with links limited to safeMarkdownHref and
 * images never loaded: an image URL in agent-written markdown is a tracking
 * and exfiltration channel, so its alt text links to it instead.
 *
 * Lives behind its own entry, `dyson-common-ui/markdown`, so apps that do
 * not render markdown never load the parser; react-markdown, remark-gfm
 * and remark-breaks are (optional) peer dependencies of that entry. Lazy-
 * load it where markdown is rare:
 *
 *   const MarkdownBody = React.lazy(() =>
 *     import('dyson-common-ui/markdown').then((m) => ({ default: m.MarkdownBody })));
 *
 * Styles hook on .md-body, .md-demoted, .md-image-link and .md-image-text. */
import React from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkBreaks from 'remark-breaks';
import { isPlainParagraph, safeMarkdownHref } from './markdownSafety.js';

export const MD_PLUGINS = [remarkGfm, remarkBreaks];

function MarkdownLink({ node: _node, href, children, ...props }) {
  const safe = safeMarkdownHref(href);
  if (!safe) return <>{children}</>;
  const external = /^(https?:|mailto:)/i.test(safe);
  return (
    <a {...props} href={safe}
      target={external ? '_blank' : undefined}
      rel={external ? 'noopener noreferrer' : undefined}>
      {children}
    </a>
  );
}

function MarkdownImage({ src, alt, title }) {
  const label = String(alt || '').trim() || 'Image';
  const href = safeMarkdownHref(src);
  if (!href) return <span className="md-image-text">{label}</span>;
  const external = /^https?:/i.test(href);
  return (
    <a className="md-image-link" href={href} title={title || href}
      target={external ? '_blank' : undefined}
      rel={external ? 'noopener noreferrer' : undefined}>
      {label}
    </a>
  );
}

export const MD_COMPONENTS = { a: MarkdownLink, img: MarkdownImage };

// Headings become bold lines, so a brief cannot outrank the page's own.
const Demoted = ({ node: _node, children }) => <p className="md-demoted"><strong>{children}</strong></p>;
export const MD_BRIEF_COMPONENTS = {
  ...MD_COMPONENTS, h1: Demoted, h2: Demoted, h3: Demoted, h4: Demoted, h5: Demoted, h6: Demoted,
};

/// Memoised: the input is an immutable string and the pipeline is not
/// cheap. Plain prose skips the pipeline (see isPlainParagraph).
export const MarkdownBody = React.memo(function MarkdownBody({ markdown, className = 'md-body', variant = null }) {
  const source = markdown || '';
  const components = variant === 'brief' ? MD_BRIEF_COMPONENTS : MD_COMPONENTS;
  return (
    <div className={className}>
      {isPlainParagraph(source) ? (
        <p>{source}</p>
      ) : (
        <ReactMarkdown remarkPlugins={MD_PLUGINS} components={components}>
          {source}
        </ReactMarkdown>
      )}
    </div>
  );
});
