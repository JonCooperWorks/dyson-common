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
import React6, { useSyncExternalStore as useSyncExternalStore2 } from "react";

// ui/marks.js
var MARK_VARIANTS = {
  "classic": [
    {
      "t": "f",
      "d": "M56.9,-11.1 L94.2,-18.3 L94.2,18.3 L56.9,11.1Z M54.8,18.9 L90.8,31.3 L72.5,63.0 L43.8,38.1Z M38.1,43.8 L63.0,72.5 L31.3,90.8 L18.9,54.8Z M11.1,56.9 L18.3,94.2 L-18.3,94.2 L-11.1,56.9Z M-18.9,54.8 L-31.3,90.8 L-63.0,72.5 L-38.1,43.8Z M-43.8,38.1 L-72.5,63.0 L-90.8,31.3 L-54.8,18.9Z M-56.9,11.1 L-94.2,18.3 L-94.2,-18.3 L-56.9,-11.1Z M-54.8,-18.9 L-90.8,-31.3 L-72.5,-63.0 L-43.8,-38.1Z M-38.1,-43.8 L-63.0,-72.5 L-31.3,-90.8 L-18.9,-54.8Z M-11.1,-56.9 L-18.3,-94.2 L18.3,-94.2 L11.1,-56.9Z M22.8,-66.2 L35.2,-102.1 L70.9,-81.5 L45.9,-52.8Z M52.8,-45.9 L81.5,-70.9 L102.1,-35.2 L66.2,-22.8Z"
    },
    {
      "t": "c",
      "cx": 0,
      "cy": 0,
      "r": 26
    }
  ],
  "evolution": [
    {
      "t": "r",
      "d": "M46.69,8.65 L77.26,10.86 L62.31,46.95 L39.14,26.9Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M26.9,39.14 L46.95,62.31 L10.86,77.26 L8.65,46.69Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M-8.65,46.69 L-10.86,77.26 L-46.95,62.31 L-26.9,39.14Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M-39.14,26.9 L-62.31,46.95 L-77.26,10.86 L-46.69,8.65Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M-46.69,-8.65 L-77.26,-10.86 L-62.31,-46.95 L-39.14,-26.9Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M-26.9,-39.14 L-46.95,-62.31 L-10.86,-77.26 L-8.65,-46.69Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M14.88,-64.45 L18.45,-94.9 L54.06,-80.14 L35.05,-56.09Z",
      "w": 5.09
    },
    {
      "t": "r",
      "d": "M65.76,-38.74 L87.74,-54.82 L100.8,-23.27 L73.89,-19.11Z",
      "w": 5.09
    },
    {
      "t": "c",
      "cx": 0,
      "cy": 0,
      "r": 25.44
    }
  ],
  "aperture": [
    {
      "t": "s",
      "d": "M55.72,22.51 A60.09,60.09 0 0 1 -2.1,60.06",
      "w": 25.04
    },
    {
      "t": "s",
      "d": "M-33.6,49.82 A60.09,60.09 0 0 1 -58.31,-14.54",
      "w": 25.04
    },
    {
      "t": "s",
      "d": "M-41.75,-43.23 A60.09,60.09 0 0 1 26.34,-54.01",
      "w": 25.04
    },
    {
      "t": "s",
      "d": "M55.26,-65.86 A85.97,85.97 0 0 1 79.71,-32.2",
      "w": 18.36
    },
    {
      "t": "c",
      "cx": 43.54,
      "cy": -89.27,
      "r": 6.68
    },
    {
      "t": "c",
      "cx": 0,
      "cy": 0,
      "r": 25.04
    }
  ],
  "orbits": [
    {
      "t": "s",
      "d": "M53.24,-30.74 A61.48,61.48 0 1 1 -48.45,-37.85",
      "w": 14.84
    },
    {
      "t": "s",
      "d": "M-52.8,84.5 A99.64,99.64 0 1 1 74.05,-66.67",
      "w": 12.72
    },
    {
      "t": "c",
      "cx": -37.85,
      "cy": -48.45,
      "r": 13.78
    },
    {
      "t": "c",
      "cx": 72.87,
      "cy": -67.95,
      "r": 10.6
    },
    {
      "t": "c",
      "cx": 0,
      "cy": 0,
      "r": 27.56
    }
  ],
  "hexshell": [
    {
      "t": "r",
      "d": "M70.06,29.02 L56.47,39.44 L40.65,32.89 L38.42,15.91 L52,5.49 L67.82,12.04Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M29.02,70.06 L12.04,67.82 L5.49,52 L15.91,38.42 L32.89,40.65 L39.44,56.47Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M-29.02,70.06 L-39.44,56.47 L-32.89,40.65 L-15.91,38.42 L-5.49,52 L-12.04,67.82Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M-70.06,29.02 L-67.82,12.04 L-52,5.49 L-38.42,15.91 L-40.65,32.89 L-56.47,39.44Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M-70.06,-29.02 L-56.47,-39.44 L-40.65,-32.89 L-38.42,-15.91 L-52,-5.49 L-67.82,-12.04Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M-29.02,-70.06 L-12.04,-67.82 L-5.49,-52 L-15.91,-38.42 L-32.89,-40.65 L-39.44,-56.47Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M42.16,-88.02 L47.47,-76.1 L39.8,-65.55 L26.82,-66.91 L21.52,-78.83 L29.18,-89.39Z",
      "w": 4.89
    },
    {
      "t": "r",
      "d": "M93.96,-42.09 L90.03,-34.03 L81.08,-33.4 L76.06,-40.84 L80,-48.9 L88.94,-49.52Z",
      "w": 4.89
    },
    {
      "t": "c",
      "cx": 0,
      "cy": 0,
      "r": 22.02
    }
  ],
  "eclipse": [
    {
      "t": "e",
      "d": "M71.52,0 A71.52,71.52 0 1 0 -71.52,0 A71.52,71.52 0 1 0 71.52,0Z M45.98,-14.47 A31.5,31.5 0 1 0 -17.03,-14.47 A31.5,31.5 0 1 0 45.98,-14.47Z"
    },
    {
      "t": "c",
      "cx": 62.01,
      "cy": -62.01,
      "r": 7.66
    },
    {
      "t": "c",
      "cx": 71.64,
      "cy": -71.64,
      "r": 4.68
    }
  ],
  "monogram": [
    {
      "t": "s",
      "d": "M-56.59,-76.18 L-56.59,76.18",
      "w": 37
    },
    {
      "t": "s",
      "d": "M-56.59,-76.18 A113.18,76.18 0 0 1 52.21,-21",
      "w": 37
    },
    {
      "t": "s",
      "d": "M49.77,26.06 A113.18,76.18 0 0 1 -56.59,76.18",
      "w": 37
    },
    {
      "t": "c",
      "cx": 93.92,
      "cy": 1.77,
      "r": 11.97
    },
    {
      "t": "c",
      "cx": -2.18,
      "cy": 0,
      "r": 26.12
    }
  ],
  "constellation": [
    {
      "t": "c",
      "cx": 0,
      "cy": 0,
      "r": 23.95
    },
    {
      "t": "c",
      "cx": 64.83,
      "cy": -10.27,
      "r": 8.23
    },
    {
      "t": "c",
      "cx": 58.49,
      "cy": 29.8,
      "r": 10.46
    },
    {
      "t": "c",
      "cx": 29.8,
      "cy": 58.49,
      "r": 12.22
    },
    {
      "t": "c",
      "cx": -10.27,
      "cy": 64.83,
      "r": 13.36
    },
    {
      "t": "c",
      "cx": -46.41,
      "cy": 46.41,
      "r": 13.75
    },
    {
      "t": "c",
      "cx": -64.83,
      "cy": 10.27,
      "r": 13.36
    },
    {
      "t": "c",
      "cx": -58.49,
      "cy": -29.8,
      "r": 12.22
    },
    {
      "t": "c",
      "cx": -29.8,
      "cy": -58.49,
      "r": 10.46
    },
    {
      "t": "c",
      "cx": 10.27,
      "cy": -64.83,
      "r": 8.23
    },
    {
      "t": "c",
      "cx": 54.61,
      "cy": -69.9,
      "r": 5.32
    },
    {
      "t": "c",
      "cx": 79.28,
      "cy": -64.2,
      "r": 3.99
    }
  ]
};
var MARK_VARIANT_LABELS = {
  "classic": "Classic",
  "evolution": "Evolution",
  "aperture": "Aperture",
  "orbits": "Orbits",
  "hexshell": "Hex shell",
  "eclipse": "Eclipse",
  "monogram": "Monogram",
  "constellation": "Constellation"
};
var MARK_VARIANT_NAMES = Object.keys(MARK_VARIANTS);
var DEFAULT_MARK_VARIANT = "orbits";

// ui/DysonMark.jsx
var DYSON_BLUE = "#3b82f6";
var markStore = createStore({ variant: DEFAULT_MARK_VARIANT });
function setBrandMarkVariant(variant) {
  const next = MARK_VARIANTS[variant] ? variant : DEFAULT_MARK_VARIANT;
  markStore.dispatch((s) => s.variant === next ? s : { variant: next });
}
function useBrandMarkVariant() {
  return useSyncExternalStore2(markStore.subscribe, () => markStore.getSnapshot().variant);
}
function Glyph({ color, variant }) {
  const shapes = MARK_VARIANTS[variant] ?? MARK_VARIANTS[DEFAULT_MARK_VARIANT];
  return shapes.map((s, i) => {
    switch (s.t) {
      case "c":
        return /* @__PURE__ */ React6.createElement("circle", { key: i, cx: s.cx, cy: s.cy, r: s.r, fill: color });
      case "e":
        return /* @__PURE__ */ React6.createElement("path", { key: i, d: s.d, fill: color, fillRule: "evenodd" });
      case "s":
        return /* @__PURE__ */ React6.createElement(
          "path",
          {
            key: i,
            d: s.d,
            fill: "none",
            stroke: color,
            strokeWidth: s.w,
            strokeLinecap: "round",
            strokeLinejoin: "round"
          }
        );
      case "r":
        return /* @__PURE__ */ React6.createElement(
          "path",
          {
            key: i,
            d: s.d,
            fill: color,
            stroke: color,
            strokeWidth: s.w,
            strokeLinejoin: "round"
          }
        );
      default:
        return /* @__PURE__ */ React6.createElement("path", { key: i, d: s.d, fill: color });
    }
  });
}
function DysonMark({ size = 24, color = DYSON_BLUE, title = "Dyson", variant, ...rest }) {
  const appVariant = useBrandMarkVariant();
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
    /* @__PURE__ */ React6.createElement(Glyph, { color, variant: variant ?? appVariant })
  );
}
function ComputerMark({ size = 24, color = DYSON_BLUE, title = "Dyson Computer", variant, ...rest }) {
  const appVariant = useBrandMarkVariant();
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
    /* @__PURE__ */ React6.createElement("g", { transform: "translate(120,84) scale(0.62)" }, /* @__PURE__ */ React6.createElement(Glyph, { color, variant: variant ?? appVariant }))
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
  const [copiedReport, setCopiedReport] = useState(false);
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
  const copyReport = async () => {
    if (await copyToClipboard(JSON.stringify(doc, null, 2))) {
      setCopiedReport(true);
      setTimeout(() => setCopiedReport(false), 1200);
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
  )), /* @__PURE__ */ React7.createElement("button", { className: "secrep-btn", onClick: copyReport }, copiedReport ? "copied" : "copy report"), /* @__PURE__ */ React7.createElement("span", { className: "secrep-spacer" }), /* @__PURE__ */ React7.createElement(
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
  const [copied, setCopied] = useState(false);
  const copyFinding = async (event) => {
    event.stopPropagation();
    if (await copyToClipboard(JSON.stringify(f, null, 2))) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  };
  return /* @__PURE__ */ React7.createElement("div", { className: "secrep-card" }, /* @__PURE__ */ React7.createElement("div", { className: "secrep-card-head", onClick: onToggle }, /* @__PURE__ */ React7.createElement("span", { className: "secrep-caret", style: { transform: open ? "rotate(90deg)" : "none" } }, /* @__PURE__ */ React7.createElement(Chevron, null)), /* @__PURE__ */ React7.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), /* @__PURE__ */ React7.createElement("span", { className: "secrep-card-title" }, f.title || f.run_finding_id || f.id), f.key && /* @__PURE__ */ React7.createElement("span", { className: "secrep-chip" }, f.key), f.recurring && /* @__PURE__ */ React7.createElement("span", { className: "secrep-chip" }, "recurring x", f.occurrences), /* @__PURE__ */ React7.createElement("button", { className: "secrep-btn secrep-copy-finding", type: "button", onClick: copyFinding }, copied ? "copied" : "copy")), open && /* @__PURE__ */ React7.createElement("div", { className: "secrep-card-body" }, row("class", f.vulnerability_class && /* @__PURE__ */ React7.createElement("span", { className: "secrep-chip" }, f.vulnerability_class)), row("boundary", text(f.trust_boundary)), row("flow", (f.entry_point || f.sink_or_decision) && /* @__PURE__ */ React7.createElement("span", { className: "secrep-mono" }, f.entry_point, f.entry_point && f.sink_or_decision ? " \u2192 " : "", f.sink_or_decision)), row("root cause", text(f.root_cause)), row("reachability", text(f.reachability)), row("impact", text(f.tenant_or_instance_impact)), row("rationale", text(f.severity_rationale)), row("fix", text(f.fix_recommendation)), row("paths", Array.isArray(f.affected_paths) && f.affected_paths.length > 0 && /* @__PURE__ */ React7.createElement("span", { className: "secrep-paths" }, f.affected_paths.map((p, i) => /* @__PURE__ */ React7.createElement("span", { key: i, className: "secrep-chip" }, p)))), row("evidence", Array.isArray(f.evidence) && f.evidence.length > 0 && /* @__PURE__ */ React7.createElement("pre", { className: "secrep-pre secrep-pre-wrap" }, f.evidence.join("\n"))), row("patch", !!(f.suggested_patch && f.suggested_patch.trim()) && /* @__PURE__ */ React7.createElement("pre", { className: "secrep-pre" }, f.suggested_patch))));
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
  DEFAULT_MARK_VARIANT,
  DYSON_BLUE,
  Glyph as DysonGlyph,
  DysonMark,
  EmptyState,
  MARK_VARIANTS,
  MARK_VARIANT_LABELS,
  MARK_VARIANT_NAMES,
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
  setBrandMarkVariant,
  useBrandMarkVariant,
  useConfirm,
  useEscapeKey
};
