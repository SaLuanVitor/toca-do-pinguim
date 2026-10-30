// Chaves do `.wslconfig` documentadas em learn.microsoft.com/windows/wsl/wsl-config
// (versão de 16/10/2026). Padrão e descrição seguem a documentação; o app não muda
// nenhum padrão, só grava a chave quando a pessoa escolhe um valor.

import type { Limites } from './tipos';

export type TipoChave =
  | 'bool'
  | 'tamanho'
  | 'numero'
  | 'texto'
  | 'caminho'
  | 'caminhoLinux'
  | 'escolha'
  | 'portas'
  | 'ip'
  | 'modulos'
  | 'hostname'
  | 'usuario';

/** Teto que depende da máquina: a RAM, os processadores ou o disco onde a swap fica. */
export type LimiteMaquina = 'ram' | 'processadores' | 'discoDaSwap';

export interface Chave {
  secao: string;
  chave: string;
  grupo: string;
  tipo: TipoChave;
  padrao: string;
  descricao: string;
  opcoes?: string[];
  /** Só existe no Windows 11 (e algumas pedem a versão 22H2). */
  win11?: boolean;
  /** Unidade mostrada ao lado do número: ms, processadores, despejos. */
  unidade?: string;
  /** Faixa do número, ou do tamanho em bytes. */
  min?: number;
  max?: number;
  maxMaquina?: LimiteMaquina;
}

const GIB = 1024 ** 3;
const MIB = 1024 ** 2;
/** Maior número que o WSL lê nas chaves de tempo, que são inteiros de 32 bits. */
const MAX_32 = 2_147_483_647;

export const GRUPOS = ['Recursos', 'Disco', 'Rede', 'Desligamento', 'Memória', 'Kernel', 'Avançado'] as const;

export const CATALOGO: Chave[] = [
  // Recursos
  { secao: 'wsl2', chave: 'memory', grupo: 'Recursos', tipo: 'tamanho', padrao: '50% da RAM do Windows', descricao: 'Memória máxima da VM do WSL.', min: GIB, maxMaquina: 'ram' },
  { secao: 'wsl2', chave: 'processors', grupo: 'Recursos', tipo: 'numero', padrao: 'todos os processadores lógicos', descricao: 'Quantos processadores lógicos a VM usa.', unidade: 'processadores', min: 1, maxMaquina: 'processadores' },
  { secao: 'wsl2', chave: 'swap', grupo: 'Recursos', tipo: 'tamanho', padrao: '25% da RAM, arredondado para cima em GB', descricao: 'Tamanho da swap da VM; 0 desliga a swap.', min: 0, maxMaquina: 'discoDaSwap' },
  { secao: 'wsl2', chave: 'swapFile', grupo: 'Recursos', tipo: 'caminho', padrao: '%Temp%\\swap.vhdx', descricao: 'Onde fica o disco virtual da swap. Mover para outro disco libera o C:.' },
  { secao: 'wsl2', chave: 'gpuSupport', grupo: 'Recursos', tipo: 'bool', padrao: 'ligado', descricao: 'Acesso da VM à GPU do Windows.' },
  { secao: 'wsl2', chave: 'guiApplications', grupo: 'Recursos', tipo: 'bool', padrao: 'ligado', descricao: 'Apps gráficos do Linux (WSLg).' },
  { secao: 'wsl2', chave: 'nestedVirtualization', grupo: 'Recursos', tipo: 'bool', padrao: 'ligado', descricao: 'Virtualização aninhada: rodar VMs dentro do WSL.', win11: true },

  // Disco
  { secao: 'wsl2', chave: 'defaultVhdSize', grupo: 'Disco', tipo: 'tamanho', padrao: '1 TB', descricao: 'Tamanho máximo do disco virtual de cada distro nova. Ele cresce conforme o uso, então pode passar do espaço livre de hoje.', min: GIB },
  { secao: 'experimental', chave: 'sparseVhd', grupo: 'Disco', tipo: 'bool', padrao: 'desligado', descricao: 'Discos virtuais novos devolvem ao Windows o espaço liberado dentro do Linux.' },
  { secao: 'general', chave: 'distributionInstallPath', grupo: 'Disco', tipo: 'caminho', padrao: '%LocalAppData%\\wsl', descricao: 'Pasta onde distros novas são instaladas.' },
  { secao: 'wsl2', chave: 'crashDumpFolder', grupo: 'Disco', tipo: 'caminho', padrao: '%Temp%\\wsl-crashes', descricao: 'Pasta dos despejos de falha do WSL.' },
  { secao: 'wsl2', chave: 'maxCrashDumpCount', grupo: 'Disco', tipo: 'numero', padrao: '10', descricao: 'Quantos despejos de falha guardar; os mais antigos são apagados.', unidade: 'despejos', min: 0, max: MAX_32 },
  { secao: 'wsl2', chave: 'virtiofs', grupo: 'Disco', tipo: 'bool', padrao: 'desligado', descricao: 'Experimental: VirtioFS para as pastas do Windows.' },

  // Rede
  { secao: 'wsl2', chave: 'networkingMode', grupo: 'Rede', tipo: 'escolha', opcoes: ['nat', 'mirrored', 'none', 'consomme', 'bridged'], padrao: 'nat', descricao: 'Modo de rede. mirrored espelha as interfaces do Windows; bridged está obsoleto desde o WSL 2.4.5.', win11: true },
  { secao: 'wsl2', chave: 'localhostForwarding', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Portas abertas no WSL respondem em localhost no Windows. Ignorado no modo mirrored.' },
  { secao: 'wsl2', chave: 'dnsTunneling', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Muda como o DNS do WSL passa pelo Windows.', win11: true },
  { secao: 'wsl2', chave: 'dnsProxy', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Só no modo nat: DNS do Linux aponta para o NAT do Windows; desligado, espelha os servidores DNS do Windows.' },
  { secao: 'wsl2', chave: 'firewall', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Regras do Firewall do Windows filtram o tráfego do WSL.', win11: true },
  { secao: 'wsl2', chave: 'autoProxy', grupo: 'Rede', tipo: 'bool', padrao: 'ligado', descricao: 'Usa o proxy HTTP configurado no Windows.', win11: true },
  { secao: 'experimental', chave: 'ignoredPorts', grupo: 'Rede', tipo: 'portas', padrao: 'nenhuma', descricao: 'Só no modo mirrored: portas que o Linux pode usar mesmo ocupadas no Windows, separadas por vírgula (3000,9000).', win11: true },
  { secao: 'experimental', chave: 'hostAddressLoopback', grupo: 'Rede', tipo: 'bool', padrao: 'desligado', descricao: 'Só no modo mirrored: Windows e Linux se alcançam pelos IPs do Windows, além de 127.0.0.1.', win11: true },
  { secao: 'experimental', chave: 'bestEffortDnsParsing', grupo: 'Rede', tipo: 'bool', padrao: 'desligado', descricao: 'Só com dnsTunneling: resolve a pergunta DNS ignorando registros desconhecidos.', win11: true },
  { secao: 'experimental', chave: 'dnsTunnelingIpAddress', grupo: 'Rede', tipo: 'ip', padrao: '10.255.255.254', descricao: 'Só com dnsTunneling: servidor DNS gravado no resolv.conf do Linux.', win11: true },
  { secao: 'experimental', chave: 'initialAutoProxyTimeout', grupo: 'Rede', tipo: 'numero', padrao: '1000', descricao: 'Só com autoProxy: quanto esperar pela configuração de proxy ao iniciar.', unidade: 'ms', min: 0, max: MAX_32, win11: true },

  // Desligamento
  { secao: 'general', chave: 'instanceIdleTimeout', grupo: 'Desligamento', tipo: 'numero', padrao: '15000', descricao: 'Tempo sem uso até a distro desligar; -1 nunca desliga.', unidade: 'ms', min: -1, max: MAX_32 },
  { secao: 'wsl2', chave: 'vmIdleTimeout', grupo: 'Desligamento', tipo: 'numero', padrao: '60000', descricao: 'Tempo sem nenhuma distro até a VM desligar.', unidade: 'ms', min: 0, max: MAX_32, win11: true },

  // Memória
  { secao: 'experimental', chave: 'autoMemoryReclaim', grupo: 'Memória', tipo: 'escolha', opcoes: ['dropCache', 'gradual', 'disabled'], padrao: 'dropCache', descricao: 'Como a VM devolve ao Windows a memória de cache: de uma vez, aos poucos ou nunca. Só age com a VM ociosa.' },

  // Kernel
  { secao: 'wsl2', chave: 'kernel', grupo: 'Kernel', tipo: 'caminho', padrao: 'kernel da Microsoft', descricao: 'Kernel Linux próprio, por caminho do Windows.' },
  { secao: 'wsl2', chave: 'kernelModules', grupo: 'Kernel', tipo: 'caminho', padrao: 'nenhum', descricao: 'Disco virtual com os módulos do kernel próprio.' },
  { secao: 'wsl2', chave: 'kernelCommandLine', grupo: 'Kernel', tipo: 'texto', padrao: 'nenhum', descricao: 'Argumentos extras da linha de comando do kernel.' },
  { secao: 'wsl2', chave: 'loadDefaultKernelModules', grupo: 'Kernel', tipo: 'bool', padrao: 'ligado', descricao: 'Carrega tun, ip_tables e br_netfilter ao subir a VM.' },
  { secao: 'wsl2', chave: 'loadKernelModules', grupo: 'Kernel', tipo: 'modulos', padrao: 'nenhum', descricao: 'Módulos extras a carregar, separados por vírgula.' },
  { secao: 'wsl2', chave: 'kernelBootTimeout', grupo: 'Kernel', tipo: 'numero', padrao: '30000', descricao: 'Espera pelo kernel subir.', unidade: 'ms', min: 0, max: MAX_32 },
  { secao: 'wsl2', chave: 'kernelDebugPort', grupo: 'Kernel', tipo: 'numero', padrao: '0', descricao: 'Porta do depurador do kernel; 0 desliga.', unidade: 'porta', min: 0, max: 65535 },

  // Avançado
  { secao: 'wsl2', chave: 'safeMode', grupo: 'Avançado', tipo: 'bool', padrao: 'desligado', descricao: 'Modo de segurança: desliga vários recursos para recuperar uma distro com problema.' },
  { secao: 'wsl2', chave: 'debugConsole', grupo: 'Avançado', tipo: 'bool', padrao: 'desligado', descricao: 'Abre a janela "WSL Debug Console", com o log do kernel, toda vez que a VM do WSL sobe, inclusive depois de o app reiniciá-lo.', win11: true },
  { secao: 'wsl2', chave: 'debugConsoleLogFile', grupo: 'Avançado', tipo: 'caminho', padrao: 'nenhum', descricao: 'Arquivo onde gravar a saída do console do kernel.' },
  { secao: 'wsl2', chave: 'distributionStartTimeout', grupo: 'Avançado', tipo: 'numero', padrao: '60000', descricao: 'Espera por uma distro subir.', unidade: 'ms', min: 0, max: MAX_32 },
  { secao: 'wsl2', chave: 'mountDeviceTimeout', grupo: 'Avançado', tipo: 'numero', padrao: '5000', descricao: 'Espera ao montar e desmontar discos.', unidade: 'ms', min: 0, max: MAX_32 },
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

const FATOR: Record<string, number> = { '': 1, B: 1, K: 1024, KB: 1024, M: MIB, MB: MIB, G: GIB, GB: GIB, T: 1024 * GIB, TB: 1024 * GIB };

/** Bytes de um tamanho como o WSL escreve (8GB, 512MB, 1T), ou null. */
export function tamanhoEmBytes(v: string): number | null {
  const m = /^(\d+(?:\.\d+)?)\s*([KMGT]?B?)$/i.exec(v.trim());
  if (!m) return null;
  return Math.round(Number(m[1]) * FATOR[m[2].toUpperCase()]);
}

/** O tamanho em número e unidade para a tela, só com MB e GB inteiros. */
export function partesDoTamanho(v: string): { numero: string; unidade: 'MB' | 'GB' } {
  const b = tamanhoEmBytes(v);
  if (b === null) return { numero: '', unidade: 'GB' };
  if (b % GIB === 0) return { numero: String(b / GIB), unidade: 'GB' };
  return { numero: String(Math.round(b / MIB)), unidade: 'MB' };
}

export interface Contexto {
  limites: Limites | null;
  /** Valores na tela agora, para regras que dependem de outra chave (a swap e o disco dela). */
  edicao: Record<string, string>;
}

const gb = (b: number) => `${(b / GIB).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} GB`;

/** Letra do disco onde a swap fica: a do swapFile escolhido, ou o C:, onde fica o %Temp%. */
function discoDaSwap(edicao: Record<string, string>): string {
  const caminho = caminhoDoArquivo(edicao['wsl2.swapfile'] ?? '').trim();
  const m = /^([A-Za-z]):/.exec(caminho);
  return (m?.[1] ?? 'C').toUpperCase();
}

/** Teto e o motivo dele, quando a máquina limita a chave. */
export function teto(c: Chave, ctx: Contexto): { valor: number; motivo: string } | null {
  const l = ctx.limites;
  if (c.maxMaquina === 'ram' && l) return { valor: l.ram_bytes, motivo: 'a RAM do Windows' };
  if (c.maxMaquina === 'processadores' && l) return { valor: l.processadores, motivo: 'os processadores lógicos da máquina' };
  if (c.maxMaquina === 'discoDaSwap' && l) {
    const letra = discoDaSwap(ctx.edicao);
    const disco = l.discos.find((d) => d.letra === letra);
    if (disco) return { valor: disco.livre_bytes, motivo: `o espaço livre no ${letra}:` };
  }
  return c.max !== undefined ? { valor: c.max, motivo: '' } : null;
}

/** Faixa aceita, escrita para a tela: "De 1 GB a 31,7 GB, a RAM do Windows". */
export function dica(c: Chave, ctx: Contexto): string | null {
  const t = teto(c, ctx);
  const fmt = (n: number) => (c.tipo === 'tamanho' ? gb(n) : n.toLocaleString('pt-BR'));
  if (c.min === undefined && !t) return null;
  const de = c.min !== undefined ? fmt(c.min) : null;
  // Nos tempos, o teto de 32 bits não é limite de verdade; em bytes, qualquer tamanho passa dele.
  if (t && c.tipo !== 'tamanho' && t.valor >= MAX_32) return de !== null ? `A partir de ${de}.` : null;
  if (de !== null && t) return `De ${de} a ${fmt(t.valor)}${t.motivo ? `, ${t.motivo}` : ''}.`;
  if (t) return `Até ${fmt(t.valor)}${t.motivo ? `, ${t.motivo}` : ''}.`;
  return `A partir de ${de}.`;
}

/** Máscara dos campos numéricos: só dígitos, e o sinal de menos no começo quando a chave aceita -1. */
export function mascaraNumero(texto: string, aceitaNegativo: boolean): string {
  const digitos = texto.replace(/\D/g, '').slice(0, 10);
  return aceitaNegativo && texto.trimStart().startsWith('-') ? `-${digitos}` : digitos;
}

/** "15 s" ou "2 min", para o valor em milissegundos fazer sentido. */
export function emSegundos(ms: number): string | null {
  if (!Number.isFinite(ms) || ms < 1000) return null;
  const s = ms / 1000;
  if (s < 120) return `${s.toLocaleString('pt-BR', { maximumFractionDigits: 1 })} s`;
  return `${(s / 60).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} min`;
}

/** Mensagem de erro, ou null se o valor serve. Vazio é sempre válido: volta ao padrão. */
export function validar(c: Chave, v: string, ctx: Contexto = { limites: null, edicao: {} }): string | null {
  const t = v.trim();
  if (t === '') return null;
  const limite = teto(c, ctx);
  switch (c.tipo) {
    case 'tamanho': {
      const b = tamanhoEmBytes(t);
      if (b === null) return 'Use um número com unidade, como 8GB ou 512MB.';
      if (c.min !== undefined && b < c.min) return `O mínimo é ${gb(c.min)}.`;
      if (limite && b > limite.valor) return `Passa de ${gb(limite.valor)}${limite.motivo ? `, ${limite.motivo}` : ''}.`;
      return null;
    }
    case 'numero': {
      if (!/^-?\d+$/.test(t)) return 'Use um número inteiro.';
      const n = Number(t);
      if (c.min !== undefined && n < c.min) return `O mínimo é ${c.min.toLocaleString('pt-BR')}.`;
      if (limite && n > limite.valor) return `O máximo é ${limite.valor.toLocaleString('pt-BR')}${limite.motivo ? `, ${limite.motivo}` : ''}.`;
      return null;
    }
    case 'caminho':
      return /^([A-Za-z]:\\|\\\\)/.test(t) ? null : 'Use um caminho completo do Windows, como D:\\WSL\\swap.vhdx.';
    case 'caminhoLinux':
      return t.startsWith('/') ? null : 'Use um caminho absoluto do Linux, como /mnt/.';
    case 'escolha':
      return c.opcoes?.includes(t) ? null : `Use uma destas opções: ${c.opcoes?.join(', ')}.`;
    case 'bool':
      return /^(true|false)$/i.test(t) ? null : 'Use true ou false.';
    case 'portas': {
      const portas = t.split(',').map((p) => p.trim());
      return portas.every((p) => /^\d+$/.test(p) && Number(p) >= 1 && Number(p) <= 65535)
        ? null
        : 'Use portas de 1 a 65535 separadas por vírgula, como 3000,9000.';
    }
    case 'ip':
      return /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.test(t) && t.split('.').every((p) => Number(p) <= 255)
        ? null
        : 'Use um endereço IPv4, como 10.255.255.254.';
    case 'modulos':
      return /^[A-Za-z0-9_-]+(\s*,\s*[A-Za-z0-9_-]+)*$/.test(t) ? null : 'Use nomes de módulo separados por vírgula, como vxlan,ip6_tables.';
    case 'hostname':
      return /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/.test(t)
        ? null
        : 'Use até 63 letras, números e hífens, sem hífen no começo ou no fim.';
    case 'usuario':
      return /^[a-z_][a-z0-9_-]{0,31}$/.test(t) ? null : 'Use um nome de usuário do Linux: minúsculas, números, _ e -, até 32.';
    default:
      return null;
  }
}
