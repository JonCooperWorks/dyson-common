// ui/Modal.jsx
import React2 from "react";

// ui/useEscapeKey.js
import React from "react";
function useEscapeKey(handler) {
  React.useEffect(() => {
    if (!handler) return void 0;
    const onKey = (e) => {
      if (e.key === "Escape") handler(e);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handler]);
}

// ui/Modal.jsx
var FOCUSABLE_SELECTOR = [
  "a[href]",
  "area[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])'
].join(",");
function focusableWithin(root) {
  if (!root) return [];
  return Array.prototype.filter.call(
    root.querySelectorAll(FOCUSABLE_SELECTOR),
    (el) => el.tabIndex !== -1 && !el.hasAttribute("disabled")
  );
}
function Modal({
  onClose,
  label,
  labelledBy,
  className = "modal",
  scrimClassName = "modal-scrim",
  closeOnScrimClick = true,
  children
}) {
  useEscapeKey(onClose);
  const dialogRef = React2.useRef(null);
  const openerRef = React2.useRef(void 0);
  if (openerRef.current === void 0) {
    openerRef.current = typeof document !== "undefined" ? document.activeElement : null;
  }
  React2.useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.contains(document.activeElement)) {
      const focusables = focusableWithin(dialog);
      (focusables[0] || dialog).focus();
    }
    return () => {
      const opener = openerRef.current;
      if (opener && opener.isConnected && typeof opener.focus === "function") {
        opener.focus();
      }
    };
  }, []);
  const onScrimClick = (e) => {
    if (closeOnScrimClick && e.target === e.currentTarget) onClose?.();
  };
  const onKeyDown = (e) => {
    if (e.key !== "Tab") return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusables = focusableWithin(dialog);
    if (focusables.length === 0) {
      e.preventDefault();
      return;
    }
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = document.activeElement;
    if (e.shiftKey) {
      if (active === first || !dialog.contains(active)) {
        e.preventDefault();
        last.focus();
      }
    } else if (active === last || !dialog.contains(active)) {
      e.preventDefault();
      first.focus();
    }
  };
  return /* @__PURE__ */ React2.createElement("div", { className: scrimClassName, onClick: onScrimClick }, /* @__PURE__ */ React2.createElement(
    "div",
    {
      ref: dialogRef,
      className,
      role: "dialog",
      "aria-modal": "true",
      "aria-label": label,
      "aria-labelledby": labelledBy,
      tabIndex: -1,
      onKeyDown
    },
    children
  ));
}

// ui/Combobox.jsx
import React3 from "react";
import { createPortal } from "react-dom";
function Combobox({
  options,
  value = "",
  onSelect,
  onClear,
  placeholder = "",
  disabled = false,
  ariaLabel
}) {
  const opts = options || [];
  const selected = opts.find((o) => o.value === value) || null;
  const committedLabel = selected ? selected.label : "";
  const [query, setQuery] = React3.useState(committedLabel);
  const [open, setOpen] = React3.useState(false);
  const [activeIndex, setActiveIndex] = React3.useState(-1);
  const [listPos, setListPos] = React3.useState(null);
  const inputRef = React3.useRef(null);
  const listId = React3.useId();
  React3.useEffect(() => {
    setQuery(committedLabel);
  }, [committedLabel]);
  const positionList = React3.useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const openUp = spaceBelow < 240 && rect.top > spaceBelow;
    setListPos({
      left: rect.left,
      width: rect.width,
      top: openUp ? void 0 : rect.bottom + 2,
      bottom: openUp ? window.innerHeight - rect.top + 2 : void 0
    });
  }, []);
  React3.useLayoutEffect(() => {
    if (!open) return void 0;
    positionList();
    window.addEventListener("scroll", positionList, true);
    window.addEventListener("resize", positionList);
    return () => {
      window.removeEventListener("scroll", positionList, true);
      window.removeEventListener("resize", positionList);
    };
  }, [open, positionList]);
  const editing = query.trim() !== committedLabel.trim();
  const needle = editing ? query.trim().toLowerCase() : "";
  const visible = needle ? opts.filter((o) => o.label.toLowerCase().includes(needle) || String(o.value).toLowerCase().includes(needle)) : opts;
  const exactMatch = (text) => {
    const t = text.trim().toLowerCase();
    if (!t) return null;
    return opts.find((o) => o.label.toLowerCase() === t || String(o.value).toLowerCase() === t) || null;
  };
  const commit = (opt) => {
    if (!opt) return;
    onSelect?.(opt);
    setQuery(opt.label);
    setOpen(false);
    setActiveIndex(-1);
  };
  const onChange = (e) => {
    setQuery(e.target.value);
    setOpen(true);
    setActiveIndex(-1);
    if (!e.target.value.trim()) onClear?.();
  };
  const onKeyDown = (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setActiveIndex((i) => Math.min(i + 1, visible.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      if (!open) return;
      e.preventDefault();
      const pick = activeIndex >= 0 && activeIndex < visible.length ? visible[activeIndex] : exactMatch(query) || (visible.length === 1 ? visible[0] : null);
      if (pick) commit(pick);
    } else if (e.key === "Escape") {
      if (!open) return;
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      setQuery(committedLabel);
      setActiveIndex(-1);
    }
  };
  return /* @__PURE__ */ React3.createElement("div", { className: "combobox" }, /* @__PURE__ */ React3.createElement(
    "input",
    {
      type: "text",
      role: "combobox",
      "aria-expanded": open,
      "aria-controls": listId,
      "aria-autocomplete": "list",
      autoComplete: "off",
      autoCorrect: "off",
      autoCapitalize: "off",
      spellCheck: false,
      "aria-label": ariaLabel,
      ref: inputRef,
      className: "combobox-input",
      value: query,
      placeholder,
      disabled,
      onChange,
      onFocus: (e) => {
        setOpen(true);
        e.target.select();
      },
      onBlur: () => {
        setOpen(false);
        setQuery(committedLabel);
        setActiveIndex(-1);
      },
      onKeyDown
    }
  ), open && !disabled && listPos ? createPortal(
    /* @__PURE__ */ React3.createElement(
      "ul",
      {
        className: "combobox-list",
        id: listId,
        role: "listbox",
        style: {
          left: listPos.left,
          width: listPos.width,
          top: listPos.top,
          bottom: listPos.bottom
        },
        onMouseDown: (e) => e.preventDefault()
      },
      visible.length === 0 ? /* @__PURE__ */ React3.createElement("li", { className: "combobox-empty" }, "no matches") : visible.map((opt, i) => /* @__PURE__ */ React3.createElement(
        "li",
        {
          key: opt.value,
          role: "option",
          "aria-selected": opt.value === value,
          className: `combobox-option${i === activeIndex ? " is-active" : ""}`,
          onMouseDown: (e) => {
            e.preventDefault();
            commit(opt);
          }
        },
        /* @__PURE__ */ React3.createElement("span", { className: "combobox-option-label" }, opt.label),
        opt.hint ? /* @__PURE__ */ React3.createElement("span", { className: "combobox-hint" }, opt.hint) : null
      ))
    ),
    document.body
  ) : null);
}

// ui/DysonMark.jsx
import React4 from "react";
var PANELS = [
  "M56.9,-11.1 L94.2,-18.3 L94.2,18.3 L56.9,11.1Z",
  "M54.8,18.9 L90.8,31.3 L72.5,63.0 L43.8,38.1Z",
  "M38.1,43.8 L63.0,72.5 L31.3,90.8 L18.9,54.8Z",
  "M11.1,56.9 L18.3,94.2 L-18.3,94.2 L-11.1,56.9Z",
  "M-18.9,54.8 L-31.3,90.8 L-63.0,72.5 L-38.1,43.8Z",
  "M-43.8,38.1 L-72.5,63.0 L-90.8,31.3 L-54.8,18.9Z",
  "M-56.9,11.1 L-94.2,18.3 L-94.2,-18.3 L-56.9,-11.1Z",
  "M-54.8,-18.9 L-90.8,-31.3 L-72.5,-63.0 L-43.8,-38.1Z",
  "M-38.1,-43.8 L-63.0,-72.5 L-31.3,-90.8 L-18.9,-54.8Z",
  "M-11.1,-56.9 L-18.3,-94.2 L18.3,-94.2 L11.1,-56.9Z",
  "M22.8,-66.2 L35.2,-102.1 L70.9,-81.5 L45.9,-52.8Z",
  "M52.8,-45.9 L81.5,-70.9 L102.1,-35.2 L66.2,-22.8Z"
];
var SHELL = PANELS.join(" ");
var DYSON_BLUE = "#3b82f6";
function Glyph({ color }) {
  return /* @__PURE__ */ React4.createElement(React4.Fragment, null, /* @__PURE__ */ React4.createElement("path", { d: SHELL, fill: color }), /* @__PURE__ */ React4.createElement("circle", { r: "26", fill: color }));
}
function DysonMark({ size = 24, color = DYSON_BLUE, title = "Dyson", ...rest }) {
  return /* @__PURE__ */ React4.createElement(
    "svg",
    {
      width: size,
      height: size,
      viewBox: "-112 -112 224 224",
      role: "img",
      "aria-label": title,
      style: { display: "block" },
      ...rest
    },
    /* @__PURE__ */ React4.createElement(Glyph, { color })
  );
}
function ComputerMark({ size = 24, color = DYSON_BLUE, title = "Dyson Computer", ...rest }) {
  return /* @__PURE__ */ React4.createElement(
    "svg",
    {
      width: size,
      height: size,
      viewBox: "0 0 240 224",
      role: "img",
      "aria-label": title,
      style: { display: "block" },
      ...rest
    },
    /* @__PURE__ */ React4.createElement("rect", { x: "16", y: "8", width: "208", height: "152", rx: "18", fill: "none", stroke: "currentColor", strokeWidth: "11" }),
    /* @__PURE__ */ React4.createElement("rect", { x: "108", y: "160", width: "24", height: "26", fill: "currentColor" }),
    /* @__PURE__ */ React4.createElement("rect", { x: "74", y: "186", width: "92", height: "14", rx: "7", fill: "currentColor" }),
    /* @__PURE__ */ React4.createElement("g", { transform: "translate(120,84) scale(0.62)" }, /* @__PURE__ */ React4.createElement(Glyph, { color }))
  );
}

// ui/format.js
function formatUsd(value) {
  if (value === null || value === void 0 || Number.isNaN(Number(value))) return "$0.00";
  const n = Number(value);
  const sign = n < 0 ? "-" : "";
  const abs = Math.abs(n);
  if (abs > 0 && abs < 0.01) return `${sign}$${trimFixed(abs, abs < 1e-4 ? 6 : 4)}`;
  return `${sign}$${abs.toFixed(2)}`;
}
function formatBalance(value) {
  const cents = Math.round((Number(value) || 0) * 100);
  if (cents === 0) return "$0.00";
  const abs = (Math.abs(cents) / 100).toFixed(2);
  return cents < 0 ? `\u2212$${abs}` : `$${abs}`;
}
function formatTokens(value) {
  const n = Number(value || 0);
  if (n >= 1e9) return `${trimFixed(n / 1e9, 1)}B`;
  if (n >= 1e6) return `${trimFixed(n / 1e6, 1)}M`;
  if (n >= 1e3) return `${trimFixed(n / 1e3, 1)}k`;
  return String(Math.max(0, Math.round(n)));
}
function formatCount(value) {
  return new Intl.NumberFormat("en-US").format(Number(value || 0));
}
function formatBytes(n) {
  if (!Number.isFinite(n) || n <= 0) return "\u2014";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}
function formatDuration(seconds) {
  const s = Math.max(0, Math.round(Number(seconds || 0)));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.round(s / 60)}m`;
  const h = s / 3600;
  return h < 100 ? `${trimFixed(h, 1)}h` : `${Math.round(h)}h`;
}
function trimFixed(value, digits) {
  return Number(value).toFixed(digits).replace(/\.0+$|(\.\d*?)0+$/u, "$1");
}

// ui/clipboard.js
async function copyToClipboard(text) {
  if (!text) return false;
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    return true;
  } catch (_) {
    return false;
  }
}

// ui/theme.js
var COOKIE = "dyson-theme";
var MODES = ["system", "light", "dark"];
var SURFACE = { dark: "#161922", light: "#ffffff" };
function createThemeController({ storageKey, stripInstanceLabel = false }) {
  function cookieDomain() {
    let host = location.hostname.replace(/\.$/, "").toLowerCase();
    if (stripInstanceLabel) host = host.replace(/^[^.]+\./, "");
    if (!host.includes(".") || /^[0-9.]+$/.test(host)) return null;
    return host;
  }
  function readCookie() {
    const m = document.cookie.match(/(?:^|;\s*)dyson-theme=([^;]*)/);
    const v = m && decodeURIComponent(m[1]);
    return MODES.includes(v) ? v : null;
  }
  function writeCookie(mode) {
    const dom = cookieDomain();
    document.cookie = `${COOKIE}=${mode}; Path=/; Max-Age=31536000; SameSite=Lax` + (dom ? `; Domain=${dom}` : "") + (location.protocol === "https:" ? "; Secure" : "");
  }
  function getMode() {
    const shared = readCookie();
    if (shared) return shared;
    try {
      const v = localStorage.getItem(storageKey);
      return MODES.includes(v) ? v : "system";
    } catch {
      return "system";
    }
  }
  function resolvedTheme(mode = getMode()) {
    if (mode === "system") {
      return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
    }
    return mode;
  }
  function applyMode(mode) {
    const root = document.documentElement;
    if (mode === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", mode);
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute("content", SURFACE[resolvedTheme(mode)]);
  }
  function setMode(mode) {
    const next = MODES.includes(mode) ? mode : "system";
    try {
      localStorage.setItem(storageKey, next);
    } catch {
    }
    writeCookie(next);
    applyMode(next);
    return next;
  }
  function toggleTheme() {
    return setMode(resolvedTheme() === "dark" ? "light" : "dark");
  }
  function initTheme() {
    applyMode(getMode());
    window.matchMedia?.("(prefers-color-scheme: light)").addEventListener?.("change", () => {
      if (getMode() === "system") applyMode("system");
    });
  }
  return { getMode, resolvedTheme, applyMode, setMode, toggleTheme, initTheme };
}
export {
  Combobox,
  ComputerMark,
  DYSON_BLUE,
  DysonMark,
  Modal,
  MODES as THEME_MODES,
  copyToClipboard,
  createThemeController,
  formatBalance,
  formatBytes,
  formatCount,
  formatDuration,
  formatTokens,
  formatUsd,
  useEscapeKey
};
