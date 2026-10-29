# Segurança

## Como relatar uma vulnerabilidade

Não abra issue pública. Use o relato privado do GitHub: aba **Security** do repositório,
**Report a vulnerability**. Só quem mantém o projeto vê o relato até a correção sair.

Ajuda incluir: a versão do app (aparece em Configurações > Este app e no começo do log), a
versão do WSL (`wsl --version`), os passos para reproduzir e o que um atacante consegue com a
falha.

## Versões cobertas

Só a versão mais recente, a que está em
[Releases](https://github.com/SaLuanVitor/toca-do-pinguim/releases/latest). Correções saem
numa versão nova, que o app instalado oferece pelo aviso de atualização.

## O que o app faz com privilégio

Para avaliar o impacto de uma falha, vale saber onde o app tem acesso:

- **Agente dentro das distros.** Um binário próprio é copiado para
  `/var/lib/toca-do-pinguim/` de cada distro ligada, numa pasta 700 do root, e roda como
  `root`, porque lê o `/proc` de todos os processos e envia sinais a pedido. Ele recusa sinal
  ao PID 1 e a si mesmo e só recebe comandos pelo stdin do `wsl.exe` aberto pelo app. Até a
  0.5.1 a pasta ficava em `/tmp`, onde um usuário comum podia criá-la antes do app e plantar
  um binário executado como root.
- **Arquivos de configuração.** O app grava `%UserProfile%\.wslconfig` e o `/etc/wsl.conf` de
  cada distro, como `root`, guardando a versão anterior ao lado. Valores com quebra de linha
  e seções fora das documentadas são recusados.
- **Atualização automática.** O instalador baixado só roda se a assinatura dele bater com a
  chave pública embutida no app. A chave privada fica fora do repositório, num secret do
  ambiente `publicacao` do GitHub, e só o job que assina a recebe, depois do build.
- **Cadeia de suprimentos.** As Actions são fixadas por commit. O workflow Segurança confere
  as dependências contra a base RustSec e o `npm audit`, as licenças e a origem das crates,
  procura segredos no histórico e, em PR, revisa as dependências novas; o CodeQL analisa o
  código. O Dependabot só alerta, na aba Security; atualizações entram por commit e PR de
  quem mantém o projeto.
- **Instalação.** É por usuário, em `%LOCALAPPDATA%\Toca do Pinguim`, sem pedir
  administrador. O instalador ainda não tem assinatura digital do Windows.
