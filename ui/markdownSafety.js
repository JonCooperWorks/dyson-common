/* Dependency-free helpers for rendering untrusted markdown; MarkdownBody
 * (dyson-common-ui/markdown) uses them, and they are exported from the
 * main entry too. */

/// A string that CommonMark + GFM can only render as one plain paragraph,
/// so the remark pipeline (about 1ms a short blurb) can be skipped. Far
/// stricter than the set of strings that are actually inert: it rejects
/// anything that might mean something, including `://`, `www.` and `@`,
/// which remark-gfm autolinks with no punctuation cue.
const MD_SIGNIFICANT = /[\n\r\t*_`#>[\]|~<>&\\!$^{}]/;
const MD_AUTOLINKABLE = /(:\/\/|www\.|@)/;
const MD_BLOCK_OPENER = /^[-+=\d\s]/;

export function isPlainParagraph(text) {
  return typeof text === 'string'
    && text.length > 0
    && text === text.trim()
    && !MD_SIGNIFICANT.test(text)
    && !MD_AUTOLINKABLE.test(text)
    && !MD_BLOCK_OPENER.test(text);
}

/// The address a markdown link or image may point at, or '' when none:
/// http(s), mailto, and same-origin relative paths. Never `javascript:`,
/// `data:`, or a protocol-relative `//host` (nor `/\host`, which browsers
/// read the same way).
export function safeMarkdownHref(href) {
  const value = String(href || '').trim();
  if (!value) return '';
  if (/^(https?:|mailto:)/i.test(value)) return value;
  if (/^(#|\/(?![/\\])|\.\/|\.\.\/)/.test(value)) return value;
  return '';
}
