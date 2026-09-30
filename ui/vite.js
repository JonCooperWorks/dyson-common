/* Vite helpers for dyson web apps. Node-side, imported from vite.config.js:
 *
 *   import { buildIdPlugin } from 'dyson-common-ui/vite';
 *   const buildId = buildIdPlugin();
 *   export default defineConfig({ plugins: [react(), buildId.plugin], define: buildId.define });
 *
 * One id per build, compiled in as `__BUILD__` and emitted beside the
 * page as `version.json`, which `createBuildWatcher` polls. */

export function buildIdPlugin({ fileName = 'version.json', global = '__BUILD__', build = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}` } = {}) {
  return {
    build,
    define: { [global]: JSON.stringify(build) },
    plugin: {
      name: 'dyson-build-id',
      apply: 'build',
      generateBundle() { this.emitFile({ type: 'asset', fileName, source: JSON.stringify({ build }) }); },
    },
  };
}
