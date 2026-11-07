"""The whole website's docs in one file, for AI agents: docs/public/llms-full.txt
(also served at /llms-full.txt). AGENTS.md points to it.

    python tools/agent_docs.py          # write it
    python tools/agent_docs.py check    # fail if it's out of date (the Checks run this)

Pages come in the website's sidebar order (docs/.vitepress/config.mts);
pages the sidebar doesn't list follow, then the rest of docs/ is ignored.
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = os.path.join(ROOT, 'docs')
OUT = os.path.join(DOCS, 'public', 'llms-full.txt')
SITE = 'https://xcrafttm.github.io/XC-Circuit'
# Pages that aren't about using or extending the app (or are elsewhere in the repo).
SKIP = {'changelog.md', 'known-issues.md', 'discord-blocks-plan.md'}


def page_files():
    config = open(os.path.join(DOCS, '.vitepress', 'config.mts'), encoding='utf-8').read()
    order = []
    for link in re.findall(r'link:\s*"(/[^"#]*)"', config):
        rel = link.strip('/')
        rel = 'index.md' if not rel else (rel + '/index.md' if link.endswith('/') else rel + '.md')
        if rel not in order and os.path.isfile(os.path.join(DOCS, rel)):
            order.append(rel)
    if 'index.md' not in order:
        order.insert(0, 'index.md')
    rest = []
    for folder, dirs, files in os.walk(DOCS):
        dirs[:] = [d for d in dirs if d not in ('node_modules', '.vitepress', 'public')]
        for f in files:
            if f.endswith('.md'):
                rel = os.path.relpath(os.path.join(folder, f), DOCS).replace(os.sep, '/')
                if rel not in order:
                    rest.append(rel)
    return [p for p in order + sorted(rest) if os.path.basename(p) not in SKIP]


def clean(text):
    # Front matter (the home page's hero) and Vue components don't read as text.
    text = re.sub(r'\A---\n.*?\n---\n', '', text, flags=re.S)
    text = '\n'.join(line for line in text.split('\n') if not re.fullmatch(r'\s*<[A-Z][A-Za-z]*\s*/>\s*', line))
    return text.strip() + '\n'


def build():
    parts = [
        '# XC Circuit: the complete documentation\n',
        f'All pages of {SITE}, in the order of its sidebar. Made by tools/agent_docs.py from docs/; '
        'start with AGENTS.md in the repository for how the code is laid out.\n',
    ]
    for rel in page_files():
        url = SITE + '/' + (rel[:-len('index.md')] if rel.endswith('index.md') else rel[:-3])
        body = clean(open(os.path.join(DOCS, rel), encoding='utf-8').read())
        parts.append(f'\n\n---\n\n<!-- docs/{rel} · {url} -->\n\n{body}')
    return ''.join(parts)


def main():
    text = build()
    if sys.argv[1:] == ['check']:
        current = open(OUT, encoding='utf-8').read() if os.path.isfile(OUT) else ''
        if current != text:
            print('docs/public/llms-full.txt is out of date: run python tools/agent_docs.py')
            sys.exit(1)
        print('docs/public/llms-full.txt is up to date.')
        return
    with open(OUT, 'w', encoding='utf-8', newline='\n') as f:
        f.write(text)
    print(f'wrote {os.path.relpath(OUT, ROOT)}: {len(page_files())} pages, {len(text) // 1024} KB')


if __name__ == '__main__':
    main()
