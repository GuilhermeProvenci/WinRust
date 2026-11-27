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

## Integração em andamento — upstream v0.6.0, `9fc501bb7e2860c0174f69765371df78358df704`

A atualização é seletiva e ainda não equivale à v0.6.0 completa. A base do fork é a branch `integrate/upstream-0.6.0`, iniciada em `51111c3`, que já traz parte do trabalho do upstream 0.5.8. Foram incorporados o registro duplo de pastas para acelerar o "Mostrar na pasta" do Chromium (#203, `8a6ad08`), navegação em dispositivos portáteis (#19, `837294a`) e abertura de pastas em nova aba com clique central (#180, `87baaa2`). O nome de dispositivos portáteis usa os catálogos do fork. A geometria de tela cheia foi adaptada de #193 (`f45f966`) para aceitar até um pixel de excesso sem tratar janelas comuns com borda como jogos.

A integração completa continua pendente: barra orientada a eventos, notificações, performance, proteção de jogos e outras alterações estruturais exigem reconciliação com múltiplos monitores, widget, idiomas e atualizador do fork. O upstream também cria o journal USN em volumes sem journal (#143); essa alteração de estado dos discos não foi incorporada automaticamente. A navegação em MTP e o uso de tela cheia dependem de testes manuais com dispositivos e jogos reais antes de considerar uma publicação segura.

Também foi adaptada parte de #197 (`c673dc5`): ao ocultar ou reposicionar a barra durante tela cheia, não elevá-la sobre outras janelas. Isso não inclui toda a lógica de estabilização de tela cheia do upstream; valide com jogos em ambos os monitores. O Alt+Tab também ignora o movimento sintético gerado quando abre sob um ponteiro parado (#183/#116).
