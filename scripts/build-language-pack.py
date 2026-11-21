#!/usr/bin/env python3
"""Build a compact, data-only offline language pack from a reviewed catalog."""
import argparse
import json
import pathlib
import struct
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import importlib.util
spec = importlib.util.spec_from_file_location('audit', pathlib.Path(__file__).with_name('i18n-catalog.py'))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def build(catalog):
    locale = catalog['language']
    if locale not in ('pt-BR', 'ja'): raise ValueError('unsupported locale')
    if catalog.get('needs_review'): raise ValueError('resolve ambiguous translations before building')
    messages = catalog['messages']
    if len(messages) > 20_000: raise ValueError('too many messages')
    output = bytearray(b'WRLANG\x01\0')
    def put(text):
        raw = text.encode('utf-8')
        if len(raw) > 32_768 or b'\0' in raw: raise ValueError('invalid message')
        output.extend(struct.pack('<I', len(raw)))
        output.extend(raw)
    put(locale)
    output.extend(struct.pack('<I', len(messages)))
    for key, value in sorted(messages.items()):
        if audit.fields(key) != audit.fields(value): raise ValueError(f'placeholder mismatch: {key!r}')
        put(key)
        put(value)
    if len(output) > 2 * 1024 * 1024: raise ValueError('pack exceeds size limit')
    return bytes(output)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('catalog', type=pathlib.Path)
    parser.add_argument('output', type=pathlib.Path)
    args = parser.parse_args()
    data = json.loads(args.catalog.read_text(encoding='utf-8'))
    package = build(data)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(package)
    print(f'{data["language"]}: {len(data["messages"])} messages, {len(package)} bytes')


if __name__ == '__main__':
    main()
