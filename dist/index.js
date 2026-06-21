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
  const listId = React3.useId();
  React3.useEffect(() => {
    setQuery(committedLabel);
  }, [committedLabel]);
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
  ), open && !disabled ? /* @__PURE__ */ React3.createElement("ul", { className: "combobox-list", id: listId, role: "listbox" }, visible.length === 0 ? /* @__PURE__ */ React3.createElement("li", { className: "combobox-empty" }, "no matches") : visible.map((opt, i) => /* @__PURE__ */ React3.createElement(
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
  ))) : null);
}
export {
  Combobox,
  Modal,
  useEscapeKey
};
