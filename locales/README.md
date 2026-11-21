# Language catalogs

`pt-BR.json` and `ja.json` are the source catalogs. The Japanese catalog was contributed by @eltociear. The translations used AI assistance and benefit from human review.

The installer does not embed these translations. `python scripts/build-language-pack.py locales/pt-BR.json dist/WinRust/locales/pt-BR.wrlang` creates a compact, data-only offline pack. `scripts/package.ps1` builds both packs and packages them beside the installer in a ZIP. The installer lets users choose which packs to install and which installed language to use; unselected existing packs are not removed. English and Russian remain built in.

`python scripts/i18n-catalog.py audit --catalog locales/pt-BR.json --strict` compares the catalog to source texts and checks format placeholders. Change `--catalog` for another language and use `--source PATH` to audit a different checkout. The extractor handles ordinary literal calls; other Rust expressions need manual review. Resolve ambiguous keys in `needs_review` before building a pack. Existing catalog entries are preserved during export.

Programs fall back to English if a pack is absent, invalid or lacks a message. Runtime formatting rejects translations with incompatible placeholders; translations that reorder anonymous `{}` arguments require human review. Installed packs are cached for the process lifetime, so restart running applications after updating a pack.
