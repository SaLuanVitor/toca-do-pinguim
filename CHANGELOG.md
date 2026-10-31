# Changelog

As mudanças de cada versão. O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e a numeração segue o [versionamento semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

- **Excluir distro.** Na aba Distros, o botão Excluir aparece em toda distro que não é do Docker
  Desktop e só funciona com ela parada. Para confirmar, digita-se o nome dela; opcionalmente, o
  app exporta antes uma cópia em `.tar`, que volta com `wsl --import`, e se a cópia falhar nada é
  excluído. No fim, o app diz quanto espaço liberou e registra a exclusão na Atividade.
- Um botão **?** ao lado de Devolver o cache, em Desempenho > Memória e na opção automática,
  explica o que é esse cache e quando devolvê-lo vale a pena.

### Corrigido

- A análise de pastas, em Desempenho > Disco, se perdia ao trocar de aba ou fechar o app, e
  levava minutos para refazer. Agora a última análise de cada distro fica guardada, aparece ao
  voltar, com a hora em que foi feita, e só serve para apagar pastas enquanto tiver menos de
  30 min; depois disso, a tela pede uma análise nova antes de apagar.
- A aba Desempenho > Disco abria vazia a cada abertura do app até a leitura terminar, e não
  mostrava que estava lendo. Agora a última leitura dos discos e do Docker fica gravada e aparece
  na hora, com a hora em que foi feita, enquanto uma nova é lida por trás; um indicador girando
  marca o que está sendo lido. Medido com a versão de depuração: na primeira abertura, discos em
  4,8 s e Docker em 12,1 s; na seguinte, os dois em 1,3 s.

## [0.11.0] - 2026-10-30

### Adicionado

- **Segundo plano.** O app ganhou um ícone na área de notificação, entre os ícones ocultos: um
  clique abre a janela, e o menu dele tem Sair. Fechar a janela deixa o app lá, avisando de
  memória acabando e de WSL travado, em vez de sair; a primeira vez, uma notificação diz onde ele
  ficou. Em Configurações > Este app dá para desligar isso e ligar a abertura com o Windows, que
  sobe o app direto na área de notificação.

### Corrigido

- O aviso de WSL travado ficava na tela para sempre quando a distro afetada era uma que o app
  quase não chama por `wsl.exe -d`, como a do Docker: ele só saía com uma nova chamada à mesma
  distro dando certo. Agora sai também quando o agente dela volta a mandar dados, quando ela é
  desligada ou quando o WSL é desligado pelo app.
- Depois de uma trava do WSL, o app esperava até 5 min entre tentativas de subir os agentes, e
  essa espera continuava valendo depois que o WSL voltava ou era desligado; as distros demoravam a
  aparecer. Agora a espera zera nesses dois casos.
- O aviso diz o que acontece de fato: o WSL não está abrindo sessões novas, o que já está
  rodando continua, e o app não consegue ler as distros.
- Ligar ou reiniciar uma distro pelo app levava cerca de 19 s para ela aparecer com dados quando
  ela estava sem atividade: o app esperava o prazo de ociosidade do WSL mais 5 s antes de subir o
  agente, para não segurá-la ligada. Quando é você quem liga, essa espera não vale mais; medido,
  0,8 s.
- A aba Desempenho > Disco só aparecia quando tudo terminava de ser lido, e o espaço do Docker
  leva cerca de 5 s por consulta, com duas consultas em sequência. Com o WSL travado, ela ainda
  esperava até 60 s pelo `df` da distro travada. Agora os discos e o Docker chegam cada um no seu
  tempo, as consultas rodam em paralelo, o Docker é lido de antemão quando liga e a última
  leitura fica guardada; distro travada não é consultada, e o `df` desiste em 5 s. Medido com o
  WSL travado: Docker em 4,6 s e discos em 5,1 s ao abrir o app, e 0,1 s ao voltar à aba.

## [0.10.0] - 2026-10-30

### Adicionado

- Em Desempenho > Disco, o **espaço do Docker por projeto do Compose**: containers, imagens que
  só aquele projeto usa e volumes, com o total. Dá para apagar os containers parados e os
  volumes sem uso de um projeto, com confirmação.
- A swap do WSL aparece entre os discos. O WSL 2.6.1 a grava em `%Temp%\<GUID>\swap.vhdx`, e
  não no `%Temp%\swap.vhdx` da documentação; o app procura nos dois e respeita o `swapFile`.
- A análise de pastas lista todas as distros instaladas. Uma desligada aparece como tal, e
  "Ligar e analisar" a liga, com confirmação, só para a análise; o WSL a desliga sozinho
  depois.
- Mais pastas de cache na análise: pnpm, Maven, Gradle, snap e o npm do root.

### Mudado

- A aba Disco segue a ordem discos, dentro de uma distro, Docker e limpeza programada. O
  Docker e os itens dele na limpeza programada só aparecem em quem tem Docker.
- Em Desempenho > Memória, a barra de dentro da VM chamava as partes do cache de "Tabelas de
  arquivos", "Blocos de disco" e "Arquivos lidos", o que fazia parecer espaço em disco. Agora
  são "Cache: nomes e pastas", "Cache: blocos lidos" e "Cache: conteúdo de arquivos", a barra
  se chama "RAM dentro da VM do WSL", e o texto abaixo diz que tudo ali é RAM.
- Outros textos que misturavam RAM e disco: a coluna de memória em Processos e no Docker por
  projeto agora diz RAM, assim como a etiqueta dos eventos de falta de memória na Atividade; a
  swap aparece como "arquivo no disco"; as pastas de cache da aba Disco são "em disco"; e, em
  Configurações, `memory`, `swap` e `autoMemoryReclaim` dizem o que é RAM e o que é disco.

### Corrigido

- A lista "Maiores pastas da raiz" escondia a maior pasta, que costuma ser o `/tmp`.

## [0.9.0] - 2026-10-30

### Adicionado

- Aba **Disco**, em Desempenho: o tamanho de cada disco virtual no Windows, quanto está ocupado dentro
  e quanto é espaço vazio que o disco não devolveu, incluindo o disco de dados do Docker
  Desktop e um disco antigo dele que tenha ficado para trás. O Docker aparece por categoria
  (imagens, containers, volumes, cache de build) com o quanto cada uma libera, e cada limpeza
  pede confirmação: containers parados vêm por padrão só os que não pertencem a um projeto do
  Compose, e volumes sem uso são escolhidos um a um. A análise de uma distro, a pedido, mostra
  as pastas maiores, as entradas do `/tmp` com a data e os caches conhecidos; o que um
  container ligado monta fica bloqueado. Limpeza semanal opcional, desligada por padrão.
- **Compactar** um disco virtual em Desempenho > Disco: devolve ao Windows o espaço que o Linux já
  liberou por dentro. Desliga o WSL, roda o `diskpart` como administrador (o Windows pede a
  permissão) e mostra o tamanho antes e depois. Pede o Docker Desktop fechado.
- Limite de processadores da VM, em Desempenho > Processamento: grava `processors` no `.wslconfig`, para o
  próximo início ou com reinício.
- Devolução automática do cache de arquivos com dois gatilhos ajustáveis, em Desempenho > Memória: RAM
  livre do Windows abaixo de uma porcentagem (10% por padrão) ou cache da VM acima de um
  tamanho.

### Mudado

- Desempenho ganhou três abas, **Processamento**, **Memória** e **Disco**, e cada uma só mostra
  o que é dela: o teto de RAM e a devolução do cache ficam em Memória, o limite de
  processadores em Processamento, e a tabela por distro foi dividida entre as duas. O aviso de
  RAM acabando aparece só em Memória; a entrada Desempenho no menu e a aba Memória ganham um
  sinal enquanto ele estiver ativo. As abas mudam com as setas do teclado.
- A devolução automática do cache saiu de Configurações > Este app e foi para Desempenho >
  Memória.
  Quem já a tinha ligada continua com ela ligada e com o gatilho de 10%.

### Corrigido

- O aviso e a notificação de RAM acabando mandavam baixar o limite do WSL mesmo quando a maior
  parte estava com os programas do Windows. Medido: 31,7 GB de RAM, o WSL com 8,9 GB e o
  resto com Chrome e outros programas. Agora o WSL só é apontado quando ocupa mais que o resto
  do Windows somado.

- Os campos de Configurações aceitavam qualquer texto. Agora números e tamanhos só aceitam
  dígitos, o tamanho tem a unidade (MB ou GB) numa lista, e cada campo mostra a faixa
  aceita a partir da máquina: `memory` de 1 GB até a RAM do Windows, `processors` até os
  processadores lógicos, `swap` até o espaço livre do disco onde ela fica. Tempos mostram o
  valor em segundos, e lista de portas, IP, módulos do kernel, nome de máquina e usuário do
  Linux são conferidos pelo formato. O backend recusa também memória e processadores fora do
  limite.
- A descrição do `debugConsole` diz que ele abre a janela "WSL Debug Console" a cada subida da
  VM, e a confirmação de reiniciar o WSL avisa quando ele está ligado.
- No modo demonstração, o caminho do `.wslconfig` aparecia com caracteres trocados.
- Devolver o cache numa VM com muita escrita pendente levava o `sync` a passar de 30 s, e o app
  avisava que o WSL tinha parado de responder. A devolução agora espera até 5 min e não conta
  como queda.
- A leitura do Docker falhava inteira quando um container sumia entre a lista e o `inspect`,
  como acontece com os de CI. Agora o que sumiu é ignorado.
- Depois de uma devolução automática que falhava, a seguinte era tentada 5 s depois. Agora o
  intervalo de 10 minutos conta a partir de cada tentativa.

## [0.8.1] - 2026-10-29

### Corrigido

- Com o app aberto, uma versão nova só aparecia em até 6 horas ou ao reabrir. Agora ele procura
  a cada hora e também quando você volta para a janela, se a última busca tiver mais de 15
  minutos.

## [0.8.0] - 2026-10-29

### Adicionado

- **Devolver cache agora**, em Desempenho: esvazia o cache da VM e mostra quanto voltou para o
  Windows 15 s e 2 min depois, também registrado na Atividade. Medido numa máquina com Docker e
  CI rodando: de 10,0 GB para 7,8 GB em 15 s, e 7,6 GB 2 min depois.
- Opção em Configurações > Este app para devolver o cache sozinho quando o Windows fica abaixo
  de 10% de memória livre, no máximo uma vez a cada 10 minutos. Vem desligada.
- Quando o cache passa do uso real e o teto da VM passa da sugestão, Desempenho mostra em
  destaque quanto a VM devolveria com o limite sugerido, com o botão de ajustar.

### Alterado

- O cache da VM aparece em três partes: tabelas de arquivos percorridos, blocos de disco e
  arquivos lidos. Numa máquina medida, a maior delas eram as tabelas, e não o conteúdo de
  arquivos, como o rótulo único "cache de arquivos" dava a entender.

## [0.7.0] - 2026-10-29

### Adicionado

- **Para onde vai a memória**, em Desempenho: quanto a VM do WSL ocupa no Windows ao lado dos
  outros programas, e, dentro da VM, quanto é de programas e quanto é cache de arquivos.
- **Docker por projeto**, em Desempenho: memória e containers ligados de cada projeto do
  Docker Compose, lidos a cada 15 s, com botão para parar ou ligar o projeto inteiro. Um
  container que reinicia sem parar aparece em destaque e vai para a Atividade.
- Aviso e notificação do Windows quando a memória disponível do Windows cai abaixo de 10%,
  dizendo quanto o WSL ocupa e qual projeto mais usa. O aviso some acima de 15%.
- Aviso e notificação quando o WSL para de responder, com o botão para desligá-lo.
- **Processos presos**, na aba Processos: o que está há mais de 1 min esperando disco ou rede
  sem poder ser interrompido, o zumbi que o processo pai não recolheu em 1 min (com o PID do
  pai, que é quem precisa ser encerrado) e o que usa um núcleo inteiro há mais de 5 min. Cada
  um vai uma vez para a Atividade.
- **Containers com problema**, no cartão do Docker: o que falha no próprio healthcheck e o que
  passa de 90% do limite de memória, com botão para reiniciar só esses. Healthcheck falhando
  e reinício em ciclo também viram notificação do Windows.
- Configurações > Este app tem um interruptor para desligar a busca de atualizações, o único
  acesso do app à internet, e um botão para procurar na hora.
- O instalador mostra, antes de instalar, o que o app acessa na rede e o que muda no sistema.
- README com as seções de privacidade e de política de assinatura, preparando o pedido de
  assinatura gratuita à SignPath Foundation.

### Alterado

- O cartão Memória, em Desempenho, chamava de "Em uso" só o que os programas usam dentro da
  VM, e podia mostrar 1 GB com o Gerenciador de Tarefas mostrando 8,6 GB para o `vmmemWSL`.
  A diferença é cache de arquivos, que o Linux não conta como uso e o Windows cobra da VM. O
  cartão agora separa programas e kernel, cache e quanto a VM ocupa na RAM do Windows.

### Corrigido

- Desligar o WSL esperava no máximo 90 s. Com o WSL travado, o `wsl --shutdown` levou 117 s,
  e o app desistia antes de ele terminar. Agora espera até 3 minutos.

## [0.6.0] - 2026-10-29

### Segurança

- O agente passa a morar em `/var/lib/toca-do-pinguim/`, numa pasta 700 do root, com o
  arquivo temporário criado pelo `mktemp`. Em `/tmp`, um usuário comum da distro podia criar
  a pasta antes do app e plantar um binário que o app executaria como root.
- A chave que assina as atualizações sai do build: o instalador é montado sem ela, e um job
  separado, no ambiente protegido `publicacao`, só assina. Todas as Actions são fixadas por
  commit, e a tag de versão precisa estar na `main`.
- Workflow **Segurança**, a cada push, PR e toda segunda-feira: `cargo-deny` (base RustSec,
  licenças, crates e origem) para o app no alvo Windows e o agente no Linux, `npm audit`,
  `gitleaks` no histórico e revisão das dependências novas em PR. Workflow **CodeQL** para
  Rust, TypeScript e os próprios workflows.
- SvelteKit 3 e `@sveltejs/adapter-static` 4, que fecham os alertas de gravidade baixa do
  SvelteKit (redirecionamento, XSS e cookie). Nenhum deles alcançava o app, que é estático e
  não tem servidor. Fica o alerta do `deepmerge`, sem versão corrigida: ele só roda no build,
  para juntar a configuração do próprio projeto.
- O Dependabot só alerta, na aba Security: não abre PR. Toda mudança entra por commit e PR
  de quem mantém o projeto.

### Adicionado

- Arquivos de projeto aberto: `SECURITY.md` (relato privado de falha e onde o app tem
  privilégio), `CONTRIBUTING.md`, código de conduta e modelos de issue e de PR.
- Cada release e o zip portátil trazem o `THIRD-PARTY-NOTICES.md`, com a licença de cada
  biblioteca de terceiros incluída no app, gerado pelo `cargo-about` e por
  `scripts/avisos-de-licenca.mjs`.
- Configurações > Este app mostra a versão do app.
- README com capturas de tela e um resumo em inglês.

## [0.5.1] - 2026-10-29

### Corrigido

- Gravar a tecla de atalho deixava o atalho do menu Iniciar sem ícone, porque regravar o
  `.lnk` apagava o caminho do ícone. Agora ele aponta para o executável, e um ícone escolhido
  à mão é mantido.

## [0.5.0] - 2026-10-29

### Adicionado

- Instalar e remover pelo terminal: `scripts/instalar.ps1` baixa a versão mais recente,
  confere o SHA256 e instala sem perguntas; `scripts/desinstalar.ps1` fecha o app, remove e,
  com `-ApagarDados`, apaga histórico e log. Os dois rodam com `irm ... | iex`.
- O instalador registra o app em App Paths: `start toca-do-pinguim` e o Win+R o abrem. Grava
  também o comando de remoção silenciosa, e o editor aparece como SaLuanVitor.
- Instância única: abrir o app com ele já aberto só traz a janela para frente, em vez de subir
  uma segunda cópia com outro agente em cada distro.
- Em Configurações > Este app, a tecla de atalho do Windows que abre o app, gravada no atalho
  do menu Iniciar (criado se não existir, como na versão portátil).

## [0.4.0] - 2026-10-29

### Adicionado

- Página **Configurações** com as 42 chaves documentadas do `.wslconfig`, em sete grupos
  (Recursos, Disco, Rede, Desligamento, Memória, Kernel e Avançado). Cada chave mostra o
  padrão do WSL e marca as que só existem no Windows 11; campo vazio tira a chave do arquivo
  e volta ao padrão. Valores são conferidos antes de gravar, caminhos vão com as barras
  invertidas dobradas que o WSL pede, e chaves que o app não conhece aparecem e são
  mantidas. Salva só no arquivo ou salva e reinicia o WSL, com confirmação.
- Na mesma página, o seletor "Aplicar a" escolhe uma distro e edita o `/etc/wsl.conf` dela:
  as 19 chaves documentadas (systemd, comando ao ligar, usuário padrão, montagem dos discos do
  Windows, programas do Windows, rede, GPU e fuso). Ler o arquivo de uma distro desligada a
  liga, por isso só acontece com "Ligar e ler". Ao gravar, a versão anterior fica em
  `/etc/wsl.conf.toca-do-pinguim.bak`; "Salvar e reiniciar" termina só aquela distro.

## [0.3.0] - 2026-10-29

### Adicionado

- Página **Log**, com o log técnico gravado em `%LOCALAPPDATA%\com.luan.tocadopinguim\logs`
  (até 1 MB por arquivo, 5 rotações). Por padrão mostra só avisos e erros; copia o trecho
  visível e abre a pasta. Ficam registradas as falhas e a lentidão do `wsl.exe`, com o
  código de erro do WSL, os agentes que subiram, pararam ou não subiram, os pedidos feitos
  pelo app e a versão do WSL ao abrir.
- A Atividade registra quando o WSL deixa de responder e quando volta, com quanto tempo
  ficou fora e quantas chamadas falharam, e ganhou o filtro **WSL**.
- `TOCA_LOG=debug` grava no log também as chamadas que deram certo, com o tempo de cada uma.

### Corrigido

- Com o WSL travado (`Wsl/Service/0x8007274c`), o app tentava subir o agente a cada 30 s, e
  cada tentativa ocupava o serviço pelos 30 s do prazo. Agora a espera dobra a cada falha
  seguida, até 5 minutos.

## [0.2.0] - 2026-10-29

### Adicionado

- Atualização automática: o app instalado procura versão nova ao abrir e a cada 6 horas e
  mostra um aviso. Nada é baixado sem clique: "Atualizar e reiniciar" baixa, confere a
  assinatura e reabre o app na versão nova. Na versão portátil, o aviso leva à página da
  versão. Quem está na 0.1.0 precisa instalar a 0.2.0 à mão uma vez, porque a 0.1.0 não tem
  esse recurso.

## [0.1.0] - 2026-10-29

Primeira versão pública.

### Adicionado

- Aba **Processos**: processos de todas as distros com CPU e memória ao vivo, agrupados por
  distro ou em lista única, ordenação estável por qualquer coluna e encerramento por SIGTERM
  ou SIGKILL.
- Aba **Desempenho**: gráficos de CPU e memória da VM do WSL e ajuste do limite de memória
  no `.wslconfig`, com sugestão a partir do pico medido nos últimos 7 dias.
- Aba **Portas**: sockets em escuta, com o processo e a distro de cada um.
- Aba **Atividade**: distros que ligaram e desligaram, processos encerrados, mortes por falta
  de memória e mudanças de configuração.
- Aba **Distros**: iniciar, terminar e reiniciar distros e desligar o WSL.
- Busca única com Ctrl+K.
- O app observa sem segurar distro ligada: o agente sai quando a sessão dele é a única, e o
  WSL desliga a distro no tempo normal.
- Instalador para Windows 10 e 11 e versão portátil em `.zip`.

[Não lançado]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/SaLuanVitor/toca-do-pinguim/releases/tag/v0.1.0
