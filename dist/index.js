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

// ui/GlitchMark.jsx
import React7 from "react";
var NIGHT = "#0f1115";
var DRIFT_MAX_SIZE = 24;
var ORBITS = /* @__PURE__ */ React7.createElement(React7.Fragment, null, /* @__PURE__ */ React7.createElement("path", { d: "M53.24,-30.74 A61.48,61.48 0 1 1 -48.45,-37.85", fill: "none", strokeWidth: "14.84" }), /* @__PURE__ */ React7.createElement("path", { d: "M-52.8,84.5 A99.64,99.64 0 1 1 74.05,-66.67", fill: "none", strokeWidth: "12.72", strokeLinecap: "butt" }), /* @__PURE__ */ React7.createElement("circle", { cx: "-37.85", cy: "-48.45", r: "13.78", stroke: "none" }), /* @__PURE__ */ React7.createElement("circle", { cx: "86", cy: "-76", r: "12", stroke: "none" }), /* @__PURE__ */ React7.createElement("circle", { cx: "4.5", cy: "3.5", r: "27.56", stroke: "none" }));
function GlitchMark({ size = 28 }) {
  const id = React7.useId().replace(/:/g, "");
  const drift = size <= DRIFT_MAX_SIZE;
  return /* @__PURE__ */ React7.createElement("svg", { className: "glitch-mark", width: size, height: size, viewBox: "0 0 512 512", "aria-hidden": "true" }, /* @__PURE__ */ React7.createElement("rect", { width: "512", height: "512", rx: "112", fill: NIGHT }), drift ? /* @__PURE__ */ React7.createElement("g", { transform: "translate(256,256) scale(1.9)", fill: "#fff", stroke: "#fff", strokeLinecap: "round" }, ORBITS) : /* @__PURE__ */ React7.createElement(React7.Fragment, null, /* @__PURE__ */ React7.createElement("defs", null, /* @__PURE__ */ React7.createElement("clipPath", { id: `${id}a` }, /* @__PURE__ */ React7.createElement("rect", { x: "-130", y: "-130.4", width: "260", height: "126.8" })), /* @__PURE__ */ React7.createElement("clipPath", { id: `${id}b` }, /* @__PURE__ */ React7.createElement("rect", { x: "-130", y: "-4.4", width: "260", height: "9.8" })), /* @__PURE__ */ React7.createElement("clipPath", { id: `${id}c` }, /* @__PURE__ */ React7.createElement("rect", { x: "-130", y: "4.6", width: "260", height: "125.8" }))), /* @__PURE__ */ React7.createElement("g", { transform: "translate(256,256) scale(1.9)", fill: "#fff", stroke: "#fff", strokeLinecap: "round" }, /* @__PURE__ */ React7.createElement("g", { clipPath: `url(#${id}a)` }, ORBITS), /* @__PURE__ */ React7.createElement("g", { clipPath: `url(#${id}b)` }, /* @__PURE__ */ React7.createElement("g", { transform: "translate(-4.2 0)", fill: "#ff3b4e", stroke: "#ff3b4e", opacity: ".9" }, ORBITS)), /* @__PURE__ */ React7.createElement("g", { clipPath: `url(#${id}b)` }, /* @__PURE__ */ React7.createElement("g", { transform: "translate(7 0)" }, ORBITS)), /* @__PURE__ */ React7.createElement("g", { clipPath: `url(#${id}c)` }, ORBITS))));
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
async function copyText(text2, select = null) {
  try {
    if (!navigator.clipboard?.writeText) throw new Error("no clipboard API");
    await navigator.clipboard.writeText(text2);
    return "copied";
  } catch {
    if (legacyCopy(text2)) return "copied";
    if (select) {
      window.getSelection()?.selectAllChildren(select);
      return "selected";
    }
    return "failed";
  }
}
async function copyFetched(load) {
  const text2 = load();
  try {
    if (!window.ClipboardItem || !navigator.clipboard?.write) throw new Error("no ClipboardItem");
    await navigator.clipboard.write([new window.ClipboardItem({ "text/plain": text2.then((t) => new Blob([t], { type: "text/plain" })) })]);
    return "copied";
  } catch {
    try {
      return await copyText(await text2);
    } catch {
      return "failed";
    }
  }
}
async function copyToClipboard(text2) {
  if (!text2) return false;
  return await copyText(text2) === "copied";
}
function legacyCopy(text2) {
  if (typeof document === "undefined") return false;
  const box = document.createElement("textarea");
  box.value = text2;
  box.setAttribute("readonly", "");
  box.style.cssText = "position:fixed;top:0;left:0;opacity:0;pointer-events:none";
  const active = document.activeElement;
  document.body.appendChild(box);
  box.select();
  let done = false;
  try {
    done = document.execCommand("copy");
  } catch {
    done = false;
  }
  box.remove();
  active?.focus?.({ preventScroll: true });
  return done;
}

// ui/buildWatch.js
function createBuildWatcher({ build, url = "/version.json", every = 5 * 60 * 1e3, reloadKey = "dyson-reloaded" } = {}) {
  const current = typeof build === "string" && build ? build : "dev";
  let newer = false;
  let watching = false;
  const listeners = /* @__PURE__ */ new Set();
  async function check() {
    if (newer || current === "dev") return newer;
    try {
      const answer = await fetch(`${url}?t=${Date.now()}`, { cache: "no-store" });
      if (!answer.ok) return false;
      const served = (await answer.json())?.build;
      if (served && served !== current) {
        newer = true;
        listeners.forEach((fn) => fn(served));
      }
    } catch {
    }
    return newer;
  }
  function watch() {
    if (watching || typeof window === "undefined") return;
    watching = true;
    setInterval(() => {
      if (!document.hidden) check();
    }, every);
    document.addEventListener("visibilitychange", () => {
      if (!document.hidden) check();
    });
    window.addEventListener("focus", check);
    window.addEventListener("vite:preloadError", (event) => {
      const key = `${reloadKey}-${current}`;
      try {
        if (sessionStorage.getItem(key)) return;
        sessionStorage.setItem(key, "1");
      } catch {
        return;
      }
      event.preventDefault();
      window.location.reload();
    });
  }
  return {
    build: current,
    outdated: () => newer,
    onOutdated(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    check,
    watch
  };
}

// ui/useStored.js
import { useCallback, useEffect, useState } from "react";
function useStored(key, initial) {
  const [value, setValue] = useState(() => {
    try {
      const raw = localStorage.getItem(key);
      if (raw != null) return JSON.parse(raw);
    } catch {
    }
    return typeof initial === "function" ? initial() : initial;
  });
  const set = useCallback((next) => setValue((current) => {
    const chosen = typeof next === "function" ? next(current) : next;
    try {
      localStorage.setItem(key, JSON.stringify(chosen));
    } catch {
    }
    return chosen;
  }), [key]);
  return [value, set];
}
function useDebounced(value, delay = 250) {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(id);
  }, [value, delay]);
  return debounced;
}

// ui/SegmentedControl.jsx
import React8 from "react";
function SegmentedControl({ value, onChange, options, label, className = "" }) {
  return /* @__PURE__ */ React8.createElement("div", { className: `dc-seg ${className}`.trim(), role: "group", "aria-label": label }, options.map(([id, text2, icon]) => /* @__PURE__ */ React8.createElement("button", { type: "button", key: String(id), "aria-pressed": value === id, className: value === id ? "active" : "", onClick: () => onChange(id) }, icon, text2)));
}

// ui/Pager.jsx
import React10, { useState as useState3 } from "react";

// ui/Sorting.jsx
import React9, { useEffect as useEffect2, useLayoutEffect, useRef as useRef2, useState as useState2 } from "react";
import { createPortal as createPortal2 } from "react-dom";

// ui/sort.js
var SORT_NATURAL = { time: true, num: true, text: false, order: false };
var SORT_DIRECTION = {
  time: ["oldest first", "newest first"],
  num: ["lowest first", "highest first"],
  text: ["A \u2192 Z", "Z \u2192 A"],
  order: ["lowest first", "highest first"]
};
var firstDesc = (column) => column?.[2] ? column[2] === "desc" : SORT_NATURAL[column?.[1]] ?? false;
function parseSort(value) {
  const text2 = (value || "").trim();
  if (!text2 || text2 === "-") return null;
  return text2.startsWith("-") ? { key: text2.slice(1), desc: true } : { key: text2, desc: false };
}
var sortParam = (sort) => sort ? `${sort.desc ? "-" : ""}${sort.key}` : "";
function sortWays(kind) {
  const [up, down] = SORT_DIRECTION[kind] || SORT_DIRECTION.text;
  return SORT_NATURAL[kind] ? [[true, down], [false, up]] : [[false, up], [true, down]];
}
function sortRows(rows, sort, value) {
  if (!sort) return rows;
  const keyed = rows.map((row2, i) => {
    const v = value(row2, sort.key);
    return [v == null || v === "" || Number.isNaN(v) ? null : typeof v === "string" ? v.toLowerCase() : v, i, row2];
  });
  keyed.sort((a, b) => {
    if (a[0] === null || b[0] === null) return a[0] === b[0] ? a[1] - b[1] : a[0] === null ? 1 : -1;
    const order = a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0;
    return (sort.desc ? -order : order) || a[1] - b[1];
  });
  return keyed.map((k) => k[2]);
}

// ui/Sorting.jsx
function useSorter(columns, { value, onChange, fallback = "newest first" }) {
  const parsed = parseSort(value);
  const active = parsed && columns[parsed.key] ? parsed : null;
  const set = (next) => onChange(sortParam(next));
  const cycle = (key) => {
    const natural = firstDesc(columns[key]);
    if (active?.key !== key) return set({ key, desc: natural });
    if (active.desc === natural) return set({ key, desc: !natural });
    return set(null);
  };
  const describe = (sort) => {
    const [label, kind] = columns[sort.key] || [sort.key, "text"];
    return { label, direction: (SORT_DIRECTION[kind] || SORT_DIRECTION.text)[sort.desc ? 1 : 0] };
  };
  return { columns, active, param: sortParam(active), set, cycle, describe, fallback };
}
function SortGlyph({ dir }) {
  return /* @__PURE__ */ React9.createElement("svg", { className: `dc-sort-glyph ${dir || ""}`.trim(), width: "8", height: "12", viewBox: "0 0 8 12", "aria-hidden": "true" }, /* @__PURE__ */ React9.createElement("path", { className: "up", d: "M4 1.5 7 5H1z" }), /* @__PURE__ */ React9.createElement("path", { className: "down", d: "M4 10.5 1 7h6z" }));
}
var Stroke = ({ d, size }) => /* @__PURE__ */ React9.createElement("svg", { width: size, height: size, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "2", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true" }, /* @__PURE__ */ React9.createElement("path", { d }));
var CLOSE = "M18 6L6 18M6 6l12 12";
var DOWN = "M6 9l6 6 6-6";
function SortTh({ sorter, keys, children, className = "", title }) {
  const [open, setOpen] = useState2(false);
  const button = useRef2(null);
  const onKey = keys.find((k) => sorter.active?.key === k);
  const dir = onKey ? sorter.active.desc ? "desc" : "asc" : null;
  const primary = onKey || keys[0];
  const aside = onKey && onKey !== keys[0] ? sorter.columns[onKey][0] : null;
  const described = onKey ? sorter.describe(sorter.active) : null;
  const hint = described ? `Sorted by ${described.label.toLowerCase()}, ${described.direction}. Click to ${sorter.active.desc === firstDesc(sorter.columns[onKey]) ? "reverse" : `go back to ${sorter.fallback}`}.` : `Sort by ${sorter.columns[primary][0].toLowerCase()}`;
  return /* @__PURE__ */ React9.createElement("th", { className: `dc-sortable ${onKey ? "sorted" : ""} ${className}`.trim(), "aria-sort": dir === "asc" ? "ascending" : dir === "desc" ? "descending" : "none", title }, /* @__PURE__ */ React9.createElement("span", { className: "dc-sort-head" }, /* @__PURE__ */ React9.createElement("button", { type: "button", className: "dc-sort-btn", onClick: () => sorter.cycle(primary), title: hint }, /* @__PURE__ */ React9.createElement("span", { className: "dc-sort-label" }, aside ? /* @__PURE__ */ React9.createElement("span", { className: "dc-sort-aside" }, aside) : children), /* @__PURE__ */ React9.createElement(SortGlyph, { dir })), keys.length > 1 && /* @__PURE__ */ React9.createElement("button", { type: "button", ref: button, className: "dc-sort-more", "aria-haspopup": "menu", "aria-expanded": open, "aria-label": `More ways to sort ${typeof children === "string" ? children : "this column"}`, title: "Sort by\u2026", onClick: () => setOpen((o) => !o) }, /* @__PURE__ */ React9.createElement(Stroke, { d: DOWN, size: 10 }))), open && /* @__PURE__ */ React9.createElement(SortMenu, { anchor: button, sorter, keys, onClose: () => setOpen(false) }));
}
function SortMenu({ anchor, sorter, keys, onClose }) {
  const menu = useRef2(null);
  const [at, setAt] = useState2(null);
  useLayoutEffect(() => {
    const place = () => {
      const r = anchor.current?.getBoundingClientRect();
      if (r) setAt({ top: r.bottom + 6, left: Math.max(8, Math.min(r.right - 232, window.innerWidth - 240)) });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [anchor]);
  useEffect2(() => {
    const onDown = (event) => {
      if (!menu.current?.contains(event.target) && !anchor.current?.contains(event.target)) onClose();
    };
    const onKey = (event) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
        anchor.current?.focus();
      }
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const items = [...menu.current?.querySelectorAll("button") || []];
        const i = items.indexOf(document.activeElement);
        items[(i + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length]?.focus();
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    menu.current?.querySelector('button[aria-checked="true"], button')?.focus();
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [anchor, onClose]);
  const pick = (sort) => {
    sorter.set(sort);
    onClose();
  };
  return createPortal2(/* @__PURE__ */ React9.createElement("div", { ref: menu, className: "dc-sort-menu", role: "menu", style: at ? { top: at.top, left: at.left } : { visibility: "hidden" } }, /* @__PURE__ */ React9.createElement("div", { className: "dc-sort-menu-head" }, "Sort by"), keys.map((key) => {
    const [label, kind] = sorter.columns[key];
    return /* @__PURE__ */ React9.createElement("div", { key, className: "dc-sort-menu-row" }, /* @__PURE__ */ React9.createElement("span", { className: "dc-sort-menu-label" }, label), /* @__PURE__ */ React9.createElement("span", { className: "dc-sort-menu-ways" }, sortWays(kind).map(([desc, text2]) => {
      const on = sorter.active?.key === key && sorter.active.desc === desc;
      return /* @__PURE__ */ React9.createElement("button", { type: "button", key: String(desc), role: "menuitemradio", "aria-checked": on, className: on ? "on" : "", onClick: () => pick(on ? null : { key, desc }) }, /* @__PURE__ */ React9.createElement(SortGlyph, { dir: desc ? "desc" : "asc" }), text2);
    })));
  }), sorter.active && /* @__PURE__ */ React9.createElement("button", { type: "button", className: "dc-sort-menu-reset", onClick: () => pick(null) }, /* @__PURE__ */ React9.createElement(Stroke, { d: CLOSE, size: 11 }), "Back to ", sorter.fallback)), document.body);
}
function SortPill({ sorter }) {
  if (!sorter?.active) return null;
  const { label, direction } = sorter.describe(sorter.active);
  return /* @__PURE__ */ React9.createElement("span", { className: "dc-sort-pill", role: "status" }, /* @__PURE__ */ React9.createElement(SortGlyph, { dir: sorter.active.desc ? "desc" : "asc" }), /* @__PURE__ */ React9.createElement("span", null, "Sorted by ", /* @__PURE__ */ React9.createElement("b", null, label), " \xB7 ", direction), /* @__PURE__ */ React9.createElement("button", { type: "button", onClick: () => sorter.set(null), "aria-label": `Stop sorting; back to ${sorter.fallback}`, title: `Back to ${sorter.fallback}` }, /* @__PURE__ */ React9.createElement(Stroke, { d: CLOSE, size: 11 })));
}
function SortPicker({ sorter, always = false, boxClassName = "" }) {
  const options = [{ value: "", label: `Default \xB7 ${sorter.fallback}` }, ...Object.entries(sorter.columns).flatMap(([key, [label, kind]]) => sortWays(kind).map(([desc, text2]) => ({ value: sortParam({ key, desc }), label: `${label} \xB7 ${text2}` })))];
  return /* @__PURE__ */ React9.createElement("div", { className: `dc-sort-picker ${always ? "always" : ""}`.trim() }, /* @__PURE__ */ React9.createElement("span", null, "Sort"), /* @__PURE__ */ React9.createElement("div", { className: `dc-sort-picker-box ${boxClassName}`.trim() }, /* @__PURE__ */ React9.createElement(Combobox, { ariaLabel: "Sort by", options, value: sorter.param, onSelect: (option) => sorter.set(parseSort(option.value)), onClear: () => sorter.set(null) })));
}

// ui/Pager.jsx
var PAGE_SIZES = [25, 50, 100, 200];
function usePageSize({ storageKey = "dyson-page-size", sizes = PAGE_SIZES, fallback = 50 } = {}) {
  const [size, setSize] = useState3(() => {
    try {
      const n = Number(localStorage.getItem(storageKey));
      return sizes.includes(n) ? n : fallback;
    } catch {
      return fallback;
    }
  });
  const choose = (n) => {
    setSize(n);
    try {
      localStorage.setItem(storageKey, String(n));
    } catch {
    }
  };
  return [size, choose];
}
var Chevron = ({ d }) => /* @__PURE__ */ React10.createElement("svg", { width: "13", height: "13", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "2", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true" }, /* @__PURE__ */ React10.createElement("path", { d }));
var FIRST = "M11 17l-5-5 5-5M18 17l-5-5 5-5";
var PREV = "M15 18l-6-6 6-6";
var NEXT = "M9 18l6-6-6-6";
var LAST = "M13 17l5-5-5-5M6 17l5-5-5-5";
function Pager({ page, pages = null, from, shown, total, size, onPage, onSize, sizes = PAGE_SIZES, busy = false, noun = ["row", "rows"], note = null, sorter = null, className = "" }) {
  const [draft, setDraft] = useState3(null);
  const last = pages ?? null;
  const canPrev = page > 1;
  const canNext = last == null ? shown >= size : page < last;
  const go = (n) => {
    const target = Math.max(1, last == null ? n : Math.min(last, n));
    if (target !== page) onPage(target);
  };
  function jump(event) {
    event.preventDefault();
    const n = Number(draft);
    setDraft(null);
    if (Number.isFinite(n) && n >= 1) go(Math.round(n));
  }
  const range = shown ? `${formatCount(from + 1)}\u2013${formatCount(from + shown)}` : "0";
  const step = (label, d, onClick, disabled, extra = {}) => /* @__PURE__ */ React10.createElement("button", { type: "button", className: "dc-pager-btn", onClick, disabled, title: label, "aria-label": label.replace(/\s+[←→]$/, ""), ...extra }, /* @__PURE__ */ React10.createElement(Chevron, { d }));
  return /* @__PURE__ */ React10.createElement("nav", { className: `dc-pager ${className}`.trim(), "aria-label": "Pages" }, /* @__PURE__ */ React10.createElement("span", { className: "dc-pager-range" }, range, " of ", formatCount(total), " ", total === 1 ? noun[0] : noun[1], note && /* @__PURE__ */ React10.createElement("span", { className: "dc-pager-note" }, " \xB7 ", note), busy && /* @__PURE__ */ React10.createElement("span", { className: "dc-pager-note" }, " \xB7 loading\u2026")), sorter && /* @__PURE__ */ React10.createElement(SortPill, { sorter }), (canPrev || canNext) && /* @__PURE__ */ React10.createElement("span", { className: "dc-pager-steps" }, step("First page", FIRST, () => go(1), !canPrev), step("Previous page  \u2190", PREV, () => go(page - 1), !canPrev, { "data-pager": "prev" }), /* @__PURE__ */ React10.createElement("form", { className: "dc-pager-at", onSubmit: jump }, /* @__PURE__ */ React10.createElement("span", null, "Page"), /* @__PURE__ */ React10.createElement("input", { type: "number", min: "1", max: last ?? void 0, value: draft ?? page, onChange: (e) => setDraft(e.target.value), onBlur: () => setDraft(null), "aria-label": "Page number" }), last != null && /* @__PURE__ */ React10.createElement("span", null, "of ", formatCount(last))), step("Next page  \u2192", NEXT, () => go(page + 1), !canNext, { "data-pager": "next" }), last != null && step("Last page", LAST, () => go(last), !canNext)), onSize && /* @__PURE__ */ React10.createElement("span", { className: "dc-pager-size" }, /* @__PURE__ */ React10.createElement("span", null, "Rows"), /* @__PURE__ */ React10.createElement(SegmentedControl, { label: "Rows per page", value: size, onChange: onSize, options: sizes.map((n) => [n, String(n)]) })));
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
import React11, { useState as useState4, useEffect as useEffect3, useMemo } from "react";
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
function Chevron2() {
  return /* @__PURE__ */ React11.createElement(
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
    /* @__PURE__ */ React11.createElement("path", { d: "m5 4 4 4-4 4" })
  );
}
function SecurityReportView({ reportPath, fallback, load = loadFromMindRoute }) {
  const [doc, setDoc] = useState4(null);
  const [unavailable, setUnavailable] = useState4(false);
  const [view, setView] = useState4("cards");
  const [sevFilter, setSevFilter] = useState4("");
  const [classFilter, setClassFilter] = useState4("");
  const [flagFilter, setFlagFilter] = useState4("all");
  const [query, setQuery] = useState4("");
  const [expanded, setExpanded] = useState4(() => /* @__PURE__ */ new Set());
  const [copiedReport, setCopiedReport] = useState4(false);
  useEffect3(() => {
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
  return /* @__PURE__ */ React11.createElement("div", { className: "secrep" }, /* @__PURE__ */ React11.createElement("div", { className: "secrep-inner" }, /* @__PURE__ */ React11.createElement("div", { className: "secrep-summary" }, /* @__PURE__ */ React11.createElement("span", { className: "secrep-summary-count" }, doc.findings.length, " ", doc.findings.length === 1 ? "finding" : "findings"), SEVERITY_LABELS.map((sev) => {
    const n = summary[sev] || 0;
    if (!n) return null;
    return /* @__PURE__ */ React11.createElement("span", { key: sev, className: "secrep-sev" }, /* @__PURE__ */ React11.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), /* @__PURE__ */ React11.createElement("span", { className: "secrep-sev-n" }, n), /* @__PURE__ */ React11.createElement("span", { className: "secrep-sev-label" }, sev));
  }), /* @__PURE__ */ React11.createElement("span", null, summary.new || 0, " new \xB7 ", summary.recurring || 0, " recurring"), /* @__PURE__ */ React11.createElement("span", { className: "secrep-mono secrep-summary-path", title: doc.target && doc.target.repo_path }, doc.target && doc.target.repo_path), doc.model && doc.model.model && /* @__PURE__ */ React11.createElement("span", { className: "secrep-mono" }, doc.model.model)), /* @__PURE__ */ React11.createElement("div", { className: "secrep-controls" }, view === "cards" && /* @__PURE__ */ React11.createElement(React11.Fragment, null, /* @__PURE__ */ React11.createElement("select", { className: "secrep-select", value: sevFilter, onChange: (e) => setSevFilter(e.target.value) }, /* @__PURE__ */ React11.createElement("option", { value: "" }, "all severities"), SEVERITY_LABELS.map((s) => /* @__PURE__ */ React11.createElement("option", { key: s, value: s }, s))), classes.length > 0 && /* @__PURE__ */ React11.createElement("select", { className: "secrep-select", value: classFilter, onChange: (e) => setClassFilter(e.target.value) }, /* @__PURE__ */ React11.createElement("option", { value: "" }, "all classes"), classes.map((c) => /* @__PURE__ */ React11.createElement("option", { key: c, value: c }, c))), /* @__PURE__ */ React11.createElement("select", { className: "secrep-select", value: flagFilter, onChange: (e) => setFlagFilter(e.target.value) }, /* @__PURE__ */ React11.createElement("option", { value: "all" }, "new + recurring"), /* @__PURE__ */ React11.createElement("option", { value: "new" }, "new"), /* @__PURE__ */ React11.createElement("option", { value: "recurring" }, "recurring")), /* @__PURE__ */ React11.createElement(
    "input",
    {
      className: "secrep-search",
      type: "search",
      placeholder: "search",
      value: query,
      onChange: (e) => setQuery(e.target.value)
    }
  )), /* @__PURE__ */ React11.createElement("button", { className: "secrep-btn", onClick: copyReport }, copiedReport ? "copied" : "copy report"), /* @__PURE__ */ React11.createElement("span", { className: "secrep-spacer" }), /* @__PURE__ */ React11.createElement(
    "button",
    {
      className: "secrep-btn secrep-toggle",
      "data-on": view === "cards",
      onClick: () => setView("cards")
    },
    "rendered"
  ), /* @__PURE__ */ React11.createElement(
    "button",
    {
      className: "secrep-btn secrep-toggle",
      "data-on": view === "report",
      onClick: () => setView("report")
    },
    "report"
  ), /* @__PURE__ */ React11.createElement(
    "button",
    {
      className: "secrep-btn secrep-toggle",
      "data-on": view === "json",
      onClick: () => setView("json")
    },
    "json"
  )), view === "json" ? /* @__PURE__ */ React11.createElement("div", { className: "secrep-json-wrap" }, /* @__PURE__ */ React11.createElement("pre", { className: "secrep-json" }, JSON.stringify(doc, null, 2))) : view === "report" ? (
    // The full written report — the artefact's markdown body, the
    // same node the fetch-failure fallback renders.  Lets the reader
    // see the narrative + evidence even when the structured findings
    // list is empty (e.g. a pentest that confirmed no findings).
    /* @__PURE__ */ React11.createElement("div", { className: "secrep-report" }, fallback)
  ) : /* @__PURE__ */ React11.createElement(React11.Fragment, null, visible.length === 0 && /* @__PURE__ */ React11.createElement("div", { className: "secrep-empty" }, doc.findings.length === 0 ? "No confirmed findings." : "No findings match."), groups.map(({ sev, items }) => /* @__PURE__ */ React11.createElement("div", { key: sev, className: "secrep-group" }, /* @__PURE__ */ React11.createElement("div", { className: "secrep-eyebrow" }, /* @__PURE__ */ React11.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), sev, " \xB7 ", items.length), items.map(({ f, idx }) => /* @__PURE__ */ React11.createElement(
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
  const [copied, setCopied] = useState4(false);
  const copyFinding = async (event) => {
    event.stopPropagation();
    if (await copyToClipboard(JSON.stringify(f, null, 2))) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  };
  return /* @__PURE__ */ React11.createElement("div", { className: "secrep-card" }, /* @__PURE__ */ React11.createElement("div", { className: "secrep-card-head", onClick: onToggle }, /* @__PURE__ */ React11.createElement("span", { className: "secrep-caret", style: { transform: open ? "rotate(90deg)" : "none" } }, /* @__PURE__ */ React11.createElement(Chevron2, null)), /* @__PURE__ */ React11.createElement("span", { className: "secrep-dot", style: { background: SEVERITY_COLOR[sev] } }), /* @__PURE__ */ React11.createElement("span", { className: "secrep-card-title" }, f.title || f.run_finding_id || f.id), f.key && /* @__PURE__ */ React11.createElement("span", { className: "secrep-chip" }, f.key), f.recurring && /* @__PURE__ */ React11.createElement("span", { className: "secrep-chip" }, "recurring x", f.occurrences), /* @__PURE__ */ React11.createElement("button", { className: "secrep-btn secrep-copy-finding", type: "button", onClick: copyFinding }, copied ? "copied" : "copy")), open && /* @__PURE__ */ React11.createElement("div", { className: "secrep-card-body" }, row("class", f.vulnerability_class && /* @__PURE__ */ React11.createElement("span", { className: "secrep-chip" }, f.vulnerability_class)), row("boundary", text(f.trust_boundary)), row("flow", (f.entry_point || f.sink_or_decision) && /* @__PURE__ */ React11.createElement("span", { className: "secrep-mono" }, f.entry_point, f.entry_point && f.sink_or_decision ? " \u2192 " : "", f.sink_or_decision)), row("root cause", text(f.root_cause)), row("reachability", text(f.reachability)), row("impact", text(f.tenant_or_instance_impact)), row("rationale", text(f.severity_rationale)), row("fix", text(f.fix_recommendation)), row("paths", Array.isArray(f.affected_paths) && f.affected_paths.length > 0 && /* @__PURE__ */ React11.createElement("span", { className: "secrep-paths" }, f.affected_paths.map((p, i) => /* @__PURE__ */ React11.createElement("span", { key: i, className: "secrep-chip" }, p)))), row("evidence", Array.isArray(f.evidence) && f.evidence.length > 0 && /* @__PURE__ */ React11.createElement("pre", { className: "secrep-pre secrep-pre-wrap" }, f.evidence.join("\n"))), row("patch", !!(f.suggested_patch && f.suggested_patch.trim()) && /* @__PURE__ */ React11.createElement("pre", { className: "secrep-pre" }, f.suggested_patch))));
}
function text(v) {
  return v && String(v).trim() ? /* @__PURE__ */ React11.createElement("span", null, v) : null;
}
function row(label, value) {
  if (!value) return null;
  return /* @__PURE__ */ React11.createElement(React11.Fragment, { key: label }, /* @__PURE__ */ React11.createElement("span", { className: "secrep-label" }, label), /* @__PURE__ */ React11.createElement("span", { className: "secrep-value" }, value));
}

// ui/EvalResultsView.jsx
import React12, { useState as useState5 } from "react";
var TERMINAL = /* @__PURE__ */ new Set(["passed", "failed", "error"]);
function pct(x) {
  if (!Number.isFinite(x)) return "\u2014";
  return `${(x * 100).toFixed(1)}%`;
}
function elapsed(ms) {
  if (ms == null || ms === "") return "\u2014";
  const value = Number(ms);
  if (!Number.isFinite(value) || value < 0) return "\u2014";
  if (value < 1e3) return `${Math.round(value)}ms`;
  const seconds = value / 1e3;
  if (seconds < 60) return `${Number(seconds.toFixed(seconds < 10 ? 2 : 1))}s`;
  const wholeSeconds = Math.round(seconds);
  const minutes = Math.floor(wholeSeconds / 60);
  const remainder = wholeSeconds % 60;
  if (minutes < 60) return `${minutes}m ${remainder}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}
function caseElapsedMs(c) {
  if (c?.duration_ms != null) return Number(c.duration_ms);
  const started = Number(c?.started_at);
  const finished = Number(c?.finished_at);
  return Number.isFinite(started) && Number.isFinite(finished) ? Math.max(0, finished - started) * 1e3 : null;
}
function targetElapsedMs(target, cases) {
  if (target?.duration_ms != null) return Number(target.duration_ms);
  const key = target?.target?.key;
  const durations = (cases || []).filter((c) => c?.target?.key === key).map(caseElapsedMs).filter((value) => Number.isFinite(value));
  return durations.length ? durations.reduce((sum, value) => sum + value, 0) : null;
}
function statusTone(status) {
  if (status === "passed") return "ok";
  if (status === "failed") return "fail";
  if (status === "error") return "error";
  return "pending";
}
function Chevron3() {
  return /* @__PURE__ */ React12.createElement(
    "svg",
    {
      width: 10,
      height: 10,
      viewBox: "0 0 16 16",
      fill: "none",
      stroke: "currentColor",
      strokeWidth: 1.5,
      strokeLinecap: "round",
      strokeLinejoin: "round",
      "aria-hidden": "true"
    },
    /* @__PURE__ */ React12.createElement("path", { d: "m5 4 4 4-4 4" })
  );
}
function Meter({ label, value, tone = "accent", title }) {
  const clamped = Math.max(0, Math.min(1, Number(value) || 0));
  return /* @__PURE__ */ React12.createElement("div", { className: "evalrep-meter", title }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-meter-label" }, label), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-meter-track" }, /* @__PURE__ */ React12.createElement(
    "span",
    {
      className: `evalrep-meter-fill evalrep-fill-${tone}`,
      style: { width: `${clamped * 100}%` }
    }
  )), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-meter-value" }, pct(clamped)));
}
function TrendSparkline({ history, threshold }) {
  const points = (history || []).map((r) => ({
    score: Number(r?.score),
    passed: r?.status === "passed"
  })).filter((p) => Number.isFinite(p.score));
  if (points.length < 2) return null;
  const W = 240;
  const H = 44;
  const pad = 4;
  const n = points.length;
  const x = (i) => pad + i * (W - 2 * pad) / (n - 1);
  const y = (s) => H - pad - s * (H - 2 * pad);
  const path = points.map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(p.score).toFixed(1)}`).join(" ");
  const thr = Number.isFinite(threshold) ? y(threshold) : null;
  return /* @__PURE__ */ React12.createElement("div", { className: "evalrep-trend" }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-trend-label" }, "score trend \xB7 ", n, " runs"), /* @__PURE__ */ React12.createElement(
    "svg",
    {
      className: "evalrep-trend-svg",
      viewBox: `0 0 ${W} ${H}`,
      width: W,
      height: H,
      preserveAspectRatio: "none",
      role: "img",
      "aria-label": "score trend across runs"
    },
    thr != null && /* @__PURE__ */ React12.createElement(
      "line",
      {
        x1: 0,
        x2: W,
        y1: thr,
        y2: thr,
        className: "evalrep-trend-threshold",
        strokeDasharray: "3 3"
      }
    ),
    /* @__PURE__ */ React12.createElement("path", { d: path, className: "evalrep-trend-line", fill: "none" }),
    points.map((p, i) => /* @__PURE__ */ React12.createElement(
      "circle",
      {
        key: i,
        cx: x(i),
        cy: y(p.score),
        r: 2.6,
        className: `evalrep-trend-dot evalrep-dot-${p.passed ? "ok" : "fail"}`
      }
    ))
  ));
}
function CaseRow({ c, dimKeys, open, onToggle, showTiming, showTarget }) {
  const errored = !!c.error;
  const byKey = new Map((c.dimensions || []).map((d) => [d.key, d]));
  return /* @__PURE__ */ React12.createElement(React12.Fragment, null, /* @__PURE__ */ React12.createElement("tr", { className: `evalrep-case-row${errored ? " evalrep-case-errored" : ""}`, onClick: onToggle }, /* @__PURE__ */ React12.createElement("td", { className: "evalrep-case-toggle" }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-caret", style: { transform: open ? "rotate(90deg)" : "none" } }, /* @__PURE__ */ React12.createElement(Chevron3, null))), /* @__PURE__ */ React12.createElement("td", { className: "evalrep-case-id", title: c.case_id }, c.case_id), showTarget ? /* @__PURE__ */ React12.createElement("td", { className: "evalrep-case-target", title: c.target?.label }, c.target?.label || "\u2014") : null, dimKeys.map((k) => {
    const d = byKey.get(k);
    return /* @__PURE__ */ React12.createElement("td", { key: k, className: "evalrep-case-dim" }, d ? /* @__PURE__ */ React12.createElement("span", { className: "evalrep-dimcell", title: `raw ${d.score}` }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-dimbar" }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-dimbar-fill", style: { width: `${Math.max(0, Math.min(1, d.normalized)) * 100}%` } })), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-dimcell-val" }, pct(d.normalized))) : /* @__PURE__ */ React12.createElement("span", { className: "evalrep-muted" }, "\u2014"));
  }), /* @__PURE__ */ React12.createElement("td", { className: "evalrep-case-score" }, errored ? /* @__PURE__ */ React12.createElement("span", { className: "evalrep-muted" }, "\u2014") : pct(c.case_score)), showTiming ? /* @__PURE__ */ React12.createElement("td", { className: "evalrep-case-time" }, elapsed(caseElapsedMs(c))) : null, /* @__PURE__ */ React12.createElement("td", { className: "evalrep-case-state" }, errored ? /* @__PURE__ */ React12.createElement("span", { className: "evalrep-chip evalrep-chip-error" }, "error") : /* @__PURE__ */ React12.createElement("span", { className: "evalrep-chip evalrep-chip-ok" }, "scored"))), open && /* @__PURE__ */ React12.createElement("tr", { className: "evalrep-case-detail-row" }, /* @__PURE__ */ React12.createElement("td", { colSpan: dimKeys.length + 4 + (showTiming ? 1 : 0) + (showTarget ? 1 : 0) }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-case-detail" }, errored && /* @__PURE__ */ React12.createElement("div", { className: "evalrep-error-note" }, c.error), /* @__PURE__ */ React12.createElement(DetailBlock, { label: "input", value: c.input }), /* @__PURE__ */ React12.createElement(DetailBlock, { label: "reference", value: c.reference }), /* @__PURE__ */ React12.createElement(DetailBlock, { label: "candidate output", value: c.output, truncated: c.output_truncated }), (c.dimensions || []).length > 0 && /* @__PURE__ */ React12.createElement("div", { className: "evalrep-rationales" }, c.dimensions.map((d) => /* @__PURE__ */ React12.createElement("div", { key: d.key, className: "evalrep-rationale" }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-rationale-head" }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-rationale-key" }, d.key), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-rationale-score" }, d.score, " \xB7 ", pct(d.normalized))), /* @__PURE__ */ React12.createElement("div", { className: "evalrep-rationale-text" }, d.rationale))))))));
}
function DetailBlock({ label, value, truncated }) {
  if (value == null || value === "") return null;
  return /* @__PURE__ */ React12.createElement("div", { className: "evalrep-detail-block" }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-detail-label" }, label, truncated ? /* @__PURE__ */ React12.createElement("span", { className: "evalrep-muted" }, " \xB7 truncated") : null), /* @__PURE__ */ React12.createElement("pre", { className: "evalrep-pre" }, value));
}
function EvalResultsView({ results, status, history, title, showTiming = true }) {
  const [expanded, setExpanded] = useState5(() => /* @__PURE__ */ new Set());
  const isTerminal = TERMINAL.has(status) || results && (results.error || results.aggregate);
  if (!results || !results.aggregate) {
    if (isTerminal && results && results.error) {
      return /* @__PURE__ */ React12.createElement("div", { className: "evalrep" }, title ? /* @__PURE__ */ React12.createElement("div", { className: "evalrep-title" }, title) : null, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-fatal" }, results.error));
    }
    const s = status || "pending";
    return /* @__PURE__ */ React12.createElement("div", { className: "evalrep" }, title ? /* @__PURE__ */ React12.createElement("div", { className: "evalrep-title" }, title) : null, /* @__PURE__ */ React12.createElement("div", { className: `evalrep-progress evalrep-progress-${statusTone(s)}` }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-spinner", "aria-hidden": "true" }), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-progress-label" }, s === "running" ? "running evaluation\u2026" : "queued\u2026")));
  }
  const agg = results.aggregate;
  const perDim = agg.per_dimension || {};
  const dimKeys = Object.keys(perDim);
  const runStatus = status || (agg.pass ? "passed" : "failed");
  const fatal = results.error;
  const comparisonTargets = Array.isArray(results?.comparison?.targets) ? results.comparison.targets : [];
  const isComparison = comparisonTargets.length > 1;
  const toggle = (idx) => setExpanded((prev) => {
    const next = new Set(prev);
    if (next.has(idx)) next.delete(idx);
    else next.add(idx);
    return next;
  });
  return /* @__PURE__ */ React12.createElement("div", { className: "evalrep" }, title ? /* @__PURE__ */ React12.createElement("div", { className: "evalrep-title" }, title) : null, fatal ? /* @__PURE__ */ React12.createElement("div", { className: "evalrep-fatal" }, fatal) : null, /* @__PURE__ */ React12.createElement("div", { className: `evalrep-verdict evalrep-verdict-${statusTone(runStatus)}` }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-verdict-main" }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-verdict-badge" }, isComparison ? "COMPARE" : runStatus === "passed" ? "PASS" : runStatus === "failed" ? "FAIL" : "ERROR"), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-verdict-score" }, isComparison ? `${comparisonTargets.length} agents` : pct(agg.score)), /* @__PURE__ */ React12.createElement("span", { className: "evalrep-verdict-threshold" }, isComparison ? `${(results.cases || []).length} exact trajectories` : `vs ${pct(agg.pass_threshold)} threshold`)), !isComparison ? /* @__PURE__ */ React12.createElement("div", { className: "evalrep-verdict-gauge" }, /* @__PURE__ */ React12.createElement("span", { className: "evalrep-gauge-track" }, /* @__PURE__ */ React12.createElement(
    "span",
    {
      className: `evalrep-gauge-fill evalrep-fill-${statusTone(runStatus)}`,
      style: { width: `${Math.max(0, Math.min(1, agg.score)) * 100}%` }
    }
  ), /* @__PURE__ */ React12.createElement(
    "span",
    {
      className: "evalrep-gauge-threshold",
      style: { left: `${Math.max(0, Math.min(1, agg.pass_threshold)) * 100}%` }
    }
  ))) : null, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-verdict-meta" }, /* @__PURE__ */ React12.createElement("span", null, agg.cases_total, " case", agg.cases_total === 1 ? "" : "s"), showTiming && results.duration_ms != null ? /* @__PURE__ */ React12.createElement("span", null, elapsed(results.duration_ms), " total") : null, agg.cases_errored > 0 ? /* @__PURE__ */ React12.createElement("span", { className: "evalrep-meta-error" }, agg.cases_errored, " errored") : null, results.judge_model ? /* @__PURE__ */ React12.createElement("span", { className: "evalrep-mono" }, "judge \xB7 ", results.judge_model) : null)), !isComparison ? /* @__PURE__ */ React12.createElement(TrendSparkline, { history, threshold: agg.pass_threshold }) : null, isComparison ? /* @__PURE__ */ React12.createElement("div", { className: "evalrep-targets", "aria-label": "Exact agent comparison" }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-section-label" }, "agents \xB7 exact totals"), /* @__PURE__ */ React12.createElement("div", { className: "evalrep-target-grid" }, comparisonTargets.map((target) => /* @__PURE__ */ React12.createElement("div", { className: "evalrep-target", key: target.target?.key || target.target?.label }, /* @__PURE__ */ React12.createElement("span", { title: target.target?.label }, target.target?.label || "agent"), /* @__PURE__ */ React12.createElement("strong", null, target.aggregate?.cases_total || 0, " test", target.aggregate?.cases_total === 1 ? "" : "s"), /* @__PURE__ */ React12.createElement("small", null, showTiming ? `${elapsed(targetElapsedMs(target, results.cases))} elapsed \xB7 ` : "", target.aggregate?.cases_total || 0, " case", target.aggregate?.cases_total === 1 ? "" : "s", " \xB7 ", target.aggregate?.cases_errored || 0, " error", target.aggregate?.cases_errored === 1 ? "" : "s"))))) : null, !isComparison && dimKeys.length > 0 && /* @__PURE__ */ React12.createElement("div", { className: "evalrep-dims" }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-section-label" }, "per-dimension mean"), dimKeys.map((k) => /* @__PURE__ */ React12.createElement(
    Meter,
    {
      key: k,
      label: k,
      value: perDim[k]?.mean_normalized,
      tone: "accent",
      title: `${k}: ${pct(perDim[k]?.mean_normalized)}`
    }
  ))), /* @__PURE__ */ React12.createElement("div", { className: "evalrep-cases" }, /* @__PURE__ */ React12.createElement("div", { className: "evalrep-section-label" }, isComparison ? "exact trajectories \xB7 no per-case averages" : "cases"), /* @__PURE__ */ React12.createElement("div", { className: "evalrep-table-wrap" }, /* @__PURE__ */ React12.createElement("table", { className: "evalrep-table" }, /* @__PURE__ */ React12.createElement("thead", null, /* @__PURE__ */ React12.createElement("tr", null, /* @__PURE__ */ React12.createElement("th", { "aria-label": "expand" }), /* @__PURE__ */ React12.createElement("th", null, "case"), isComparison ? /* @__PURE__ */ React12.createElement("th", null, "agent") : null, dimKeys.map((k) => /* @__PURE__ */ React12.createElement("th", { key: k, className: "evalrep-th-dim" }, k)), /* @__PURE__ */ React12.createElement("th", null, "score"), showTiming ? /* @__PURE__ */ React12.createElement("th", null, "time") : null, /* @__PURE__ */ React12.createElement("th", null, "state"))), /* @__PURE__ */ React12.createElement("tbody", null, (results.cases || []).map((c, i) => /* @__PURE__ */ React12.createElement(
    CaseRow,
    {
      key: `${c.target?.key || "case"}:${c.case_id || i}`,
      c,
      dimKeys,
      open: expanded.has(i),
      onToggle: () => toggle(i),
      showTiming,
      showTarget: isComparison
    }
  )))))));
}

// ui/EvalReportView.jsx
import React13, { useState as useState6, useEffect as useEffect4, useRef as useRef3, useCallback as useCallback2 } from "react";
var TERMINAL2 = /* @__PURE__ */ new Set(["passed", "failed", "error"]);
var POLL_MS = 2500;
async function loadFromMindRoute2(reportPath) {
  const r = await fetch("/api/mind/file?path=" + encodeURIComponent(reportPath), { credentials: "same-origin" });
  if (!r.ok) throw new Error(String(r.status));
  const payload = await r.json();
  return JSON.parse(payload.content);
}
function normalize(doc) {
  if (!doc || typeof doc !== "object") throw new Error("bad doc");
  const results = doc.results != null ? doc.results : doc.aggregate || doc.cases ? doc : null;
  const status = doc.status || (results && results.error ? "error" : null) || (results && results.aggregate ? results.aggregate.pass ? "passed" : "failed" : "pending");
  return { status, results, history: doc.history };
}
function EvalReportView({ reportPath, fallback, load = loadFromMindRoute2, history }) {
  const [state, setState] = useState6(null);
  const [unavailable, setUnavailable] = useState6(false);
  const timer = useRef3(null);
  const clearTimer = () => {
    if (timer.current) {
      clearTimeout(timer.current);
      timer.current = null;
    }
  };
  const fetchOnce = useCallback2(async (cancelledRef) => {
    try {
      const doc = await Promise.resolve(load(reportPath));
      const norm = normalize(doc);
      if (cancelledRef.cancelled) return;
      setState(norm);
      if (!TERMINAL2.has(norm.status)) {
        clearTimer();
        timer.current = setTimeout(() => fetchOnce(cancelledRef), POLL_MS);
      }
    } catch {
      if (!cancelledRef.cancelled) setUnavailable(true);
    }
  }, [load, reportPath]);
  useEffect4(() => {
    const cancelledRef = { cancelled: false };
    setState(null);
    setUnavailable(false);
    fetchOnce(cancelledRef);
    return () => {
      cancelledRef.cancelled = true;
      clearTimer();
    };
  }, [fetchOnce]);
  if (unavailable) return fallback;
  if (!state) {
    return /* @__PURE__ */ React13.createElement("div", { className: "evalrep" }, /* @__PURE__ */ React13.createElement("div", { className: "evalrep-progress evalrep-progress-pending" }, /* @__PURE__ */ React13.createElement("span", { className: "evalrep-spinner", "aria-hidden": "true" }), /* @__PURE__ */ React13.createElement("span", { className: "evalrep-progress-label" }, "loading eval\u2026")));
  }
  return /* @__PURE__ */ React13.createElement(
    EvalResultsView,
    {
      results: state.results,
      status: state.status,
      history: history || state.history
    }
  );
}

// ui/SubscriptionConnectModal.jsx
import React14 from "react";
var SUBSCRIPTION_CONNECT_PROVIDERS = Object.freeze({
  codex: Object.freeze({
    id: "codex",
    badge: "Codex",
    service: "ChatGPT",
    tone: "codex",
    flow: "device"
  }),
  claude: Object.freeze({
    id: "claude",
    badge: "Claude",
    service: "Claude",
    tone: "claude",
    flow: "code"
  }),
  // DeepSeek's account sign-in returns the browser to a loopback address
  // that does not load; the code travels in that address.
  deepseek: Object.freeze({
    id: "deepseek",
    badge: "DeepSeek",
    service: "DeepSeek",
    tone: "deepseek",
    flow: "code",
    codeLabel: "Paste the address you land on",
    codeHint: "Copy the full 127.0.0.1 address from the browser after approval.",
    codePlaceholder: "http://127.0.0.1:\u2026/oauth/callback?code=\u2026"
  })
});
function SubscriptionConnectModal({
  initial,
  subscription,
  getStatus,
  completeAuth,
  onConnected,
  onClose,
  subjectLabel = "this agent"
}) {
  const [auth, setAuth] = React14.useState(initial || {});
  const [copied, setCopied] = React14.useState(false);
  const [code, setCode] = React14.useState("");
  const [submitting, setSubmitting] = React14.useState(false);
  const getStatusRef = React14.useRef(getStatus);
  const completeAuthRef = React14.useRef(completeAuth);
  const onConnectedRef = React14.useRef(onConnected);
  getStatusRef.current = getStatus;
  completeAuthRef.current = completeAuth;
  onConnectedRef.current = onConnected;
  React14.useEffect(() => {
    let alive = true;
    let completed = false;
    const poll = async () => {
      try {
        const next = await getStatusRef.current?.();
        if (!alive || !next) return;
        setAuth(next);
        if (next.connected && !completed) {
          completed = true;
          try {
            await onConnectedRef.current?.();
          } catch {
            completed = false;
            if (alive) setAuth({ ...next, error: "Connected, but setup did not finish. Retrying\u2026" });
          }
        }
      } catch {
      }
    };
    const timer = setInterval(poll, 1e3);
    poll();
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [subscription.id]);
  const copyCode = async () => {
    if (!auth.user_code || !navigator.clipboard?.writeText) return;
    await navigator.clipboard.writeText(auth.user_code);
    setCopied(true);
  };
  const complete = async (event) => {
    event.preventDefault();
    if (!code.trim() || typeof completeAuthRef.current !== "function") return;
    setSubmitting(true);
    try {
      const next = await completeAuthRef.current(code.trim());
      setAuth(next || {});
      if (next?.connected) await onConnectedRef.current?.();
    } catch {
      setAuth((previous) => ({
        ...previous,
        error: "That authorization code could not be accepted. Check the full code and try again."
      }));
    } finally {
      setSubmitting(false);
    }
  };
  const signInUrl = auth.auth_url || auth.verification_uri;
  const isFailed = auth.state === "failed" || auth.state === "unavailable";
  return /* @__PURE__ */ React14.createElement(
    Modal,
    {
      className: `modal subscription-connect subscription-connect-${subscription.tone}`,
      scrimClassName: "modal-scrim subscription-connect-scrim",
      label: `Connect ${subscription.service} subscription`,
      onClose
    },
    /* @__PURE__ */ React14.createElement("div", { className: "subscription-connect-head" }, /* @__PURE__ */ React14.createElement("div", { className: "subscription-connect-brand" }, /* @__PURE__ */ React14.createElement("span", { className: `subscription-backend-badge subscription-backend-badge-${subscription.tone}` }, /* @__PURE__ */ React14.createElement("span", { className: "subscription-backend-mark", "aria-hidden": "true" }), subscription.badge), /* @__PURE__ */ React14.createElement("span", null, "Subscription access")), /* @__PURE__ */ React14.createElement("button", { type: "button", className: "subscription-connect-close", onClick: onClose, "aria-label": "Close" }, "\xD7")),
    /* @__PURE__ */ React14.createElement("div", { className: "subscription-connect-body" }, /* @__PURE__ */ React14.createElement("div", { className: "subscription-connect-kicker" }, "Native provider"), /* @__PURE__ */ React14.createElement("h2", null, "Connect ", subscription.service), /* @__PURE__ */ React14.createElement("p", { className: "subscription-connect-lede" }, "Sign in once, then use your ", subscription.service, " plan directly from ", subjectLabel, "."), /* @__PURE__ */ React14.createElement("div", { className: "subscription-privacy" }, /* @__PURE__ */ React14.createElement("span", { className: "subscription-privacy-mark", "aria-hidden": "true" }, "\u2713"), /* @__PURE__ */ React14.createElement("div", null, /* @__PURE__ */ React14.createElement("strong", null, "Credentials stay in Swarm."), /* @__PURE__ */ React14.createElement("span", null, subjectLabel, " receives only an instance-bound proxy token."))), signInUrl ? /* @__PURE__ */ React14.createElement("div", { className: "subscription-connect-flow" }, /* @__PURE__ */ React14.createElement("div", { className: "subscription-step" }, /* @__PURE__ */ React14.createElement("span", { className: "subscription-step-number" }, "1"), /* @__PURE__ */ React14.createElement("div", null, /* @__PURE__ */ React14.createElement("strong", null, "Open ", subscription.service), /* @__PURE__ */ React14.createElement("span", null, "Approve access in a secure provider window.")), /* @__PURE__ */ React14.createElement("a", { className: "subscription-connect-action subscription-connect-primary", href: signInUrl, target: "_blank", rel: "noreferrer" }, "Open sign-in ", /* @__PURE__ */ React14.createElement("span", { "aria-hidden": "true" }, "\u2197"))), subscription.flow === "device" && auth.user_code ? /* @__PURE__ */ React14.createElement("div", { className: "subscription-step" }, /* @__PURE__ */ React14.createElement("span", { className: "subscription-step-number" }, "2"), /* @__PURE__ */ React14.createElement("div", null, /* @__PURE__ */ React14.createElement("strong", null, "Enter this code"), /* @__PURE__ */ React14.createElement("span", null, "The window will ask for it.")), /* @__PURE__ */ React14.createElement("button", { type: "button", className: "subscription-device-code", onClick: copyCode, "aria-label": "Copy device code" }, /* @__PURE__ */ React14.createElement("span", { className: "subscription-code-value" }, auth.user_code), /* @__PURE__ */ React14.createElement("span", null, copied ? "Copied" : "Copy"))) : null, subscription.flow === "code" ? /* @__PURE__ */ React14.createElement("form", { className: "subscription-step subscription-code-step", onSubmit: complete }, /* @__PURE__ */ React14.createElement("span", { className: "subscription-step-number" }, "2"), /* @__PURE__ */ React14.createElement("label", null, /* @__PURE__ */ React14.createElement("strong", null, subscription.codeLabel || "Paste the returned code"), /* @__PURE__ */ React14.createElement("span", null, subscription.codeHint || `Copy the complete code from ${subscription.service} after approval.`), /* @__PURE__ */ React14.createElement(
      "input",
      {
        value: code,
        onChange: (event) => setCode(event.target.value),
        autoComplete: "off",
        spellCheck: "false",
        placeholder: subscription.codePlaceholder || "Authorization code"
      }
    )), /* @__PURE__ */ React14.createElement("button", { type: "submit", className: "subscription-connect-action subscription-connect-primary", disabled: !code.trim() || submitting }, submitting ? "Connecting\u2026" : "Connect")) : null) : isFailed ? /* @__PURE__ */ React14.createElement("p", { className: "subscription-connect-error" }, auth.error || `${subscription.service} sign-in could not start.`) : /* @__PURE__ */ React14.createElement("div", { className: "subscription-starting" }, /* @__PURE__ */ React14.createElement("span", null), "Preparing secure sign-in\u2026"), auth.error && !isFailed ? /* @__PURE__ */ React14.createElement("p", { className: "subscription-connect-error" }, auth.error) : null)
  );
}

// ui/GatewayPrimitives.jsx
import React15 from "react";
function PageHeader({ eyebrow, title, description, actions, children, className = "" }) {
  return /* @__PURE__ */ React15.createElement("header", { className: `gateway-page-header ${className}`.trim() }, /* @__PURE__ */ React15.createElement("div", { className: "gateway-page-heading" }, eyebrow ? /* @__PURE__ */ React15.createElement("div", { className: "gateway-page-eyebrow" }, eyebrow) : null, /* @__PURE__ */ React15.createElement("h1", null, title), description ? /* @__PURE__ */ React15.createElement("p", null, description) : null), actions ? /* @__PURE__ */ React15.createElement("div", { className: "gateway-page-actions" }, actions) : null, children);
}
function MetricGrid({ children, className = "" }) {
  return /* @__PURE__ */ React15.createElement("div", { className: `gateway-metric-grid ${className}`.trim() }, children);
}
function MetricCard({ label, value, detail, tone = "neutral", trend, className = "" }) {
  return /* @__PURE__ */ React15.createElement("article", { className: `gateway-metric gateway-tone-${tone} ${className}`.trim() }, /* @__PURE__ */ React15.createElement("div", { className: "gateway-metric-label" }, label), /* @__PURE__ */ React15.createElement("div", { className: "gateway-metric-row" }, /* @__PURE__ */ React15.createElement("strong", null, value), trend ? /* @__PURE__ */ React15.createElement("span", { className: "gateway-metric-trend" }, trend) : null), detail ? /* @__PURE__ */ React15.createElement("div", { className: "gateway-metric-detail" }, detail) : null);
}
function StatusBadge({ children, tone = "neutral", pulse = false, className = "" }) {
  return /* @__PURE__ */ React15.createElement("span", { className: `gateway-status gateway-tone-${tone} ${pulse ? "gateway-status-pulse" : ""} ${className}`.trim() }, /* @__PURE__ */ React15.createElement("span", { className: "gateway-status-dot", "aria-hidden": "true" }), children);
}
function Toolbar({ children, className = "", "aria-label": ariaLabel = "Filters" }) {
  return /* @__PURE__ */ React15.createElement("div", { className: `gateway-toolbar ${className}`.trim(), role: "search", "aria-label": ariaLabel }, children);
}
function DataTable({ columns, rows, rowKey = "id", empty = null, onRowClick, className = "" }) {
  if (!rows.length && empty) return empty;
  return /* @__PURE__ */ React15.createElement("div", { className: `gateway-table-wrap ${className}`.trim() }, /* @__PURE__ */ React15.createElement("table", { className: "gateway-table" }, /* @__PURE__ */ React15.createElement("thead", null, /* @__PURE__ */ React15.createElement("tr", null, columns.map((column) => /* @__PURE__ */ React15.createElement("th", { key: column.key }, column.label)))), /* @__PURE__ */ React15.createElement("tbody", null, rows.map((row2, index) => {
    const key = typeof rowKey === "function" ? rowKey(row2, index) : row2[rowKey] ?? index;
    return /* @__PURE__ */ React15.createElement("tr", { key, onClick: onRowClick ? () => onRowClick(row2) : void 0, className: onRowClick ? "gateway-row-action" : "" }, columns.map((column) => /* @__PURE__ */ React15.createElement("td", { key: column.key, "data-label": column.label }, column.render ? column.render(row2[column.key], row2) : row2[column.key])));
  }))));
}
function DetailList({ items, className = "" }) {
  return /* @__PURE__ */ React15.createElement("dl", { className: `gateway-detail-list ${className}`.trim() }, items.map((item) => /* @__PURE__ */ React15.createElement("div", { className: "gateway-detail-row", key: item.label }, /* @__PURE__ */ React15.createElement("dt", null, item.label), /* @__PURE__ */ React15.createElement("dd", null, item.value ?? "\u2014"))));
}
function JsonCodeBlock({ value, label = "JSON", copyLabel = "Copy", className = "" }) {
  const [state, setState] = React15.useState("");
  const code = React15.useRef(null);
  const text2 = typeof value === "string" ? value : JSON.stringify(value, null, 2);
  async function copy() {
    const result = await copyText(text2, code.current);
    setState(result);
    if (result !== "selected") window.setTimeout(() => setState(""), 1500);
  }
  const shown = state === "copied" ? "Copied" : state === "selected" ? "Selected \xB7 press \u2318C" : state === "failed" ? "Copy failed" : copyLabel;
  return /* @__PURE__ */ React15.createElement("section", { className: `gateway-code ${className}`.trim() }, /* @__PURE__ */ React15.createElement("div", { className: "gateway-code-head" }, /* @__PURE__ */ React15.createElement("span", null, label), /* @__PURE__ */ React15.createElement("button", { type: "button", onClick: copy }, shown)), /* @__PURE__ */ React15.createElement("pre", null, /* @__PURE__ */ React15.createElement("code", { ref: code }, text2)));
}
export {
  Combobox,
  ComputerMark,
  ConfirmModal,
  DEFAULT_MARK_VARIANT,
  DYSON_BLUE,
  DataTable,
  DetailList,
  Glyph as DysonGlyph,
  DysonMark,
  EmptyState,
  EvalReportView,
  EvalResultsView,
  GlitchMark,
  JsonCodeBlock,
  MARK_VARIANTS,
  MARK_VARIANT_LABELS,
  MARK_VARIANT_NAMES,
  MetricCard,
  MetricGrid,
  Modal,
  PAGE_SIZES,
  PageHeader,
  Pager,
  SORT_DIRECTION,
  SORT_NATURAL,
  SUBSCRIPTION_CONNECT_PROVIDERS,
  SecurityReportView,
  SegmentedControl,
  SortGlyph,
  SortPicker,
  SortPill,
  SortTh,
  StatusBadge,
  SubscriptionConnectModal,
  MODES as THEME_MODES,
  Toolbar,
  base64url,
  copyFetched,
  copyText,
  copyToClipboard,
  createBuildWatcher,
  createStore,
  createThemeController,
  createUseAppState,
  deepFreeze,
  downloadBlob,
  firstDesc,
  formatBalance,
  formatBytes,
  formatCount,
  formatDuration,
  formatTokens,
  formatUsd,
  parseSort,
  pkceChallenge,
  randomString,
  setBrandMarkVariant,
  sortParam,
  sortRows,
  sortWays,
  useBrandMarkVariant,
  useConfirm,
  useDebounced,
  useEscapeKey,
  usePageSize,
  useSorter,
  useStored
};
