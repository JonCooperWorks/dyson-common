/* Dyson — clipboard helpers.
 *
 * The modern clipboard API (`navigator.clipboard.writeText`) is the
 * right call, but browsers gate it on a secure context — and Dyson is
 * commonly served over plain HTTP on a Tailscale / LAN address, where
 * `navigator.clipboard` is `undefined` — and a denied permission makes
 * it throw. The legacy textarea + `execCommand('copy')` dance still
 * works there, so every helper falls through to it; when even that is
 * refused, `copyText` selects the text on screen so the user can press
 * ⌘C themselves.
 *
 * Before this file lived here, the dance was copy-pasted across
 * turns.jsx, panels.jsx, and views-secondary.jsx — three places to
 * patch when clipboard permissions changed.
 */

/// Copies `text`. Answers 'copied', 'selected' (the browser refused,
/// so the contents of the element `select` are selected instead, ready
/// for a manual copy) or 'failed'.
export async function copyText(text, select = null) {
  try {
    if (!navigator.clipboard?.writeText) throw new Error('no clipboard API');
    await navigator.clipboard.writeText(text);
    return 'copied';
  } catch {
    if (legacyCopy(text)) return 'copied';
    if (select) { window.getSelection()?.selectAllChildren(select); return 'selected'; }
    return 'failed';
  }
}

/// Copies text that has yet to be fetched: `load()` returns a promise
/// of it. The clipboard is handed that promise at once, while the click
/// still counts as the user's (Safari refuses a write made after an
/// await); where ClipboardItem is missing it waits, then writes.
/// Answers like `copyText`.
export async function copyFetched(load) {
  const text = load();
  try {
    if (!window.ClipboardItem || !navigator.clipboard?.write) throw new Error('no ClipboardItem');
    await navigator.clipboard.write([new window.ClipboardItem({ 'text/plain': text.then(t => new Blob([t], { type: 'text/plain' })) })]);
    return 'copied';
  } catch {
    try { return await copyText(await text); } catch { return 'failed'; }
  }
}

/// The boolean form: true when the text reached the clipboard. Callers
/// flip a "copied" pip for ~1 s on true.
export async function copyToClipboard(text) {
  if (!text) return false;
  return (await copyText(text)) === 'copied';
}

// Copy the selection of a hidden, read-only text box, then hand focus
// back to whatever had it (the textarea steals it to select).
function legacyCopy(text) {
  if (typeof document === 'undefined') return false;
  const box = document.createElement('textarea');
  box.value = text;
  box.setAttribute('readonly', '');
  box.style.cssText = 'position:fixed;top:0;left:0;opacity:0;pointer-events:none';
  const active = document.activeElement;
  document.body.appendChild(box);
  box.select();
  let done = false;
  try { done = document.execCommand('copy'); } catch { done = false; }
  box.remove();
  active?.focus?.({ preventScroll: true });
  return done;
}
