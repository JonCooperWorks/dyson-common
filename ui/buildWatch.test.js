import { afterEach, describe, expect, test, vi } from 'vitest';
import { createBuildWatcher } from './buildWatch.js';
import { buildIdPlugin } from './vite.js';

afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); });

describe('createBuildWatcher', () => {
  test('flags a newer build once and tells listeners', async () => {
    const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ build: 'b2' }) });
    vi.stubGlobal('fetch', fetch);
    const watcher = createBuildWatcher({ build: 'b1' });
    const seen = vi.fn();
    watcher.onOutdated(seen);
    expect(await watcher.check()).toBe(true);
    expect(watcher.outdated()).toBe(true);
    expect(seen).toHaveBeenCalledWith('b2');
    await watcher.check();
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  test('stays quiet on the same build, on errors and in dev', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ build: 'b1' }) }));
    expect(await createBuildWatcher({ build: 'b1' }).check()).toBe(false);
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
    expect(await createBuildWatcher({ build: 'b1' }).check()).toBe(false);
    const fetch = vi.fn();
    vi.stubGlobal('fetch', fetch);
    expect(await createBuildWatcher({}).check()).toBe(false);
    expect(fetch).not.toHaveBeenCalled();
  });

  test('stop() clears the poll and every listener watch() added (MEM-10)', async () => {
    vi.useFakeTimers();
    const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ build: 'b1' }) });
    vi.stubGlobal('fetch', fetch);
    const added = [];
    const removed = [];
    const wrap = (target) => {
      const add = target.addEventListener.bind(target);
      const remove = target.removeEventListener.bind(target);
      vi.spyOn(target, 'addEventListener').mockImplementation((type, fn, opts) => { added.push([target, type, fn]); add(type, fn, opts); });
      vi.spyOn(target, 'removeEventListener').mockImplementation((type, fn, opts) => { removed.push([target, type, fn]); remove(type, fn, opts); });
    };
    wrap(window);
    wrap(document);
    const watcher = createBuildWatcher({ build: 'b1', every: 1000 });
    watcher.watch();
    await vi.advanceTimersByTimeAsync(1000);
    expect(fetch).toHaveBeenCalledTimes(1);
    watcher.stop();
    await vi.advanceTimersByTimeAsync(5000);
    window.dispatchEvent(new Event('focus'));
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(added.length).toBeGreaterThan(0);
    for (const [target, type, fn] of added) {
      expect(removed.some(([t, ty, f]) => t === target && ty === type && f === fn)).toBe(true);
    }
    // Watching again after stop works.
    watcher.watch();
    await vi.advanceTimersByTimeAsync(1000);
    expect(fetch).toHaveBeenCalledTimes(2);
    watcher.stop();
    vi.restoreAllMocks();
  });

  test('the vite plugin defines the id and emits it', () => {
    const id = buildIdPlugin({ build: 'abc' });
    expect(id.define).toEqual({ __BUILD__: '"abc"' });
    const emitFile = vi.fn();
    id.plugin.generateBundle.call({ emitFile });
    expect(emitFile).toHaveBeenCalledWith({ type: 'asset', fileName: 'version.json', source: '{"build":"abc"}' });
  });
});
