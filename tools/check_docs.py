#!/usr/bin/env python3
"""Check repository documentation pairs and local references without robot access."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

SKIP = {'.git', '.governance', '.venv', '.screenshots', 'node_modules', 'vendor',
        'build', 'target', '__pycache__', 'dist', 'out'}
LINK = re.compile(r'(!?)\[[^\]\n]*\]\(<?([^\s)>]+)>?(?:\s+["\'][^\n]*?["\'])?\)')
FENCE = re.compile(r'^\s*(`{3,}|~{3,})(.*)$')


def split_markdown(text: str):
    """Separate prose from fenced examples; comments are not live references."""
    text = re.sub(r'<!--.*?-->', '', text, flags=re.S)
    prose, blocks, content = [], [], []
    marker = language = ''
    for line in text.splitlines():
        match = FENCE.match(line)
        if not marker and match:
            marker, language = match.group(1), match.group(2).strip().lower()
            content = []
        elif marker and match and match.group(1)[0] == marker[0] and len(match.group(1)) >= len(marker):
            blocks.append((language, '\n'.join(content)))
            marker = language = ''
        elif marker:
            content.append(line)
        else:
            prose.append(line)
    return '\n'.join(prose), blocks, bool(marker)


def heading_ids(prose: str):
    ids, seen = set(), {}
    for title in re.findall(r'^#{1,6}\s+(.+?)\s*#*$', prose, re.M):
        title = re.sub(r'<[^>]+>', '', title).replace('`', '').lower()
        slug = re.sub(r'[^\w\- ]', '', title).replace(' ', '-')
        count = seen.get(slug, 0)
        seen[slug] = count + 1
        ids.add(slug if count == 0 else f'{slug}-{count}')
    ids.update(re.findall(r'\bid=["\']([^"\']+)["\']', prose))
    return ids


def code_examples(blocks):
    result = []
    for language, code in blocks:
        if language not in {'sh', 'bash', 'shell', 'toml', 'json', 'python', 'rust'}:
            continue
        if language == 'json':
            try: code = json.dumps(json.loads(code), sort_keys=True, ensure_ascii=False)
            except ValueError: pass
        lines = []
        for line in code.splitlines():
            if language in {'sh', 'bash', 'shell', 'toml'}:
                line = line.split('#', 1)[0]
            line = re.sub(r'<[^<>\n]+>', '<ARG>', line).strip()
            if line: lines.append(line)
        result.append(('shell' if language in {'sh', 'bash', 'shell'} else language, lines))
    return result


def check(root: Path):
    root = root.resolve()
    errors = []
    documents = []
    for folder, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIP and not d.startswith('.'))
        documents.extend(Path(folder)/name for name in sorted(files) if name.endswith('.md'))
    parsed = {}
    for path in documents:
        try:
            raw = path.read_bytes()
            text = raw.decode('utf-8')
            if b'\r' in raw: errors.append(f'{path.relative_to(root)}: use LF line endings')
            prose, blocks, unclosed = split_markdown(text)
            if unclosed: errors.append(f'{path.relative_to(root)}: unclosed code fence')
            parsed[path.resolve()] = (prose, blocks)
        except (OSError, UnicodeError) as exc:
            errors.append(f'{path.relative_to(root)}: cannot read UTF-8: {exc}')

    links = 0
    destinations = {}
    for path, (prose, _) in parsed.items():
        targets = [(m.group(2), bool(m.group(1))) for m in LINK.finditer(prose)]
        targets += [(m.group(1), False) for m in re.finditer(r'^\s*\[[^\]]+\]:\s*<?([^\s>]+)>?', prose, re.M)]
        targets += [(m.group(1), True) for m in re.finditer(r'<(?:img|source)\b[^>]*\bsrc=["\']([^"\']+)["\']', prose, re.I)]
        destinations[path] = set()
        for url, media in targets:
            split = urlsplit(url)
            if split.scheme or split.netloc: continue
            target = (path.parent/unquote(split.path)).resolve() if split.path else path
            links += 1
            if not target.is_relative_to(root):
                errors.append(f'{path.relative_to(root)}: reference leaves repository: {url}')
                continue
            destinations[path].add(target)
            if not target.exists():
                errors.append(f'{path.relative_to(root)}: missing file: {url}')
                continue
            if split.fragment and target.suffix == '.md':
                target_prose = parsed.get(target, ('', []))[0]
                if unquote(split.fragment) not in heading_ids(target_prose):
                    errors.append(f'{path.relative_to(root)}: missing heading: {url}')
            if media and not target.is_file():
                errors.append(f'{path.relative_to(root)}: media is not a file: {url}')

    locale_root = root/'docs'
    en = {p.relative_to(locale_root/'en').as_posix() for p in (locale_root/'en').rglob('*.md')}
    zh = {p.relative_to(locale_root/'zh-CN').as_posix() for p in (locale_root/'zh-CN').rglob('*.md')}
    if not en or not zh: errors.append('Both docs/en and docs/zh-CN must contain guides')
    for name in sorted(en ^ zh): errors.append(f'Unpaired guide: {name}')
    pairs = [(locale_root/'en'/name, locale_root/'zh-CN'/name) for name in sorted(en & zh)]
    for path in documents:
        if path.name != 'README.md' or path.parent == locale_root or path.is_relative_to(locale_root): continue
        counterpart = path.with_name('README.zh-CN.md')
        if not counterpart.is_file(): errors.append(f'{path.relative_to(root)}: missing Chinese entry')
        else: pairs.append((path, counterpart))
    for path in documents:
        if path.name == 'README.zh-CN.md' and not path.with_name('README.md').is_file():
            errors.append(f'{path.relative_to(root)}: missing English entry')
    for english, chinese in pairs:
        english, chinese = english.resolve(), chinese.resolve()
        a, b = parsed.get(english), parsed.get(chinese)
        if not a or not b: continue
        if chinese not in destinations.get(english, set()) or english not in destinations.get(chinese, set()):
            errors.append(f'{english.relative_to(root)}: language switch must be reciprocal')
        ah = re.findall(r'^(#{1,6})\s', a[0], re.M)
        bh = re.findall(r'^(#{1,6})\s', b[0], re.M)
        if ah != bh: errors.append(f'{english.relative_to(root)}: heading structure differs from Chinese')
        if code_examples(a[1]) != code_examples(b[1]):
            errors.append(f'{english.relative_to(root)}: command/config examples differ from Chinese')
        ar = sum(bool(re.match(r'^\s*\|', line)) for line in a[0].splitlines())
        br = sum(bool(re.match(r'^\s*\|', line)) for line in b[0].splitlines())
        if ar != br: errors.append(f'{english.relative_to(root)}: table row count differs from Chinese')
    for locale, names in [('en', en), ('zh-CN', zh)]:
        index=(locale_root/locale/'README.md').resolve()
        for name in sorted(names-{'README.md'}):
            if (locale_root/locale/name).resolve() not in destinations.get(index,set()):
                errors.append(f'docs/{locale}/README.md: missing navigation to {name}')
    for name, locale in [('README.md','en'),('README.zh-CN.md','zh-CN')]:
        if (locale_root/locale/'README.md').resolve() not in destinations.get((root/name).resolve(),set()):
            errors.append(f'{name}: missing locale documentation index')
    return {'documents': len(documents), 'language_pairs': len(pairs),
            'local_references': links, 'errors': errors}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1])
    args=parser.parse_args()
    result=check(args.root)
    print(json.dumps(result,ensure_ascii=False,indent=2))
    return bool(result['errors'])


if __name__ == '__main__': sys.exit(main())
