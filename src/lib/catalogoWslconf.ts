// Chaves do `/etc/wsl.conf` de cada distro, documentadas em
// learn.microsoft.com/windows/wsl/wsl-config (versão de 16/10/2026).

import type { Chave } from './catalogoWslconfig';

export const GRUPOS_WSLCONF = ['Inicialização', 'Usuário', 'Discos do Windows', 'Programas do Windows', 'Rede', 'GPU', 'Hora'] as const;

export const CATALOGO_WSLCONF: Chave[] = [
  { secao: 'boot', chave: 'systemd', grupo: 'Inicialização', tipo: 'bool', padrao: 'desligado', descricao: 'Usa o systemd como processo inicial da distro. Só no WSL 2.', win11: true },
  { secao: 'boot', chave: 'command', grupo: 'Inicialização', tipo: 'texto', padrao: 'nenhum', descricao: 'Comando rodado como root quando a distro liga, como service docker start.', win11: true },
  { secao: 'boot', chave: 'protectBinfmt', grupo: 'Inicialização', tipo: 'bool', padrao: 'ligado', descricao: 'Com systemd, impede o WSL de gerar as unidades de binfmt.', win11: true },
  { secao: 'boot', chave: 'initTimeout', grupo: 'Inicialização', tipo: 'numero', padrao: '10000', descricao: 'Milissegundos de espera pelo systemd iniciar.', win11: true },

  { secao: 'user', chave: 'default', grupo: 'Usuário', tipo: 'texto', padrao: 'o usuário criado na instalação', descricao: 'Usuário das sessões abertas na distro.' },

  { secao: 'automount', chave: 'enabled', grupo: 'Discos do Windows', tipo: 'bool', padrao: 'ligado', descricao: 'Monta os discos do Windows (C:, D:) sozinho ao ligar.' },
  { secao: 'automount', chave: 'root', grupo: 'Discos do Windows', tipo: 'caminhoLinux', padrao: '/mnt/', descricao: 'Pasta onde os discos aparecem: com /mnt/, o C: fica em /mnt/c.' },
  { secao: 'automount', chave: 'options', grupo: 'Discos do Windows', tipo: 'texto', padrao: 'nenhuma', descricao: 'Opções de montagem do DrvFs, como metadata,uid=1000,gid=1000,umask=022.' },
  { secao: 'automount', chave: 'mountFsTab', grupo: 'Discos do Windows', tipo: 'bool', padrao: 'ligado', descricao: 'Processa o /etc/fstab ao ligar.' },
  { secao: 'automount', chave: 'ldconfig', grupo: 'Discos do Windows', tipo: 'bool', padrao: 'ligado', descricao: 'Põe as bibliotecas de GPU do Windows no caminho do carregador e roda o ldconfig.' },
  { secao: 'automount', chave: 'cgroups', grupo: 'Discos do Windows', tipo: 'escolha', opcoes: ['v2', 'v1'], padrao: 'v2', descricao: 'Hierarquia de cgroup montada no WSL 2.' },

  { secao: 'interop', chave: 'enabled', grupo: 'Programas do Windows', tipo: 'bool', padrao: 'ligado', descricao: 'Permite rodar programas do Windows (.exe) de dentro da distro.' },
  { secao: 'interop', chave: 'appendWindowsPath', grupo: 'Programas do Windows', tipo: 'bool', padrao: 'ligado', descricao: 'Acrescenta o PATH do Windows ao $PATH da distro.' },

  { secao: 'network', chave: 'hostname', grupo: 'Rede', tipo: 'texto', padrao: 'o nome do Windows', descricao: 'Nome de máquina da distro.' },
  { secao: 'network', chave: 'generateHosts', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'O WSL gera o /etc/hosts.' },
  { secao: 'network', chave: 'generateResolvConf', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'O WSL gera o /etc/resolv.conf; desligado, você escreve o seu.' },

  { secao: 'gpu', chave: 'enabled', grupo: 'GPU', tipo: 'bool', padrao: 'ligado', descricao: 'Programas do Linux usam a GPU do Windows por paravirtualização.' },
  { secao: 'gpu', chave: 'appendLibPath', grupo: 'GPU', tipo: 'bool', padrao: 'ligado', descricao: 'Acrescenta /usr/lib/wsl/lib ao $PATH com a GPU ligada.' },

  { secao: 'time', chave: 'useWindowsTimezone', grupo: 'Hora', tipo: 'bool', padrao: 'ligado', descricao: 'Usa e acompanha o fuso horário do Windows.' },
];
