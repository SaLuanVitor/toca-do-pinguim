# Changelog

As mudanças de cada versão. O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e a numeração segue o [versionamento semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

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

[Não lançado]: https://github.com/SaLuanVitor/toca-do-pinguim/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/SaLuanVitor/toca-do-pinguim/releases/tag/v0.1.0
