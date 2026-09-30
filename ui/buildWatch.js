/* Stale-tab detection. A tab left open across a deploy keeps running the
 * old bundle against the new server — and a lazily loaded chunk the
 * deploy removed leaves a blank page. The build stamps an id into the
 * bundle and serves the same id beside it (see `dyson-common-ui/vite`'s
 * `buildIdPlugin`); the watcher polls it and tells the app when they
 * differ, so the app can reload on its next navigation or offer a
 * reload button.
 *
 *   const watcher = createBuildWatcher({ build: __BUILD__ });
 *   watcher.watch();
 *   watcher.onOutdated(() => setBanner(true));
 *   // in the router: if (watcher.outdated()) location.assign(url)
 *
 * Checks every `every` ms while the tab is visible, and again whenever it
 * is shown or focused. A build of 'dev' never checks. */

export function createBuildWatcher({ build, url = '/version.json', every = 5 * 60 * 1000, reloadKey = 'dyson-reloaded' } = {}) {
  const current = typeof build === 'string' && build ? build : 'dev';
  let newer = false;
  let watching = false;
  const listeners = new Set();

  async function check() {
    if (newer || current === 'dev') return newer;
    try {
      const answer = await fetch(`${url}?t=${Date.now()}`, { cache: 'no-store' });
      if (!answer.ok) return false;
      const served = (await answer.json())?.build;
      if (served && served !== current) { newer = true; listeners.forEach(fn => fn(served)); }
    } catch { /* offline, or mid-deploy: look again later */ }
    return newer;
  }

  function watch() {
    if (watching || typeof window === 'undefined') return;
    watching = true;
    setInterval(() => { if (!document.hidden) check(); }, every);
    document.addEventListener('visibilitychange', () => { if (!document.hidden) check(); });
    window.addEventListener('focus', check);
    // A lazy chunk the deploy removed: reload once per build rather than
    // leave the page blank (sessionStorage stops a reload loop).
    window.addEventListener('vite:preloadError', event => {
      const key = `${reloadKey}-${current}`;
      try { if (sessionStorage.getItem(key)) return; sessionStorage.setItem(key, '1'); } catch { return; }
      event.preventDefault();
      window.location.reload();
    });
  }

  return {
    build: current,
    outdated: () => newer,
    onOutdated(fn) { listeners.add(fn); return () => listeners.delete(fn); },
    check,
    watch,
  };
}
