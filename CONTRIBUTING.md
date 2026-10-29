# Como contribuir

Relatos de bug, ideias e PRs são bem-vindos. Para falha de segurança, siga o
[SECURITY.md](SECURITY.md) em vez de abrir issue.

## Antes de começar

- **Bug:** abra uma issue pelo modelo de bug, com a versão do app, a saída de
  `wsl --version` e o trecho da página Log em que o erro aparece.
- **Funcionalidade:** abra uma issue antes do PR, para combinar o caminho. O app tem um
  princípio que vale para toda mudança: ele observa e gerencia, mas nunca mantém uma distro
  ligada por conta própria.

## Ambiente

Windows 10 ou 11 com WSL 2, Rust estável com o alvo `x86_64-unknown-linux-musl` e Node 24.

```bash
rustup target add x86_64-unknown-linux-musl
npm install
npm run tauri dev
```

`npm run dev` sozinho abre a interface no navegador, em modo demonstração, com dados
simulados: serve para mexer na tela sem WSL.

A primeira página depois de subir o servidor de desenvolvimento demora mais que as outras,
porque o SvelteKit 3 passa o runtime dele pelo Vite nessa hora. Com a máquina ocupada, isso
passou de 60 s e a página voltou com erro 500; recarregar resolve.

## Testes

| O que mudou | Rode |
|---|---|
| App (Rust, `src-tauri/`) | `cargo test -p toca-do-pinguim` |
| Agente (`crates/agente`, `crates/protocolo`) | `cargo test -p agente -p protocolo`, em Linux; no Windows o CI roda |
| Interface (`src/`) | `npm run check` |

O workflow **Verificar** roda os três a cada push e PR. O **Segurança** confere as
dependências (RustSec, `npm audit`, licenças e origem), procura segredos e, em PR, revisa as
dependências novas; o **CodeQL** analisa o código. Os três precisam passar para o PR entrar.

## Commits e PRs

- Crie a branch a partir de `develop`, com o prefixo `feat/`, `fix/`, `docs/`, `ci/` ou
  `chore/`, e abra o PR para `develop`. A `main` só recebe PR de `develop`, na hora de uma
  versão.
- Mensagens no padrão [Conventional Commits](https://www.conventionalcommits.org/pt-br/), em
  português: `feat:`, `fix:`, `docs:`, `ci:`, `build:`, `chore:`.
- Um PR por assunto. Diga no PR o que mudou para quem usa e como você conferiu.
- Mudança visível para quem usa entra no `CHANGELOG.md`, em "Não lançado".
- Textos da interface e da documentação em português do Brasil.

## Publicar uma versão

Só quem mantém o projeto publica. O passo a passo está no README, em "Publicar uma versão".

## Licença

Ao enviar um PR, você concorda que a sua contribuição seja distribuída sob a
[licença MIT](LICENSE) do projeto.
