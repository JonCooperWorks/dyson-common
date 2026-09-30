import { afterEach, describe, expect, test, vi } from 'vitest';
import { createBuildWatcher } from './buildWatch.js';
import { buildIdPlugin } from './vite.js';

afterEach(() => vi.unstubAllGlobals());

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

  test('the vite plugin defines the id and emits it', () => {
    const id = buildIdPlugin({ build: 'abc' });
    expect(id.define).toEqual({ __BUILD__: '"abc"' });
    const emitFile = vi.fn();
    id.plugin.generateBundle.call({ emitFile });
    expect(emitFile).toHaveBeenCalledWith({ type: 'asset', fileName: 'version.json', source: '{"build":"abc"}' });
  });
});
