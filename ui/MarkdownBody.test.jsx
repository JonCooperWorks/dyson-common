// The shared MarkdownBody (dyson-common-ui/markdown): the real pipeline.
import { afterEach, describe, expect, test } from 'vitest';
import React from 'react';
import { cleanup, render } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MarkdownBody } from './markdown.js';
import { isPlainParagraph, safeMarkdownHref } from './markdownSafety.js';

afterEach(cleanup);

describe('MarkdownBody', () => {
  test('renders GFM and line breaks', () => {
    const { container } = render(<MarkdownBody markdown={'**bold**\nnext\n\n| a |\n|---|\n| 1 |'}/>);
    expect(container.querySelector('.md-body strong')).toHaveTextContent('bold');
    expect(container.querySelector('br')).not.toBeNull();
    expect(container.querySelector('table td')).toHaveTextContent('1');
  });

  test('raw HTML is never rendered as elements', () => {
    const { container } = render(<MarkdownBody markdown={'hi <img src=x onerror="alert(1)"> <script>alert(1)</script> <a href="javascript:alert(1)">x</a>'}/>);
    expect(container.querySelector('img, script')).toBeNull();
    expect(container.querySelector('a')).toBeNull();
  });

  test('links: only web, mail and same-origin relative, external ones open safely', () => {
    const { container } = render(<MarkdownBody markdown={'[a](https://x.example) [b](javascript:alert(1)) [c](//evil.example) [d](#/agents)'}/>);
    const links = [...container.querySelectorAll('a')];
    expect(links.map((a) => a.getAttribute('href'))).toEqual(['https://x.example', '#/agents']);
    expect(links[0]).toHaveAttribute('rel', 'noopener noreferrer');
    expect(links[0]).toHaveAttribute('target', '_blank');
    expect(links[1]).not.toHaveAttribute('target');
    expect(container).toHaveTextContent('b');
    expect(container).toHaveTextContent('c');
  });

  test('never loads an image: the alt text links to it instead', () => {
    const { container } = render(<MarkdownBody markdown={'See ![the diagram](https://evil.example/p.png?u=me) and ![](javascript:alert(1))'}/>);
    expect(container.querySelector('img')).toBeNull();
    const link = container.querySelector('a.md-image-link');
    expect(link).toHaveTextContent('the diagram');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    expect(container.querySelector('.md-image-text')).toHaveTextContent('Image');
  });

  test('the brief variant demotes headings', () => {
    const { container } = render(<MarkdownBody variant="brief" markdown={'# Who I am\n\nReview **PRs**.'}/>);
    expect(container.querySelector('h1, h2, h3, h4, h5, h6')).toBeNull();
    expect(container.querySelector('p.md-demoted strong')).toHaveTextContent('Who I am');
  });

  test('plain prose renders the same with or without the shortcut', () => {
    const prose = 'Claude Opus 5 is a flagship model for demanding reasoning and coding.';
    expect(isPlainParagraph(prose)).toBe(true);
    const { container } = render(<MarkdownBody markdown={prose} className="x"/>);
    expect(container.innerHTML).toBe(`<div class="x"><p>${prose}</p></div>`);
    for (const s of ['*e*', 'a [l](/x)', 'see https://e.com', 'two\nlines', '- item', '', ' padded']) {
      expect(isPlainParagraph(s), s).toBe(false);
    }
  });
});

describe('safeMarkdownHref', () => {
  test('keeps web and relative links, drops everything else', () => {
    expect(safeMarkdownHref('https://example.com')).toBe('https://example.com');
    expect(safeMarkdownHref('mailto:a@b.c')).toBe('mailto:a@b.c');
    expect(safeMarkdownHref('#/agents')).toBe('#/agents');
    expect(safeMarkdownHref('./x')).toBe('./x');
    expect(safeMarkdownHref('javascript:alert(1)')).toBe('');
    expect(safeMarkdownHref(' JavaScript:alert(1)')).toBe('');
    expect(safeMarkdownHref('//evil.example')).toBe('');
    expect(safeMarkdownHref('/\\evil.example')).toBe('');
    expect(safeMarkdownHref('data:text/html,x')).toBe('');
  });
});
