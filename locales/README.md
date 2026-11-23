# Pacotes de idioma

Os catálogos `pt-BR.json`, `ru.json` e `ja.json` geram pacotes de dados separados do executável, sem acesso à rede. Execute `python scripts/build-language-pack.py locales/pt-BR.json caminho/pt-BR.wrlang` para gerar um pacote, ou `scripts/package.ps1` após compilar o instalador para gerar o ZIP completo.

Extraia o ZIP inteiro e abra `WinRust.exe`. Em **Idiomas**, escolha os pacotes a instalar e o idioma da interface; o Windows é usado como padrão quando não há escolha anterior. Pacotes já instalados que não forem selecionados são preservados. Os programas em execução podem precisar ser reiniciados após uma atualização do pacote. Se um pacote estiver ausente ou for inválido, o texto em inglês é usado.

Execute `python scripts/i18n-catalog.py audit --catalog locales/pt-BR.json --strict` para localizar textos ainda sem tradução e conferir os campos de formatação. A extração por padrões não cobre todas as expressões Rust: traduções, inclusive o catálogo japonês contribuído, exigem revisão humana.

O catálogo russo usa chaves com contexto para distinguir traduções diferentes do mesmo texto em inglês. Após alterar textos russos de origem, execute `python scripts/extract-russian-catalog.py` e confira `python scripts/i18n-catalog.py audit --catalog locales/ru.json --strict`.
