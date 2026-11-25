# Integrações do projeto original

- Base principal: `de8f62f60f25a223788ff1ed04c414da57fe912f`.
- [PR 55](https://github.com/Lemxy/WinRust/pull/55): preservação e restauração dos backups de associações de pastas e unidades.
- [PR 49](https://github.com/Lemxy/WinRust/pull/49): preservação da presença ou ausência original de `AutoRestartShell`.
- [PR 48](https://github.com/Lemxy/WinRust/pull/48): restauração do estado de execução dos serviços, além da configuração de inicialização.
- [PR 69](https://github.com/Lemxy/WinRust/pull/69): partes da barra e do suporte a monitores, reconciliadas com arraste de ícones, idiomas, widget, jogos e configuração individual de ocultação deste fork. As mudanças desse PR no Explorer não foram incorporadas.

Essas alterações se baseiam no trabalho dos respectivos autores. O comportamento específico deste fork deve ser preservado em atualizações posteriores.

## Revisão de 22/11/2026 — upstream `8a658af699c50ba6439bc31b1327c66856e899da`

Integração seletiva sobre a base `de8f62f`: eventos de clique e duplo clique da bandeja (commit `8a658af`); leitura Unicode e recuperação do papel de parede, sem alterar o desktop nativo de vários monitores; cor de fundo do desktop, alternância de teclado na própria barra e preservação de PcaSvc/cbdhsvc em novas instalações (commit `3bed7aa`). Backups anteriores desses serviços continuam elegíveis à restauração. A conversão de papel de parede mantém a última cópia válida quando a nova falha.

A atualização completa ainda não foi integrada: a arquitetura de traduções, o instalador e a barra foram alterados profundamente no upstream e conflitam com recursos próprios deste fork. Em particular, o atualizador do upstream aponta para releases oficiais e não deve substituir automaticamente o fork. A integração seletiva não altera a base Git deste checkout.
