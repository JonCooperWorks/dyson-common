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

// ui/ConfirmModal.jsx
import React3 from "react";
function ConfirmModal({
  title,
  confirmLabel = "confirm",
  cancelLabel = "cancel",
  busy = false,
  onConfirm,
  onCancel,
  children
}) {
  const onClose = busy ? () => {
  } : onCancel;
  return /* @__PURE__ */ React3.createElement(Modal, { onClose, label: title }, /* @__PURE__ */ React3.createElement("div", { className: "modal-header" }, /* @__PURE__ */ React3.createElement("span", null, title)), /* @__PURE__ */ React3.createElement("div", { className: "modal-body confirm-modal-body" }, children), /* @__PURE__ */ React3.createElement("div", { className: "modal-actions" }, /* @__PURE__ */ React3.createElement(
    "button",
    {
      type: "button",
      className: "btn btn-ghost",
      onClick: onCancel,
      disabled: busy
    },
    cancelLabel
  ), /* @__PURE__ */ React3.createElement(
    "button",
    {
      type: "button",
      className: "btn btn-primary confirm-modal-confirm",
      onClick: onConfirm,
      disabled: busy
    },
    busy ? "working\u2026" : confirmLabel
  )));
}
function useConfirm() {
  const [opts, setOpts] = React3.useState(null);
  const resolverRef = React3.useRef(null);
  const confirm = React3.useCallback((arg) => {
    return new Promise((resolve) => {
      resolverRef.current = resolve;
      setOpts(typeof arg === "string" ? { message: arg } : arg || {});
    });
  }, []);
  const settle = React3.useCallback((result) => {
    const resolve = resolverRef.current;
    resolverRef.current = null;
    setOpts(null);
    if (resolve) resolve(result);
  }, []);
  let confirmModal = null;
  if (opts) {
    const { message, children, ...rest } = opts;
    confirmModal = /* @__PURE__ */ React3.createElement(
      ConfirmModal,
      {
        ...rest,
        onConfirm: () => settle(true),
        onCancel: () => settle(false)
      },
      children != null ? children : /* @__PURE__ */ React3.createElement("div", { className: "confirm-modal-message" }, message)
    );
  }
  return [confirm, confirmModal];
}

// ui/EmptyState.jsx
import React4 from "react";
function EmptyState({ glyph = "\u2205", title, children, actions = null, className = "" }) {
  return /* @__PURE__ */ React4.createElement("div", { className: `ui-empty ${className}`.trim() }, /* @__PURE__ */ React4.createElement("div", { className: "ui-empty-glyph", "aria-hidden": "true" }, glyph), /* @__PURE__ */ React4.createElement("div", { className: "ui-empty-title" }, title), children ? /* @__PURE__ */ React4.createElement("div", { className: "ui-empty-body muted small" }, children) : null, actions ? /* @__PURE__ */ React4.createElement("div", { className: "ui-empty-actions" }, actions) : null);
}

// ui/Combobox.jsx
import React5 from "react";
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
  const [query, setQuery] = React5.useState(committedLabel);
  const [open, setOpen] = React5.useState(false);
  const [activeIndex, setActiveIndex] = React5.useState(-1);
  const [listPos, setListPos] = React5.useState(null);
  const inputRef = React5.useRef(null);
  const listId = React5.useId();
  React5.useEffect(() => {
    setQuery(committedLabel);
  }, [committedLabel]);
  const positionList = React5.useCallback(() => {
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
  React5.useLayoutEffect(() => {
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
  const exactMatch = (text2) => {
    const t = text2.trim().toLowerCase();
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
  return /* @__PURE__ */ React5.createElement("div", { className: "combobox" }, /* @__PURE__ */ React5.createElement(
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
    /* @__PURE__ */ React5.createElement(
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
      visible.length === 0 ? /* @__PURE__ */ React5.createElement("li", { className: "combobox-empty" }, "no matches") : visible.map((opt, i) => /* @__PURE__ */ React5.createElement(
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
        /* @__PURE__ */ React5.createElement("span", { className: "combobox-option-label" }, opt.label),
        opt.hint ? /* @__PURE__ */ React5.createElement("span", { className: "combobox-hint" }, opt.hint) : null
      ))
    ),
    document.body
  ) : null);
}

// ui/createStore.js
function deepFreeze(obj) {
  if (obj === null || typeof obj !== "object") return obj;
  if (Object.isFrozen(obj)) return obj;
  Object.freeze(obj);
  for (const key of Object.keys(obj)) {
    const v = obj[key];
    if (v !== null && typeof v === "object" && !Object.isFrozen(v)) deepFreeze(v);
  }
  return obj;
}
function createStore(initial) {
  let snapshot = deepFreeze(initial);
  const listeners = /* @__PURE__ */ new Set();
  const getSnapshot = () => snapshot;
  const subscribe = (fn) => {
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  };
  const dispatch = (reducer) => {
    const next = reducer(snapshot);
    if (next === snapshot) return;
    snapshot = deepFreeze(next);
    for (const fn of listeners) fn();
  };
  return { getSnapshot, subscribe, dispatch };
}

// ui/store.js
import { useSyncExternalStore, useRef } from "react";
var identity = (s) => s;
function createUseAppState(store) {
  return function useAppState(selector) {
    const sel = selector || identity;
    const cacheRef = useRef(null);
    const getSnapshot = () => {
      const snap = store.getSnapshot();
      const cache = cacheRef.current;
      if (cache && cache.snap === snap) return cache.selected;
      const selected = sel(snap);
      if (cache && Object.is(cache.selected, selected)) {
        cacheRef.current = { snap, selected: cache.selected };
        return cache.selected;
      }
      cacheRef.current = { snap, selected };
      return selected;
    };
    return useSyncExternalStore(store.subscribe, getSnapshot, getSnapshot);
  };
}

// ui/pkce.js
function base64url(bytes) {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function randomString(byteLen = 32) {
  const a = new Uint8Array(byteLen);
  crypto.getRandomValues(a);
  return base64url(a);
}
async function pkceChallenge(verifier) {
  const data = new TextEncoder().encode(verifier);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return base64url(new Uint8Array(digest));
}

// ui/download.js
function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const revokeObjectURL = URL.revokeObjectURL?.bind(URL);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename || "download";
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => revokeObjectURL?.(url), 5e3);
}

// ui/DysonMark.jsx
import React6 from "react";
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
  return /* @__PURE__ */ React6.createElement(React6.Fragment, null, /* @__PURE__ */ React6.createElement("path", { d: SHELL, fill: color }), /* @__PURE__ */ React6.createElement("circle", { r: "26", fill: color }));
}
function DysonMark({ size = 24, color = DYSON_BLUE, title = "Dyson", ...rest }) {
  return /* @__PURE__ */ React6.createElement(
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
    /* @__PURE__ */ React6.createElement(Glyph, { color })
  );
}
function ComputerMark({ size = 24, color = DYSON_BLUE, title = "Dyson Computer", ...rest }) {
  return /* @__PURE__ */ React6.createElement(
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
    /* @__PURE__ */ React6.createElement("rect", { x: "16", y: "8", width: "208", height: "152", rx: "18", fill: "none", stroke: "currentColor", strokeWidth: "11" }),
    /* @__PURE__ */ React6.createElement("rect", { x: "108", y: "160", width: "24", height: "26", fill: "currentColor" }),
    /* @__PURE__ */ React6.createElement("rect", { x: "74", y: "186", width: "92", height: "14", rx: "7", fill: "currentColor" }),
    /* @__PURE__ */ React6.createElement("g", { transform: "translate(120,84) scale(0.62)" }, /* @__PURE__ */ React6.createElement(Glyph, { color }))
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
async function copyToClipboard(text2) {
  if (!text2) return false;
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text2);
      return true;
    }
    const ta = document.createElement("textarea");
    ta.value = text2;
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

// ui/SecurityReportView.jsx
import React7, { useState, useEffect, useMemo } from "react";
var SEVERITY_LABELS = ["critical", "high", "medium", "low"];
var SEVERITY_COLOR = {
  critical: "#cf2a2a",
  high: "#d97706",
  medium: "#ca8a04",
  low: "#6b7280"
};
function severityBucket(sev) {
  const s = String(sev || "").toLowerCase();
  if (s === "critical" || s === "high" || s === "medium") return s;
  return "low";
}
function matchesQuery(f, q) {
  if (!q) return true;
  const hay = [
    f.title,
    f.key,
    f.vulnerability_class,
    f.entry_point,
    f.sink_or_decision,
    f.root_cause,
    ...Array.isArray(f.affected_paths) ? f.affected_paths : []
  ].join("\n").toLowerCase();
  return hay.includes(q);
}
async function loadFromMindRoute(reportPath) {
  const r = await fetch("/api/mind/file?path=" + encodeURIComponent(reportPath), { credentials: "same-origin" });
  if (!r.ok) throw new Error(String(r.status));
  const payload = await r.json();
  const parsed = JSON.parse(payload.content);
  if (!parsed || !Array.isArray(parsed.findings)) throw new Error("bad doc");
  return parsed;
}
function Chevron() {
  return /* @__PURE__ */ React7.createElement(
    "svg",
    {
      width: 10,
      height: 10,
      viewBox: "0 0 16 16",
      fill: "none",
      stroke: "currentColor",
      strokeWidth: 1.5,
      strokeLinecap: "round",
      strokeLinejoin: "round"
    },
    /* @__PURE__ */ React7.createElement("path", { d: "m5 4 4 4-4 4" })
  );
}
function SecurityReportView({ reportPath, fallback, load = loadFromMindRoute }) {
  const [doc, setDoc] = useState(null);
  const [unavailable, setUnavailable] = useState(false);
  const [view, setView] = useState("cards");
  const [sevFilter, setSevFilter] = useState("");
  const [classFilter, setClassFilter] = useState("");
  const [flagFilter, setFlagFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState(() => /* @__PURE__ */ new Set());
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    let cancelled = false;
    setDoc(null);
    setUnavailable(false);
    setExpanded(/* @__PURE__ */ new Set());
    Promise.resolve().then(() => load(reportPath)).then((parsed) => {
      if (!parsed || !Array.isArray(parsed.findings)) throw new Error("bad doc");
      if (!cancelled) setDoc(parsed);
    }).catch(() => {
      if (!cancelled) setUnavailable(true);
    });
    return () => {
      cancelled = true;
    };
  }, [reportPath, load]);
  const classes = useMemo(() => {
    if (!doc) return [];
    const set = new Set(doc.findings.map((f) => f.vulnerability_class).filter(Boolean));
    return [...set].sort();
  }, [doc]);
  if (unavailable || !doc) return fallback;
  const q = query.trim().toLowerCase();
  const visible = doc.findings.map((f, idx) => ({ f, idx })).filter(({ f }) => !sevFilter || severityBucket(f.severity) === sevFilter).filter(({ f }) => !classFilter || f.vulnerability_class === classFilter).filter(({ f }) => flagFilter === "all" || flagFilter === "recurring" === !!f.recurring).filter(({ f }) => matchesQuery(f, q));
  const groups = SEVERITY_LABELS.map((sev) => ({ sev, items: visible.filter(({ f }) => severityBucket(f.severity) === sev) })).filter((g) => g.items.length > 0);
  const toggle = (idx) => setExpanded((prev) => {
    const next = new Set(prev);
    if (next.has(idx)) next.delete(idx);
    else next.add(idx);
    return next;
  });
  const copyJson = async () => {
    if (await copyToClipboard(JSON.stringify(doc, null, 2))) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  };
  const summary = doc.summary || {};
  return /* @__PURE__ */ React7.createElement("div", { className: "secrep" }, /* @__PURE__ */ React7.createElement("div", { className: "secrep-inner" }, /* @__PURE__ */ React7.createElement("div", { className: "secrep-summary" }, /* @__PURE__ */ React7.createElement("span", { className: "secrep-summary-count" }, doc.findings.length, " ", doc.findings.length === 1 ? "finding" : "findings"), SEVERITY_LABELS.map((sev) => {
    const n = summary[sev] || 0;
    if (!n) return null;
    return /* @__PURE__ */ React7.createElement("span", { key: sev, className: "secrep-sev" }, /* @__PURE__ */ React7.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), /* @__PURE__ */ React7.createElement("span", { className: "secrep-sev-n" }, n), /* @__PURE__ */ React7.createElement("span", { className: "secrep-sev-label" }, sev));
  }), /* @__PURE__ */ React7.createElement("span", null, summary.new || 0, " new \xB7 ", summary.recurring || 0, " recurring"), /* @__PURE__ */ React7.createElement("span", { className: "secrep-mono secrep-summary-path", title: doc.target && doc.target.repo_path }, doc.target && doc.target.repo_path), doc.model && doc.model.model && /* @__PURE__ */ React7.createElement("span", { className: "secrep-mono" }, doc.model.model)), /* @__PURE__ */ React7.createElement("div", { className: "secrep-controls" }, view === "cards" && /* @__PURE__ */ React7.createElement(React7.Fragment, null, /* @__PURE__ */ React7.createElement("select", { className: "secrep-select", value: sevFilter, onChange: (e) => setSevFilter(e.target.value) }, /* @__PURE__ */ React7.createElement("option", { value: "" }, "all severities"), SEVERITY_LABELS.map((s) => /* @__PURE__ */ React7.createElement("option", { key: s, value: s }, s))), classes.length > 0 && /* @__PURE__ */ React7.createElement("select", { className: "secrep-select", value: classFilter, onChange: (e) => setClassFilter(e.target.value) }, /* @__PURE__ */ React7.createElement("option", { value: "" }, "all classes"), classes.map((c) => /* @__PURE__ */ React7.createElement("option", { key: c, value: c }, c))), /* @__PURE__ */ React7.createElement("select", { className: "secrep-select", value: flagFilter, onChange: (e) => setFlagFilter(e.target.value) }, /* @__PURE__ */ React7.createElement("option", { value: "all" }, "new + recurring"), /* @__PURE__ */ React7.createElement("option", { value: "new" }, "new"), /* @__PURE__ */ React7.createElement("option", { value: "recurring" }, "recurring")), /* @__PURE__ */ React7.createElement(
    "input",
    {
      className: "secrep-search",
      type: "search",
      placeholder: "search",
      value: query,
      onChange: (e) => setQuery(e.target.value)
    }
  )), view === "json" && /* @__PURE__ */ React7.createElement("button", { className: "secrep-btn", onClick: copyJson }, copied ? "copied" : "copy"), /* @__PURE__ */ React7.createElement("span", { className: "secrep-spacer" }), /* @__PURE__ */ React7.createElement(
    "button",
    {
      className: "secrep-btn secrep-toggle",
      "data-on": view === "cards",
      onClick: () => setView("cards")
    },
    "rendered"
  ), /* @__PURE__ */ React7.createElement(
    "button",
    {
      className: "secrep-btn secrep-toggle",
      "data-on": view === "json",
      onClick: () => setView("json")
    },
    "json"
  )), view === "json" ? /* @__PURE__ */ React7.createElement("div", { className: "secrep-json-wrap" }, /* @__PURE__ */ React7.createElement("pre", { className: "secrep-json" }, JSON.stringify(doc, null, 2))) : /* @__PURE__ */ React7.createElement(React7.Fragment, null, visible.length === 0 && /* @__PURE__ */ React7.createElement("div", { className: "secrep-empty" }, doc.findings.length === 0 ? "No confirmed findings." : "No findings match."), groups.map(({ sev, items }) => /* @__PURE__ */ React7.createElement("div", { key: sev, className: "secrep-group" }, /* @__PURE__ */ React7.createElement("div", { className: "secrep-eyebrow" }, /* @__PURE__ */ React7.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), sev, " \xB7 ", items.length), items.map(({ f, idx }) => /* @__PURE__ */ React7.createElement(
    FindingCard,
    {
      key: idx,
      finding: f,
      sev: severityBucket(f.severity),
      open: expanded.has(idx),
      onToggle: () => toggle(idx)
    }
  )))))));
}
function FindingCard({ finding: f, sev, open, onToggle }) {
  return /* @__PURE__ */ React7.createElement("div", { className: "secrep-card" }, /* @__PURE__ */ React7.createElement("div", { className: "secrep-card-head", onClick: onToggle }, /* @__PURE__ */ React7.createElement("span", { className: "secrep-caret", style: { transform: open ? "rotate(90deg)" : "none" } }, /* @__PURE__ */ React7.createElement(Chevron, null)), /* @__PURE__ */ React7.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), /* @__PURE__ */ React7.createElement("span", { className: "secrep-card-title" }, f.title || f.run_finding_id || f.id), f.key && /* @__PURE__ */ React7.createElement("span", { className: "secrep-chip" }, f.key), f.recurring && /* @__PURE__ */ React7.createElement("span", { className: "secrep-chip" }, "recurring x", f.occurrences)), open && /* @__PURE__ */ React7.createElement("div", { className: "secrep-card-body" }, row("class", f.vulnerability_class && /* @__PURE__ */ React7.createElement("span", { className: "secrep-chip" }, f.vulnerability_class)), row("boundary", text(f.trust_boundary)), row("flow", (f.entry_point || f.sink_or_decision) && /* @__PURE__ */ React7.createElement("span", { className: "secrep-mono" }, f.entry_point, f.entry_point && f.sink_or_decision ? " \u2192 " : "", f.sink_or_decision)), row("root cause", text(f.root_cause)), row("reachability", text(f.reachability)), row("impact", text(f.tenant_or_instance_impact)), row("rationale", text(f.severity_rationale)), row("fix", text(f.fix_recommendation)), row("paths", Array.isArray(f.affected_paths) && f.affected_paths.length > 0 && /* @__PURE__ */ React7.createElement("span", { className: "secrep-paths" }, f.affected_paths.map((p, i) => /* @__PURE__ */ React7.createElement("span", { key: i, className: "secrep-chip" }, p)))), row("evidence", Array.isArray(f.evidence) && f.evidence.length > 0 && /* @__PURE__ */ React7.createElement("pre", { className: "secrep-pre secrep-pre-wrap" }, f.evidence.join("\n"))), row("patch", !!(f.suggested_patch && f.suggested_patch.trim()) && /* @__PURE__ */ React7.createElement("pre", { className: "secrep-pre" }, f.suggested_patch))));
}
function text(v) {
  return v && String(v).trim() ? /* @__PURE__ */ React7.createElement("span", null, v) : null;
}
function row(label, value) {
  if (!value) return null;
  return /* @__PURE__ */ React7.createElement(React7.Fragment, { key: label }, /* @__PURE__ */ React7.createElement("span", { className: "secrep-label" }, label), /* @__PURE__ */ React7.createElement("span", { className: "secrep-value" }, value));
}
export {
  Combobox,
  ComputerMark,
  ConfirmModal,
  DYSON_BLUE,
  DysonMark,
  EmptyState,
  Modal,
  SecurityReportView,
  MODES as THEME_MODES,
  base64url,
  copyToClipboard,
  createStore,
  createThemeController,
  createUseAppState,
  deepFreeze,
  downloadBlob,
  formatBalance,
  formatBytes,
  formatCount,
  formatDuration,
  formatTokens,
  formatUsd,
  pkceChallenge,
  randomString,
  useConfirm,
  useEscapeKey
};
