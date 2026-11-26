import { describe, expect, test, vi } from 'vitest';
import { createSseParser, readSse } from './sse.js';

const collect = (chunks, options) => {
  const events = [];
  const parser = createSseParser((e) => events.push(e), options);
  for (const chunk of chunks) parser.push(chunk);
  parser.end();
  return events;
};

describe('createSseParser', () => {
  test('fields, comments, multi-line data, id carry-over and retry', () => {
    const events = collect([': hi\nevent: tool_call\ndata: {"a":1}\ndata:second\nid: 7\nretry: 1500\nbogus: x\n\ndata\n\n']);
    expect(events).toEqual([
      { event: 'tool_call', data: '{"a":1}\nsecond', id: '7', retry: 1500 },
      { event: 'message', data: '', id: '7', retry: undefined },
    ]);
  });

  test('every line ending, split anywhere', () => {
    const text = 'event: a\r\ndata: x\r\n\r\nevent: b\rdata: y\r\rdata: z\n\n';
    for (let at = 0; at <= text.length; at++) {
      const events = collect([text.slice(0, at), text.slice(at)]);
      expect(events.map((e) => [e.event, e.data]), `split at ${at}`).toEqual([['a', 'x'], ['b', 'y'], ['message', 'z']]);
    }
  });

  test('no data, no event; an unfinished event is dropped at the end', () => {
    expect(collect(['event: ping\n\n: keepalive\n\ndata: cut'])).toEqual([]);
  });

  test('a line or event over the limit throws', () => {
    const parser = createSseParser(() => {}, { maxBuffer: 16 });
    parser.push('data: 0123456789');
    expect(() => parser.push('abcdef')).toThrow(/16/);
  });
});

describe('readSse', () => {
  const body = (parts) => new ReadableStream({
    start(controller) {
      const enc = new TextEncoder();
      for (const part of parts) controller.enqueue(enc.encode(part));
      controller.close();
    },
  });

  test('reads a fetch Response body, keeping characters split across chunks', async () => {
    const bytes = new TextEncoder().encode('data: héllo\n\n');
    const split = new ReadableStream({
      start(c) { c.enqueue(bytes.slice(0, 8)); c.enqueue(bytes.slice(8)); c.close(); },
    });
    const onEvent = vi.fn();
    await readSse({ body: split }, onEvent);
    expect(onEvent).toHaveBeenCalledWith({ event: 'message', data: 'héllo', id: undefined, retry: undefined });
  });

  test('stops when aborted', async () => {
    const controller = new AbortController();
    const seen = [];
    await readSse({ body: body(['data: 1\n\n', 'data: 2\n\n']) }, (e) => { seen.push(e.data); controller.abort(); }, { signal: controller.signal });
    expect(seen).toEqual(['1']);
  });
});
