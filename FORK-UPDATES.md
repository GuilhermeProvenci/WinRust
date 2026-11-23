# Atualizações do fork

O instalador verifica releases de `GuilhermeProvenci/WinRust` ao abrir. Uma atualização só é oferecida quando seu manifesto estiver assinado com a chave pública incluída nesta build e a versão for superior à registrada. Após o usuário clicar em baixar, o instalador valida a assinatura, o SHA-256 e os pacotes de idioma; abre outro instalador, sem executar uma instalação silenciosa. A escolha de componentes e a confirmação continuam com o usuário. O atualizador nunca consulta releases de `Lemxy/WinRust`.

Em instalações anteriores a esse sistema, execute esta versão e aplique as seleções uma vez para registrar a versão antes de usar a atualização online. O registro impede que uma versão antiga atualize o fork; remoção total continua disponível.

## Publicar uma nova release

1. Incremente `fork-version.txt`, compile todos os componentes e gere o pacote com `scripts/package.ps1`.
2. Execute `python scripts/sign-fork-update.py`. O script usa `WINRUST_FORK_SIGNING_KEY` ou a chave privada local configurada para este fork e cria os arquivos em `dist/signed-release`.
3. Publique esses arquivos na release do fork com a tag idêntica à versão. O manifesto, a assinatura, `WinRust.exe` e os três arquivos `.wrlang` são necessários para atualização online.

A chave privada é necessária para todas as próximas releases. Faça backup protegido fora do repositório, mantenha acesso restrito e nunca a publique. A perda da chave exige distribuir manualmente um novo instalador com outra chave pública. Trocas de chave e recuperação após comprometimento precisam ser planejadas explicitamente.

A verificação automática ocorre quando o instalador é aberto; não há tarefa agendada nem instalação em segundo plano.
