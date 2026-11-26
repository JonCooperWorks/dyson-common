// ui/MarkdownBody.jsx
import React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";

// ui/markdownSafety.js
var MD_SIGNIFICANT = /[\n\r\t*_`#>[\]|~<>&\\!$^{}]/;
var MD_AUTOLINKABLE = /(:\/\/|www\.|@)/;
var MD_BLOCK_OPENER = /^[-+=\d\s]/;
function isPlainParagraph(text) {
  return typeof text === "string" && text.length > 0 && text === text.trim() && !MD_SIGNIFICANT.test(text) && !MD_AUTOLINKABLE.test(text) && !MD_BLOCK_OPENER.test(text);
}
function safeMarkdownHref(href) {
  const value = String(href || "").trim();
  if (!value) return "";
  if (/^(https?:|mailto:)/i.test(value)) return value;
  if (/^(#|\/(?![/\\])|\.\/|\.\.\/)/.test(value)) return value;
  return "";
}

// ui/MarkdownBody.jsx
var MD_PLUGINS = [remarkGfm, remarkBreaks];
function MarkdownLink({ node: _node, href, children, ...props }) {
  const safe = safeMarkdownHref(href);
  if (!safe) return /* @__PURE__ */ React.createElement(React.Fragment, null, children);
  const external = /^(https?:|mailto:)/i.test(safe);
  return /* @__PURE__ */ React.createElement(
    "a",
    {
      ...props,
      href: safe,
      target: external ? "_blank" : void 0,
      rel: external ? "noopener noreferrer" : void 0
    },
    children
  );
}
function MarkdownImage({ src, alt, title }) {
  const label = String(alt || "").trim() || "Image";
  const href = safeMarkdownHref(src);
  if (!href) return /* @__PURE__ */ React.createElement("span", { className: "md-image-text" }, label);
  const external = /^https?:/i.test(href);
  return /* @__PURE__ */ React.createElement(
    "a",
    {
      className: "md-image-link",
      href,
      title: title || href,
      target: external ? "_blank" : void 0,
      rel: external ? "noopener noreferrer" : void 0
    },
    label
  );
}
var MD_COMPONENTS = { a: MarkdownLink, img: MarkdownImage };
var Demoted = ({ node: _node, children }) => /* @__PURE__ */ React.createElement("p", { className: "md-demoted" }, /* @__PURE__ */ React.createElement("strong", null, children));
var MD_BRIEF_COMPONENTS = {
  ...MD_COMPONENTS,
  h1: Demoted,
  h2: Demoted,
  h3: Demoted,
  h4: Demoted,
  h5: Demoted,
  h6: Demoted
};
var MarkdownBody = React.memo(function MarkdownBody2({ markdown, className = "md-body", variant = null }) {
  const source = markdown || "";
  const components = variant === "brief" ? MD_BRIEF_COMPONENTS : MD_COMPONENTS;
  return /* @__PURE__ */ React.createElement("div", { className }, isPlainParagraph(source) ? /* @__PURE__ */ React.createElement("p", null, source) : /* @__PURE__ */ React.createElement(ReactMarkdown, { remarkPlugins: MD_PLUGINS, components }, source));
});
export {
  MD_BRIEF_COMPONENTS,
  MD_COMPONENTS,
  MD_PLUGINS,
  MarkdownBody,
  isPlainParagraph,
  safeMarkdownHref
};
