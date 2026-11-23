# Regras do fork WinRust

- Preserve as alterações locais e as preferências do usuário ao incorporar mudanças do repositório original. Nunca sobrescreva alterações não examinadas.
- A interface do fork deve oferecer inglês e pacotes opcionais para português brasileiro, russo e japonês; mantenha a escolha de idioma do usuário e confira os textos novos do projeto original antes da integração.
- Preserve os ajustes do fork para múltiplos monitores, barras independentes, papel de parede, ícones da área de trabalho, arraste e ordem dos aplicativos, widget do Claude e proteção de aplicativos em tela cheia. Resolva diferenças após analisar o comportamento, não pela simples preferência automática de um lado.
- Ao atualizar a partir do projeto original, registre a versão de origem, examine alterações conflitantes, avalie o impacto e teste os componentes afetados antes de gerar e aplicar o instalador.
- Não substitua a instalação em uso por uma build que não tenha compilado e passado pelos testes pertinentes. Preserve um caminho de recuperação e informe limitações de validação visual ou em jogos.
- O instalador deve continuar autocontido e comparar os componentes locais com o conteúdo da própria build, sem apresentar essa verificação como uma consulta automática ao GitHub.
- Evite alterações desnecessárias de serviços, registro, programas de terceiros ou preferências do usuário. Preserve integração e dados do Claude Usage Monitor e suas credenciais.
