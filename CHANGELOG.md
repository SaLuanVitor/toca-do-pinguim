# Changelog

As mudanças de cada versão. O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e a numeração segue o [versionamento semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

- Página **Log**, com o log técnico gravado em `%LOCALAPPDATA%\com.luan.tocadopinguim\logs`
  (até 1 MB por arquivo, 5 rotações). Por padrão mostra só avisos e erros; copia o trecho
  visível e abre a pasta. Ficam registradas as falhas e a lentidão do `wsl.exe`, com o
  código de erro do WSL, os agentes que subiram, pararam ou não subiram, os pedidos feitos
  pelo app e a versão do WSL ao abrir.
- A Atividade registra quando o WSL deixa de responder e quando volta, com quanto tempo
  ficou fora e quantas chamadas falharam, e ganhou o filtro **WSL**.

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

[Não lançado]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/SaLuanVitor/toca-do-pinguim/releases/tag/v0.1.0
