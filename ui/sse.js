/* An incremental text/event-stream parser, per the WHATWG "event stream
 * interpretation" rules: for streams read with fetch() (which can send
 * headers, unlike EventSource).
 *
 *   const stop = new AbortController();
 *   await readSse(response, (e) => { if (e.event === 'tool_call') use(JSON.parse(e.data)); },
 *                 { signal: stop.signal });
 *
 * Lines may end in \n, \r\n or \r (a \r\n pair split across chunks is one
 * ending). A field's value loses one leading space; data lines are joined
 * with \n; comments (':') and unknown fields are ignored; an event with no
 * data is not dispatched, nor one the stream ends in the middle of. The
 * id carries over to later events, as Last-Event-ID does.
 *
 * At most `maxBuffer` characters of an unfinished line and event are held
 * (1 MiB by default); past that push() throws, so a peer that never ends
 * an event cannot take the tab's memory. */

export const SSE_DEFAULT_MAX_BUFFER = 1024 * 1024;

export function createSseParser(onEvent, { maxBuffer = SSE_DEFAULT_MAX_BUFFER } = {}) {
  let line = '';
  let afterCr = false;
  let event = '';
  let data = '';
  let hasData = false;
  let retry;
  let lastId;

  const check = () => {
    if (line.length + data.length > maxBuffer) {
      throw new Error(`an SSE event longer than ${maxBuffer} characters`);
    }
  };

  const dispatch = () => {
    const out = { event: event || 'message', data, id: lastId, retry };
    const had = hasData;
    event = ''; data = ''; hasData = false; retry = undefined;
    if (had) onEvent(out);
  };

  const lineDone = (text) => {
    if (text === '') { dispatch(); return; }
    if (text[0] === ':') return;
    const colon = text.indexOf(':');
    const field = colon < 0 ? text : text.slice(0, colon);
    let value = colon < 0 ? '' : text.slice(colon + 1);
    if (value[0] === ' ') value = value.slice(1);
    if (field === 'event') event = value;
    else if (field === 'data') { data = hasData ? `${data}\n${value}` : value; hasData = true; }
    else if (field === 'id') { if (!value.includes('\0')) lastId = value; }
    else if (field === 'retry') { if (/^\d+$/.test(value)) retry = Number(value); }
  };

  function push(chunk) {
    let rest = String(chunk);
    if (afterCr) {
      afterCr = false;
      if (rest[0] === '\n') rest = rest.slice(1);
    }
    let start = 0;
    for (let i = 0; i < rest.length; i++) {
      const c = rest[i];
      if (c !== '\n' && c !== '\r') continue;
      line += rest.slice(start, i);
      check();
      const done = line;
      line = '';
      lineDone(done);
      if (c === '\r') {
        if (i + 1 < rest.length) { if (rest[i + 1] === '\n') i++; }
        else afterCr = true;
      }
      start = i + 1;
    }
    line += rest.slice(start);
    check();
  }

  // The stream ended: an unfinished event is dropped.
  function end() {
    line = ''; afterCr = false; event = ''; data = ''; hasData = false; retry = undefined;
  }

  return { push, end, lastEventId: () => lastId };
}

/* Read a fetch Response (or anything with a ReadableStream `body`) as SSE,
 * calling onEvent for each event until the stream ends or `signal` aborts.
 * Rejects on a read error or an over-long event; an abort resolves. */
export async function readSse(response, onEvent, { signal, maxBuffer } = {}) {
  const body = response?.body;
  if (!body) return;
  const reader = body.getReader();
  const decoder = new TextDecoder();
  const parser = createSseParser((event) => { if (!signal?.aborted) onEvent(event); }, { maxBuffer });
  const onAbort = () => { reader.cancel().catch(() => {}); };
  signal?.addEventListener('abort', onAbort, { once: true });
  try {
    while (!signal?.aborted) {
      const { value, done } = await reader.read();
      if (done) break;
      parser.push(decoder.decode(value, { stream: true }));
    }
    if (!signal?.aborted) parser.push(decoder.decode());
    parser.end();
  } catch (err) {
    if (!signal?.aborted) throw err;
  } finally {
    signal?.removeEventListener('abort', onAbort);
    try { reader.releaseLock(); } catch { /* a cancelled read may still hold it */ }
  }
}
