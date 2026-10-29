# Gerenciador de WSL

Gerenciador de tarefas para o WSL, no formato do Gerenciador de Tarefas do Windows:
processos de cada distro com CPU e memória, encerramento por SIGTERM ou SIGKILL,
gráficos da VM e controle das distros (iniciar, terminar, reiniciar, desligar o WSL).

## Como funciona

O Windows enxerga todas as distros como um processo só (`vmmemWSL`). Os processos só
podem ser lidos por dentro, então o app copia um agente para cada distro em execução e
conversa com ele por stdin e stdout, uma linha JSON por mensagem.

| Parte | Onde | O que faz |
|---|---|---|
| `crates/protocolo` | as duas pontas | tipos das mensagens; o mesmo crate compila para Windows e Linux |
| `crates/agente` | dentro da distro | binário estático (musl) que lê `/proc` e aplica sinais; recusa o PID 1 |
| `src-tauri` | Windows | lista distros, instala e supervisiona os agentes, repassa amostras ao front |
| `src` | WebView2 | SvelteKit em modo SPA: abas Processos, Desempenho e Distros |

O agente vai para `/tmp/wsl-tm/agente-<hash>` dentro de cada distro, copiado pelo stdin
do `wsl.exe`. Ele sai quando o stdin fecha, então fechar o app encerra todos os agentes.

Enquanto o app está aberto, as distros em execução não desligam por ociosidade: o agente
conta como atividade. Com a janela minimizada, as amostras param de ir ao front e os
agentes passam a ler a cada 5 s.

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

## Medido em 2026-10-28

Windows 11, WSL 2.6.1, duas distros rodando (Ubuntu com 16 processos, docker-desktop
com 162), executável de release:

| Medida | Valor |
|---|---|
| Executável | 3,7 MB |
| Agente | 541 KB, cerca de 0,5 MB de memória, 1 a 20 ms por leitura conforme o número de processos |
| CPU da árvore do app, janela visível, 1 leitura/s | 3,9% de um núcleo |
| CPU da árvore do app, janela minimizada | 0,2% de um núcleo |
| Memória privada da árvore do app | 226 MB, quase toda do WebView2 |

Uma medição só, numa máquina; não é dado de campo.
