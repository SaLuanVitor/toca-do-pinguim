// Chaves do `.wslconfig` documentadas em learn.microsoft.com/windows/wsl/wsl-config
// (versão de 16/10/2026). Padrão e descrição seguem a documentação; o app não muda
// nenhum padrão, só grava a chave quando a pessoa escolhe um valor.

export type TipoChave = 'bool' | 'tamanho' | 'numero' | 'texto' | 'caminho' | 'escolha';

export interface Chave {
  secao: 'wsl2' | 'experimental' | 'general';
  chave: string;
  grupo: string;
  tipo: TipoChave;
  padrao: string;
  descricao: string;
  opcoes?: string[];
  /** Só existe no Windows 11 (e algumas pedem a versão 22H2). */
  win11?: boolean;
}

export const GRUPOS = ['Recursos', 'Disco', 'Rede', 'Desligamento', 'Memória', 'Kernel', 'Avançado'] as const;

export const CATALOGO: Chave[] = [
  // Recursos
  { secao: 'wsl2', chave: 'memory', grupo: 'Recursos', tipo: 'tamanho', padrao: '50% da RAM do Windows', descricao: 'Memória máxima da VM do WSL.' },
  { secao: 'wsl2', chave: 'processors', grupo: 'Recursos', tipo: 'numero', padrao: 'todos os processadores lógicos', descricao: 'Quantos processadores lógicos a VM usa.' },
  { secao: 'wsl2', chave: 'swap', grupo: 'Recursos', tipo: 'tamanho', padrao: '25% da RAM, arredondado para cima em GB', descricao: 'Tamanho da swap da VM; 0 desliga a swap.' },
  { secao: 'wsl2', chave: 'swapFile', grupo: 'Recursos', tipo: 'caminho', padrao: '%Temp%\\swap.vhdx', descricao: 'Onde fica o disco virtual da swap. Mover para outro disco libera o C:.' },
  { secao: 'wsl2', chave: 'gpuSupport', grupo: 'Recursos', tipo: 'bool', padrao: 'ligado', descricao: 'Acesso da VM à GPU do Windows.' },
  { secao: 'wsl2', chave: 'guiApplications', grupo: 'Recursos', tipo: 'bool', padrao: 'ligado', descricao: 'Apps gráficos do Linux (WSLg).' },
  { secao: 'wsl2', chave: 'nestedVirtualization', grupo: 'Recursos', tipo: 'bool', padrao: 'ligado', descricao: 'Virtualização aninhada: rodar VMs dentro do WSL.', win11: true },

  // Disco
  { secao: 'wsl2', chave: 'defaultVhdSize', grupo: 'Disco', tipo: 'tamanho', padrao: '1 TB', descricao: 'Tamanho máximo do disco virtual de cada distro nova.' },
  { secao: 'experimental', chave: 'sparseVhd', grupo: 'Disco', tipo: 'bool', padrao: 'desligado', descricao: 'Discos virtuais novos devolvem ao Windows o espaço liberado dentro do Linux.' },
  { secao: 'general', chave: 'distributionInstallPath', grupo: 'Disco', tipo: 'caminho', padrao: '%LocalAppData%\\wsl', descricao: 'Pasta onde distros novas são instaladas.' },
  { secao: 'wsl2', chave: 'crashDumpFolder', grupo: 'Disco', tipo: 'caminho', padrao: '%Temp%\\wsl-crashes', descricao: 'Pasta dos despejos de falha do WSL.' },
  { secao: 'wsl2', chave: 'maxCrashDumpCount', grupo: 'Disco', tipo: 'numero', padrao: '10', descricao: 'Quantos despejos de falha guardar; os mais antigos são apagados.' },
  { secao: 'wsl2', chave: 'virtiofs', grupo: 'Disco', tipo: 'bool', padrao: 'desligado', descricao: 'Experimental: VirtioFS para as pastas do Windows.' },

  // Rede
  { secao: 'wsl2', chave: 'networkingMode', grupo: 'Rede', tipo: 'escolha', opcoes: ['nat', 'mirrored', 'none', 'consomme', 'bridged'], padrao: 'nat', descricao: 'Modo de rede. mirrored espelha as interfaces do Windows; bridged está obsoleto desde o WSL 2.4.5.', win11: true },
  { secao: 'wsl2', chave: 'localhostForwarding', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Portas abertas no WSL respondem em localhost no Windows. Ignorado no modo mirrored.' },
  { secao: 'wsl2', chave: 'dnsTunneling', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Muda como o DNS do WSL passa pelo Windows.', win11: true },
  { secao: 'wsl2', chave: 'dnsProxy', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Só no modo nat: DNS do Linux aponta para o NAT do Windows; desligado, espelha os servidores DNS do Windows.' },
  { secao: 'wsl2', chave: 'firewall', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Regras do Firewall do Windows filtram o tráfego do WSL.', win11: true },
  { secao: 'wsl2', chave: 'autoProxy', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Usa o proxy HTTP configurado no Windows.', win11: true },
  { secao: 'experimental', chave: 'ignoredPorts', grupo: 'Rede', tipo: 'texto', padrao: 'nenhuma', descricao: 'Só no modo mirrored: portas que o Linux pode usar mesmo ocupadas no Windows, separadas por vírgula (3000,9000).', win11: true },
  { secao: 'experimental', chave: 'hostAddressLoopback', grupo: 'Rede', tipo: 'bool', padrao: 'desligado', descricao: 'Só no modo mirrored: Windows e Linux se alcançam pelos IPs do Windows, além de 127.0.0.1.', win11: true },
  { secao: 'experimental', chave: 'bestEffortDnsParsing', grupo: 'Rede', tipo: 'bool', padrao: 'desligado', descricao: 'Só com dnsTunneling: resolve a pergunta DNS ignorando registros desconhecidos.', win11: true },
  { secao: 'experimental', chave: 'dnsTunnelingIpAddress', grupo: 'Rede', tipo: 'texto', padrao: '10.255.255.254', descricao: 'Só com dnsTunneling: servidor DNS gravado no resolv.conf do Linux.', win11: true },
  { secao: 'experimental', chave: 'initialAutoProxyTimeout', grupo: 'Rede', tipo: 'numero', padrao: '1000', descricao: 'Só com autoProxy: quanto esperar, em ms, pela configuração de proxy ao iniciar.', win11: true },

  // Desligamento
  { secao: 'general', chave: 'instanceIdleTimeout', grupo: 'Desligamento', tipo: 'numero', padrao: '15000', descricao: 'Milissegundos sem uso até a distro desligar; -1 nunca desliga.' },
  { secao: 'wsl2', chave: 'vmIdleTimeout', grupo: 'Desligamento', tipo: 'numero', padrao: '60000', descricao: 'Milissegundos sem nenhuma distro até a VM desligar.', win11: true },

  // Memória
  { secao: 'experimental', chave: 'autoMemoryReclaim', grupo: 'Memória', tipo: 'escolha', opcoes: ['dropCache', 'gradual', 'disabled'], padrao: 'dropCache', descricao: 'Como a VM devolve ao Windows a memória de cache: de uma vez, aos poucos ou nunca.' },

  // Kernel
  { secao: 'wsl2', chave: 'kernel', grupo: 'Kernel', tipo: 'caminho', padrao: 'kernel da Microsoft', descricao: 'Kernel Linux próprio, por caminho do Windows.' },
  { secao: 'wsl2', chave: 'kernelModules', grupo: 'Kernel', tipo: 'caminho', padrao: 'nenhum', descricao: 'Disco virtual com os módulos do kernel próprio.' },
  { secao: 'wsl2', chave: 'kernelCommandLine', grupo: 'Kernel', tipo: 'texto', padrao: 'nenhum', descricao: 'Argumentos extras da linha de comando do kernel.' },
  { secao: 'wsl2', chave: 'loadDefaultKernelModules', grupo: 'Kernel', tipo: 'bool', padrao: 'ligado', descricao: 'Carrega tun, ip_tables e br_netfilter ao subir a VM.' },
  { secao: 'wsl2', chave: 'loadKernelModules', grupo: 'Kernel', tipo: 'texto', padrao: 'nenhum', descricao: 'Módulos extras a carregar, separados por vírgula.' },
  { secao: 'wsl2', chave: 'kernelBootTimeout', grupo: 'Kernel', tipo: 'numero', padrao: '30000', descricao: 'Milissegundos de espera pelo kernel subir.' },
  { secao: 'wsl2', chave: 'kernelDebugPort', grupo: 'Kernel', tipo: 'numero', padrao: '0', descricao: 'Porta do depurador do kernel; 0 desliga.' },

  // Avançado
  { secao: 'wsl2', chave: 'safeMode', grupo: 'Avançado', tipo: 'bool', padrao: 'desligado', descricao: 'Modo de segurança: desliga vários recursos para recuperar uma distro com problema.' },
  { secao: 'wsl2', chave: 'debugConsole', grupo: 'Avançado', tipo: 'bool', padrao: 'desligado', descricao: 'Abre uma janela com o dmesg ao iniciar uma distro.', win11: true },
  { secao: 'wsl2', chave: 'debugConsoleLogFile', grupo: 'Avançado', tipo: 'caminho', padrao: 'nenhum', descricao: 'Arquivo onde gravar a saída do console do kernel.' },
  { secao: 'wsl2', chave: 'distributionStartTimeout', grupo: 'Avançado', tipo: 'numero', padrao: '60000', descricao: 'Milissegundos de espera por uma distro subir.' },
  { secao: 'wsl2', chave: 'mountDeviceTimeout', grupo: 'Avançado', tipo: 'numero', padrao: '5000', descricao: 'Milissegundos de espera ao montar e desmontar discos.' },
  { secao: 'wsl2', chave: 'isolateDistroCgroup', grupo: 'Avançado', tipo: 'bool', padrao: 'ligado', descricao: 'Isola cada distro no próprio cgroup; exige cgroup v2.' },
  { secao: 'wsl2', chave: 'systemDistro', grupo: 'Avançado', tipo: 'caminho', padrao: 'a que vem com o WSL', descricao: 'Imagem própria da distro de sistema (.img ou .vhd).' },
  { secao: 'wsl2', chave: 'telemetry', grupo: 'Avançado', tipo: 'bool', padrao: 'ligado', descricao: 'Envio de telemetria de diagnóstico do WSL.' },
];

export function idChave(c: { secao: string; chave: string }): string {
  return `${c.secao.toLowerCase()}.${c.chave.toLowerCase()}`;
}

/** O `.wslconfig` pede barras invertidas dobradas nos caminhos (`C:\\Temp`). */
export function caminhoParaArquivo(v: string): string {
  return v.replace(/\\\\/g, '\\').replace(/\\/g, '\\\\');
}

export function caminhoDoArquivo(v: string): string {
  return v.replace(/\\\\/g, '\\');
}

/** Mensagem de erro, ou null se o valor serve. Vazio é sempre válido: volta ao padrão. */
export function validar(c: Chave, v: string): string | null {
  const t = v.trim();
  if (t === '') return null;
  switch (c.tipo) {
    case 'tamanho':
      return /^\d+(\.\d+)?\s*(B|KB|MB|GB|TB|K|M|G|T)?$/i.test(t) ? null : 'Use um número com unidade, como 8GB ou 512MB.';
    case 'numero':
      return /^-?\d+$/.test(t) ? null : 'Use um número inteiro.';
    case 'caminho':
      return /^([A-Za-z]:\\|\\\\)/.test(t) ? null :'Use um caminho completo do Windows, como D:\\WSL\\swap.vhdx.';
    case 'escolha':
      return c.opcoes?.includes(t) ? null : `Use uma destas opções: ${c.opcoes?.join(', ')}.`;
    case 'bool':
      return /^(true|false)$/i.test(t) ? null : 'Use true ou false.';
    default:
      return null;
  }
}
