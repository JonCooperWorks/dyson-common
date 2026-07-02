/* Tests for downloadBlob — anchor click + deferred revoke. */
import { describe, expect, test, beforeEach, afterEach, vi } from 'vitest';
import { downloadBlob } from './download.js';

describe('downloadBlob', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    URL.createObjectURL = vi.fn(() => 'blob:mock');
    URL.revokeObjectURL = vi.fn();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  test('creates an object URL, clicks a download anchor, and revokes after a delay', () => {
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const blob = new Blob(['hello'], { type: 'text/plain' });

    downloadBlob(blob, 'greeting.txt');

    expect(URL.createObjectURL).toHaveBeenCalledWith(blob);
    expect(click).toHaveBeenCalledTimes(1);
    // Anchor is removed from the DOM after the synthetic click.
    expect(document.querySelector('a[download]')).toBeNull();

    // Revoke is deferred, not immediate.
    expect(URL.revokeObjectURL).not.toHaveBeenCalled();
    vi.advanceTimersByTime(5000);
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:mock');
  });

  test('falls back to a default filename', () => {
    let captured;
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(function () {
        captured = this.download;
      });
    downloadBlob(new Blob(['x']));
    expect(click).toHaveBeenCalled();
    expect(captured).toBe('download');
  });
});
