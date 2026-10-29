# Toca do Pinguim

Gerenciador de tarefas para o WSL, no formato do Gerenciador de Tarefas do Windows. Ele
observa e gerencia sem mudar o ciclo de vida das distros: nenhuma fica ligada por causa
do app.

| Aba | O que tem |
|---|---|
| Processos | processos de cada distro com CPU e memória; encerramento por SIGTERM ou SIGKILL |
| Desempenho | gráficos de CPU e memória da VM; limite de memória com pico medido e sugestão |
| Portas | sockets em escuta na VM, o processo e a distro de cada um |
| Atividade | distros que ligaram e desligaram, processos encerrados, falta de memória, configuração |
| Distros | iniciar, terminar, reiniciar e desligar o WSL |

Ctrl+K abre uma busca única por processo, porta, distro ou ação.

## Instalar

Cada versão em [Releases](https://github.com/SaLuanVitor/toca-do-pinguim/releases) traz o
instalador (`_x64-setup.exe`), uma versão portátil (`_x64-portable.zip`) e o
`SHA256SUMS.txt` para conferir o download. Requer Windows 10 ou 11 x64 com WSL 2. Ainda
não há assinatura digital, então o SmartScreen avisa na primeira execução: "Mais
informações" e depois "Executar assim mesmo". O que mudou em cada versão está no
[CHANGELOG](CHANGELOG.md).

Instalado pelo instalador, o app avisa quando sai versão nova e se atualiza com um clique.
Ele procura ao abrir e a cada 6 horas, e não baixa nada sem você pedir. O instalador baixado
é conferido contra uma assinatura própria do projeto antes de rodar.

## Como funciona

O Windows enxerga todas as distros como um processo só (`vmmemWSL`). Os processos só
podem ser lidos por dentro, então o app copia um agente para cada distro em execução e
conversa com ele por stdin e stdout, uma linha JSON por mensagem.

| Parte | Onde | O que faz |
|---|---|---|
| `crates/protocolo` | as duas pontas | tipos das mensagens; o mesmo crate compila para Windows e Linux |
| `crates/agente` | dentro da distro | binário estático (musl) que lê `/proc` e aplica sinais; recusa o PID 1 |
| `src-tauri` | Windows | lista distros, supervisiona os agentes, guarda histórico e atividade, edita o `.wslconfig` |
| `src` | WebView2 | SvelteKit em modo SPA |

O agente vai para `/tmp/toca-do-pinguim/agente-<hash>` dentro de cada distro, copiado pelo stdin
do `wsl.exe`. Ele sai quando o stdin fecha, então fechar o app encerra todos os agentes.
Além dos processos, ele lê as portas em escuta a cada 5 amostras e as mortes por falta de
memória no `/dev/kmsg`.

**Ociosidade.** O WSL desliga uma distro quando não sobra sessão aberta nela (cada
`wsl.exe` conectado aparece lá dentro como um `SessionLeader`). Medido no WSL 2.6.1 numa
Debian com systemd: com `cron`, `dbus` e `journald` rodando, ela desligou 16 s depois que
a última sessão fechou. O agente roda numa sessão, então sai sozinho quando a dele é a
única, e o app espera o `instanceIdleTimeout` (padrão 15 s, lido do `.wslconfig`) mais 5 s
antes de subir outro. Se a distro continua ligada depois disso, algo além do app a segura.
Toda chamada ao `wsl.exe` tem prazo, porque uma sessão presa também segura a distro.

Não foi medido se um processo solto da sessão (`setsid`) segura uma distro sem systemd: o
único teste disso coincidiu com o serviço do WSL parando de responder.

Com a janela minimizada, as amostras param de ir ao front e os agentes passam a ler a
cada 5 s.

**Dados.** Em `%APPDATA%\com.luan.tocadopinguim`: `memoria.csv` (pico de memória usada por
minuto, 30 dias) e `atividade.jsonl` (últimos 1000 eventos). Na primeira abertura, os dois
são copiados de `%APPDATA%\com.luan.wsltm`, a pasta de quando o app se chamava Gerenciador
de WSL; a pasta antiga fica como está. A sugestão de limite é o pico
dos últimos 7 dias com 25% de folga, arredondado para cima em GB, e só aparece depois de
uma hora de dados. Gravar o limite troca só a linha `memory` do `.wslconfig` e guarda a
versão anterior em `.wslconfig.toca-do-pinguim.bak`.

## Desenvolvimento

Requisitos: Rust com o alvo `x86_64-unknown-linux-musl` e Node.

```bash
rustup target add x86_64-unknown-linux-musl
npm install
npm run tauri dev
```

O agente é compilado no Windows com o `rust-lld` (ver `.cargo/config.toml`), sem
toolchain dentro da distro. `npm run tauri dev` e `npm run tauri build` compilam o agente
antes do app, porque o binário dele é embutido no executável.

Aberto num navegador comum (`npm run dev`), o front roda em modo demonstração, com
dados simulados e um aviso na tela.

## Publicar uma versão

A versão mora no `package.json`. O `tauri.conf.json` lê de lá, e o
`scripts/sincronizar-versao.mjs` copia o número para o `Cargo.toml` e o `Cargo.lock`.

1. Em `CHANGELOG.md`, mova o que está em "Não lançado" para uma seção nova da versão e faça
   o commit; o `npm version` recusa árvore com mudança pendente.
2. Rode `npm version patch`, `minor` ou `major`. O comando sincroniza a versão nos arquivos
   do Rust, faz o commit `chore: versão X.Y.Z` e cria a tag `vX.Y.Z`.
3. Envie o commit e a tag: `git push --follow-tags`.
4. O workflow Publicar confere se a tag bate com a versão, gera os pacotes e abre uma release
   em rascunho com as notas do CHANGELOG. Revise e publique pela página de Releases.

Rodar o Publicar à mão, pela aba Actions, gera os mesmos pacotes como artefato, sem release.

A atualização automática só enxerga a release depois de publicada, porque lê o
`latest.json` de `releases/latest`. O Publicar assina o instalador com o secret
`TAURI_SIGNING_PRIVATE_KEY`; a chave pública correspondente está em `tauri.conf.json`.
**Perder a chave privada impede atualizar quem já instalou:** uma chave nova exige que todos
reinstalem à mão.

## Medido em 2026-10-28

Windows 11, WSL 2.6.1, duas distros rodando (Ubuntu com 16 processos, docker-desktop
com 162), executável de release:

| Medida | Valor |
|---|---|
| Executável | 3,8 MB |
| Agente | cerca de 0,5 MB de memória, 1 a 20 ms por leitura conforme o número de processos |
| CPU da árvore do app, janela visível, 1 leitura/s | 3,2% a 3,9% de um núcleo |
| CPU da árvore do app, janela minimizada | 0,2% de um núcleo |
| Memória privada da árvore do app | 226 a 273 MB, quase toda do WebView2 |
| App aberto, Debian ligada sem sessão | agente saiu 2 s depois; o WSL a desligou 16 s depois disso |
| App aberto, Debian com uma sessão de 40 s | agente ficou durante a sessão e saiu quando ela fechou; desligou 16 s depois |

Medições únicas, numa máquina; não são dado de campo.

## Licença

[MIT](LICENSE).
