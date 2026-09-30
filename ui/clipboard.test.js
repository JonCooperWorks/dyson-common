import { afterEach, describe, expect, test, vi } from 'vitest';
import { copyFetched, copyText, copyToClipboard } from './clipboard.js';

const original = Object.getOwnPropertyDescriptor(navigator, 'clipboard');
function setClipboard(value) { Object.defineProperty(navigator, 'clipboard', { value, configurable: true }); }
afterEach(() => {
  if (original) Object.defineProperty(navigator, 'clipboard', original); else delete navigator.clipboard;
  delete document.execCommand;
  delete window.ClipboardItem;
});

describe('clipboard', () => {
  test('uses the clipboard API when it is there', async () => {
    const writeText = vi.fn().mockResolvedValue();
    setClipboard({ writeText });
    expect(await copyText('hi')).toBe('copied');
    expect(writeText).toHaveBeenCalledWith('hi');
    expect(await copyToClipboard('')).toBe(false);
  });

  test('falls back to the legacy copy, then to selecting the text', async () => {
    setClipboard({ writeText: vi.fn().mockRejectedValue(new Error('denied')) });
    document.execCommand = vi.fn().mockReturnValue(true);
    expect(await copyText('hi')).toBe('copied');
    document.execCommand = vi.fn().mockReturnValue(false);
    const pre = document.createElement('pre');
    pre.textContent = 'hi';
    document.body.appendChild(pre);
    expect(await copyText('hi', pre)).toBe('selected');
    expect(window.getSelection().toString()).toBe('hi');
    expect(await copyText('hi')).toBe('failed');
    expect(await copyToClipboard('hi')).toBe(false);
  });

  test('hands a pending fetch to the clipboard at once when it can', async () => {
    const write = vi.fn().mockResolvedValue();
    setClipboard({ write, writeText: vi.fn() });
    window.ClipboardItem = class { constructor(items) { this.items = items; } };
    expect(await copyFetched(() => Promise.resolve('later'))).toBe('copied');
    expect(write).toHaveBeenCalledTimes(1);
    delete window.ClipboardItem;
    const writeText = vi.fn().mockResolvedValue();
    setClipboard({ writeText });
    expect(await copyFetched(() => Promise.resolve('later'))).toBe('copied');
    expect(writeText).toHaveBeenCalledWith('later');
  });
});
