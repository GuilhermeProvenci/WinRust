#!/usr/bin/env python3
"""Extract Russian source translations with context keys and validate placeholders."""
import importlib.util, json, pathlib
root = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("audit", root / "scripts/i18n-catalog.py")
audit = importlib.util.module_from_spec(spec); spec.loader.exec_module(audit)
messages = audit.russian_pairs(root)
for key, value in messages.items():
    if audit.fields(key.split("\x1f", 1)[0]) != audit.fields(value):
        raise ValueError(f"placeholder mismatch: {key!r}")
out = root / "locales/ru.json"
out.write_text(json.dumps({"language": "ru", "messages": dict(sorted(messages.items())), "needs_review": {}}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"{len(messages)} contextual Russian messages written to {out}")
