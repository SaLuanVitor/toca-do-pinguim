// Backend simulado para desenvolver a tela no navegador, sem Tauri e sem WSL.

import type { Api } from './api';
import { estado } from './estado.svelte';
import type { AnalisePastas, Armazenamento, Conteiner, Distro, Evento, Porta, Processo } from './tipos';

const GIB = 1024 ** 3;

// Porta, nome do processo e distro dona; sem distro, o socket é de outro lugar da VM.
const PORTAS: [number, string, string | null][] = [
  [80, 'nginx', 'docker-desktop'],
  [3000, 'node', 'Ubuntu'],
  [5432, 'postgres', 'Ubuntu'],
  [22, 'sshd', 'Ubuntu'],
  [1434, '', null],
];

// Nome, projeto do Compose e memória em MB; quatro projetos, como na máquina medida.
const CONTEINERES: [string, string | null, number][] = [
  ['loja-db', 'loja-online', 1763],
  ['loja-api', 'loja-online', 403],
  ['loja-redis', 'loja-online', 12],
  ['loja-nginx', 'loja-online', 9],
  ['loja-relatorios', 'loja-online', 478],
  ['bot-api-1', 'chat-bot', 648],
  ['bot-worker-1', 'chat-bot', 492],
  ['bot-db-1', 'chat-bot', 71],
  ['blog-api-1', 'blog', 273],
  ['blog-db-1', 'blog', 51],
  ['gitlab-runner', null, 28],
];

type Modelo = [nome: string, usuario: string, cmdline: string, carga: number, rssMb: number];

const MODELOS: Record<string, Modelo[]> = {
  Ubuntu: [
    ['init', 'root', '/init', 0, 1.2],
    ['init', 'root', 'plan9 --control-socket 6 --log-level 4', 0, 2.1],
    ['SessionLeader', 'root', '/init', 0, 0.8],
    ['Relay(10)', 'root', '/init', 0, 0.9],
    ['bash', 'luan', '-bash', 0.1, 5.4],
    ['node', 'luan', 'node /home/luan/app/server.js', 4, 182],
    ['postgres', 'postgres', '/usr/lib/postgresql/16/bin/postgres -D /var/lib/postgresql/16/main', 0.6, 38],
    ['postgres', 'postgres', 'postgres: checkpointer', 0.1, 9],
    ['postgres', 'postgres', 'postgres: walwriter', 0.1, 7],
    ['sshd', 'root', 'sshd: /usr/sbin/sshd -D [listener]', 0, 7.6],
    ['cron', 'root', '/usr/sbin/cron -f', 0, 2.8],
    ['vim', 'luan', 'vim notas.md', 0.2, 11],
    ['rsync', 'luan', 'rsync -a /mnt/e/backup/ /home/luan/backup/', 0, 24],
  ],
  'docker-desktop': [
    ['init', 'root', '/init', 0, 1.1],
    ['dockerd', 'root', '/usr/local/bin/dockerd --containerd /var/run/desktop-containerd/containerd.sock', 1.5, 96],
    ['containerd', 'root', 'containerd --config /etc/containerd/containerd.toml', 0.7, 41],
    ['containerd-shim', 'root', 'containerd-shim-runc-v2 -namespace moby -id 3f2a', 0.1, 9],
    ['containerd-shim', 'root', 'containerd-shim-runc-v2 -namespace moby -id 8c1d', 0.1, 9],
    ['nginx', 'root', 'nginx: master process nginx -g daemon off;', 0.2, 12],
    ['vpnkit-bridge', 'root', '/usr/bin/vpnkit-bridge guest', 0.3, 14],
  ],
  Debian: [
    ['init', 'root', '/init', 0, 1.1],
    ['bash', 'luan', '-bash', 0, 4.9],
  ],
};

export function criarDemonstracao(): Api {
  const distros: Distro[] = [
    { nome: 'Ubuntu', versao: 2, padrao: true, rodando: true },
    { nome: 'Debian', versao: 2, padrao: false, rodando: false },
    { nome: 'docker-desktop', versao: 2, padrao: false, rodando: true },
  ];
  const processos: Record<string, Processo[]> = {};
  let uptime = 7380;
  let cpuVm = 3;
  let intervalo = 1000;
  let seq = 0;
  let timer: ReturnType<typeof setInterval>;

  function ligar(nome: string) {
    let pid = 1;
    processos[nome] = (MODELOS[nome] ?? []).map(([n, usuario, cmdline, carga, rssMb]) => ({
      pid: pid === 1 ? pid++ : (pid += 3 + Math.floor(Math.random() * 40)),
      ppid: 1,
      usuario,
      estado: n === 'rsync' ? 'D' : carga > 1 ? 'R' : 'S',
      nome: n,
      cmdline,
      cpu_pct: carga,
      rss: rssMb * 1024 * 1024,
    }));
  }

  function publicar() {
    estado.receberDistros(distros.map((d) => ({ ...d })));
  }

  function registrar(tipo: Evento['tipo'], distro: string | null, texto: string) {
    estado.receberEvento({ quando_ms: Date.now(), tipo, distro, texto });
  }

  function portas(distro: string): Porta[] {
    return PORTAS.map(([porta, nome, dona]) => ({
      protocolo: 'tcp',
      endereco: porta === 22 ? '0.0.0.0' : '::',
      porta,
      pid: dona === distro ? (processos[distro].find((p) => p.nome === nome)?.pid ?? null) : null,
    }));
  }

  function tick() {
    uptime += intervalo / 1000;
    seq++;
    cpuVm = Math.max(0.5, Math.min(60, cpuVm + (Math.random() - 0.5) * 3));
    for (const d of distros.filter((d) => d.rodando)) {
      const lista = processos[d.nome];
      for (const p of lista) {
        const base = MODELOS[d.nome]?.find((m) => m[2] === p.cmdline)?.[3] ?? 0;
        p.cpu_pct = base === 0 ? 0 : Math.max(0, base + (Math.random() - 0.5) * base);
      }
      estado.receberAmostra(d.nome, {
        seq,
        uptime_s: uptime,
        cpus: 8,
        cpu_vm_pct: cpuVm,
        mem_total: 12 * 1024 ** 3,
        mem_disponivel: 7.2 * 1024 ** 3 - Math.random() * 200 * 1024 ** 2,
        mem_livre: 1.4 * 1024 ** 3,
        mem_blocos: 1.4 * 1024 ** 3,
        mem_arquivos: 2.0 * 1024 ** 3,
        mem_tabelas: 2.4 * 1024 ** 3,
        swap_total: 4 * 1024 ** 3,
        swap_livre: 3.9 * 1024 ** 3,
        processos: lista.map((p) => ({ ...p })),
        portas: seq % 5 === 1 ? portas(d.nome) : undefined,
        agente: { rss: 512 * 1024, cpu_pct: 0.1, coleta_us: 600 },
      });
    }
  }

  function agendar() {
    clearInterval(timer);
    timer = setInterval(tick, intervalo);
  }

  for (const d of distros.filter((d) => d.rodando)) ligar(d.nome);
  agendar();
  setInterval(() => publicarRecursos(), 5000);
  // Um processo preso esperando disco, como o vigia do backend o marcaria.
  const preso = processos.Ubuntu?.find((p) => p.nome === 'rsync');
  if (preso) {
    estado.receberSuspeitos('Ubuntu', [{ pid: preso.pid, ppid: 1, nome: preso.nome, motivo: 'aguardando_es', desde_ms: Date.now() - 3 * 60_000 }]);
  }
  setTimeout(tick, 50);

  const esperar = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const achar = (nome: string) => {
    const d = distros.find((x) => x.nome === nome);
    if (!d) throw `distro ${nome} não existe`;
    return d;
  };

  const conteineres: Conteiner[] = CONTEINERES.map(([nome, projeto, mb], i) => ({
    id: (i + 1).toString(16).padStart(64, 'a'),
    nome,
    projeto,
    estado: 'running',
    memoria_bytes: mb * 1024 ** 2,
    limite_bytes: nome === 'loja-relatorios' ? 512 * 1024 ** 2 : 0,
    reinicios: nome === 'bot-api-1' ? 20 : 0,
    em_ciclo: nome === 'bot-api-1',
    saude: nome === 'loja-api' ? 'unhealthy' : nome.includes('db') || nome.includes('mssql') ? 'healthy' : null,
  }));

  function publicarRecursos() {
    const wsl = 11.6 * GIB;
    const docker = distros.find((d) => d.nome === 'docker-desktop')?.rodando ?? false;
    const disponivel = (docker ? 2.7 : 9) * GIB + Math.random() * 100 * 1024 ** 2;
    estado.memoriaWindows = {
      total_bytes: 31.7 * GIB,
      disponivel_bytes: disponivel,
      commit_bytes: 42.3 * GIB,
      commit_limite_bytes: 56 * GIB,
      wsl_bytes: distros.some((d) => d.rodando) ? wsl : null,
      pressao: disponivel < 3.17 * GIB,
    };
    estado.docker = { ligado: docker, conteineres: docker ? conteineres.map((c) => ({ ...c })) : [], erro: null };
  }

  async function agirEm(ids: string[], ligar: boolean, rotulo: string) {
    await esperar(700);
    for (const c of conteineres.filter((c) => ids.includes(c.id))) {
      c.estado = ligar ? 'running' : 'exited';
      c.memoria_bytes = ligar ? (CONTEINERES.find(([n]) => n === c.nome)?.[2] ?? 0) * 1024 ** 2 : 0;
      c.em_ciclo = false;
    }
    publicarRecursos();
    registrar('docker', 'docker-desktop', `O app ${ligar ? 'ligou' : 'parou'} ${rotulo} (${ids.length} container(s))`);
  }

  const armazenamento: Armazenamento = {
    discos: [
      { nome: 'Ubuntu', caminho: String.raw`C:\Users\voce\AppData\Local\Packages\Ubuntu\LocalState\ext4.vhdx`, tamanho_bytes: 112 * GIB, usado_bytes: 81 * GIB, esparso: false, nota: null, compactavel: true },
      { nome: 'docker-desktop', caminho: String.raw`E:\DockerDesktopWSL\main\ext4.vhdx`, tamanho_bytes: 0.1 * GIB, usado_bytes: null, esparso: false, nota: null, compactavel: false },
      { nome: 'Docker (dados)', caminho: String.raw`E:\DockerDesktopWSL\disk\docker_data.vhdx`, tamanho_bytes: 199 * GIB, usado_bytes: 177 * GIB, esparso: false, nota: null, compactavel: true },
      {
        nome: 'Docker (dados, antigo)',
        caminho: String.raw`C:\Users\voce\AppData\Local\Docker\wsl\disk\docker_data.vhdx`,
        tamanho_bytes: 5.4 * GIB,
        usado_bytes: null,
        esparso: false,
        nota: String.raw`O Docker Desktop usa a pasta E:\DockerDesktopWSL; este arquivo ficou da pasta anterior e ele não o abre mais. Pode ser apagado com o Docker Desktop fechado.`,
        compactavel: false,
      },
      {
        nome: 'Debian',
        caminho: String.raw`D:\WSL\Debian\ext4.vhdx`,
        tamanho_bytes: 6.2 * GIB,
        usado_bytes: null,
        esparso: false,
        nota: null,
        compactavel: true,
      },
      {
        nome: 'Swap do WSL',
        caminho: String.raw`C:\Users\voce\AppData\Local\Temp\swap.vhdx`,
        tamanho_bytes: 4.1 * GIB,
        usado_bytes: null,
        esparso: false,
        nota: 'Arquivo no disco que a VM usa como RAM extra quando a RAM acaba. O WSL o recria a cada início, com o tamanho de swap do .wslconfig; para ocupar menos, baixe esse tamanho ou mova o arquivo com swapFile.',
        compactavel: false,
      },
    ],
    docker: {
      categorias: [
        { tipo: 'Images', total_bytes: 117 * GIB, recuperavel_bytes: 84.6 * GIB, quantidade: 117, ativos: 47 },
        { tipo: 'Containers', total_bytes: 14.2 * GIB, recuperavel_bytes: 14.1 * GIB, quantidade: 122, ativos: 6 },
        { tipo: 'Local Volumes', total_bytes: 55.9 * GIB, recuperavel_bytes: 10.6 * GIB, quantidade: 229, ativos: 56 },
        { tipo: 'Build Cache', total_bytes: 45 * GIB, recuperavel_bytes: 19.3 * GIB, quantidade: 715, ativos: 0 },
      ],
      volumes_sem_uso: [
        { nome: 'loja-online_db-antigo', bytes: 3.2 * GIB },
        { nome: 'chat-bot_kafka', bytes: 1.4 * GIB },
        { nome: 'blog_uploads', bytes: 0.6 * GIB },
        { nome: '4f2a91c0d7e3b8a6f5e4d3c2b1a09876', bytes: 0.2 * GIB },
      ],
      sobras: 104,
      sobras_bytes: 13.8 * GIB,
      projetos: [
        {
          nome: 'loja-online',
          containers: 5,
          parados: 0,
          containers_bytes: 0.3 * GIB,
          imagens_bytes: 2.4 * GIB,
          volumes: [
            { nome: 'loja-online_db', bytes: 6.1 * GIB, em_uso: true },
            { nome: 'loja-online_db-antigo', bytes: 3.2 * GIB, em_uso: false },
          ],
        },
        {
          nome: 'chat-bot',
          containers: 4,
          parados: 4,
          containers_bytes: 0.9 * GIB,
          imagens_bytes: 1.7 * GIB,
          volumes: [
            { nome: 'chat-bot_kafka', bytes: 1.4 * GIB, em_uso: false },
            { nome: 'chat-bot_redis', bytes: 0.2 * GIB, em_uso: true },
          ],
        },
        { nome: 'blog', containers: 2, parados: 2, containers_bytes: 0.1 * GIB, imagens_bytes: 0.6 * GIB, volumes: [{ nome: 'blog_uploads', bytes: 0.6 * GIB, em_uso: false }] },
      ],
    },
    erro_docker: null,
    limpeza: { cache_de_build: false, sobras_de_containers: false, tmp_dias: null, ultima_ms: null },
    distros_ligadas: ['Ubuntu'],
    distros: ['Debian', 'Ubuntu'],
  };
  const agoraMs = Date.now();
  const analiseDemo: AnalisePastas = {
    distro: 'Ubuntu',
    raiz: [
      { caminho: '/tmp', bytes: 53 * GIB, modificado_ms: null, em_uso_por: null, apagavel: false },
      { caminho: '/home', bytes: 17 * GIB, modificado_ms: null, em_uso_por: null, apagavel: false },
      { caminho: '/root', bytes: 4.4 * GIB, modificado_ms: null, em_uso_por: null, apagavel: false },
    ],
    tmp: [
      { caminho: '/tmp/loja-analise-20261028', bytes: 30 * GIB, modificado_ms: agoraMs - 2 * 86_400_000, em_uso_por: 'loja-api', apagavel: false },
      { caminho: '/tmp/loja-analise-02', bytes: 11 * GIB, modificado_ms: agoraMs - 3 * 86_400_000, em_uso_por: null, apagavel: true },
      { caminho: '/tmp/blog-correcao-20261023', bytes: 2.1 * GIB, modificado_ms: agoraMs - 7 * 86_400_000, em_uso_por: null, apagavel: true },
      { caminho: '/tmp/modelo-venv', bytes: 1 * GIB, modificado_ms: agoraMs - 9 * 86_400_000, em_uso_por: null, apagavel: true },
    ],
    caches: [
      { caminho: '/root/.cache', bytes: 4.4 * GIB, modificado_ms: null, em_uso_por: null, apagavel: true },
      { caminho: '/home/voce/.cache', bytes: 0.9 * GIB, modificado_ms: null, em_uso_por: null, apagavel: true },
      { caminho: '/var/cache/apt/archives', bytes: 0.3 * GIB, modificado_ms: null, em_uso_por: null, apagavel: true },
    ],
  };

  let limiteGb = 12;
  let teclaAtalho = '';
  let segundoPlano = { fechar_para_bandeja: true, iniciar_com_windows: false };
  const wslconfig: [string, string, string][] = [
    ['wsl2', 'memory', '12GB'],
    ['wsl2', 'networkingMode', 'mirrored'],
    ['experimental', 'sparseVhd', 'true'],
    ['wsl2', 'algoQueOAppNaoConhece', '1'],
  ];
  const wslconf: Record<string, [string, string, string][]> = {
    Ubuntu: [
      ['boot', 'systemd', 'true'],
      ['user', 'default', 'luan'],
    ],
  };
  const agora = Date.now();
  const eventosIniciais: Evento[] = [
    { quando_ms: agora - 3 * 3600_000, tipo: 'distro', distro: 'Ubuntu', texto: 'Ubuntu foi ligada' },
    { quando_ms: agora - 2 * 3600_000, tipo: 'memoria', distro: null, texto: 'Faltou memória na VM e o kernel encerrou node', detalhe: 'Out of memory: Killed process 48211 (node) total-vm:4120332kB' },
    { quando_ms: agora - 40 * 60_000, tipo: 'processo', distro: 'Ubuntu', texto: 'O app pediu o encerramento de vim (PID 265)' },
    { quando_ms: agora - 10 * 60_000, tipo: 'distro', distro: 'Debian', texto: 'Debian ficou sem atividade; o WSL deve desligá-la em cerca de 15 s' },
    { quando_ms: agora - 9 * 60_000, tipo: 'distro', distro: 'Debian', texto: 'Debian foi desligada' },
  ];

  return {
    listarDistros: async () => distros.map((d) => ({ ...d })),
    iniciarDistro: async (nome) => {
      await esperar(900);
      achar(nome).rodando = true;
      ligar(nome);
      publicar();
      registrar('distro', nome, `${nome} foi ligada`);
    },
    terminarDistro: async (nome) => {
      await esperar(300);
      achar(nome).rodando = false;
      publicar();
      registrar('distro', nome, `${nome} foi desligada`);
    },
    reiniciarDistro: async (nome) => {
      achar(nome).rodando = false;
      publicar();
      await esperar(900);
      achar(nome).rodando = true;
      ligar(nome);
      publicar();
    },
    desligarWsl: async () => {
      await esperar(500);
      for (const d of distros) d.rodando = false;
      publicar();
      registrar('wsl', null, 'O app desligou o WSL');
    },
    enviarSinal: async (distro, pid, sinal, nome) => {
      if (pid <= 1) throw 'PID 1 é protegido';
      processos[distro] = processos[distro].filter((p) => p.pid !== pid);
      const como = sinal === 9 ? 'forçou o encerramento de' : 'pediu o encerramento de';
      registrar('processo', distro, `O app ${como} ${nome} (PID ${pid})`);
    },
    definirIntervalo: async (ms) => {
      intervalo = ms;
      agendar();
    },
    listarAtividade: async () => eventosIniciais,
    estadoRecursos: async () => {
      publicarRecursos();
      return { memoria: estado.memoriaWindows!, docker: estado.docker, saude: null, devolucao: null, devolver_cache_automatico: false, gatilho: estado.gatilho };
    },
    // Encurtado para a tela: no app real, as medidas saem 15 s e 2 min depois.
    devolverCache: async () => {
      const antes = estado.memoriaWindows?.wsl_bytes ?? 11.6 * GIB;
      const base = { automatica: false, quando_ms: Date.now(), antes_bytes: antes, logo_bytes: null, depois_bytes: null, concluida: false };
      estado.devolucao = base;
      setTimeout(() => (estado.devolucao = { ...base, logo_bytes: antes - 2.9 * GIB }), 3000);
      setTimeout(() => {
        estado.devolucao = { ...base, logo_bytes: antes - 2.9 * GIB, depois_bytes: antes - 1.3 * GIB, concluida: true };
        registrar('memoria', null, 'O app devolveu o cache da VM: ela estava 1,3 GB menor 2 min depois');
      }, 8000);
    },
    lerArmazenamento: async () => {
      await esperar(600);
      return armazenamento;
    },
    analisarPastas: async (distro, ligarAntes) => {
      if (ligarAntes) {
        achar(distro).rodando = true;
        ligar(distro);
        publicar();
        registrar('distro', distro, `${distro} foi ligada`);
        armazenamento.distros_ligadas = distros.filter((x) => x.rodando && x.nome !== 'docker-desktop').map((x) => x.nome);
      }
      await esperar(1500);
      return { ...analiseDemo, distro };
    },
    apagarPastas: async (distro, caminhos) => {
      await esperar(500);
      const liberado = analiseDemo.tmp.filter((p) => caminhos.includes(p.caminho)).reduce((s, p) => s + p.bytes, 0);
      analiseDemo.tmp = analiseDemo.tmp.filter((p) => !caminhos.includes(p.caminho));
      registrar('armazenamento', distro, `O app apagou ${(liberado / GIB).toFixed(1).replace('.', ',')} GB em ${distro}`);
      return liberado;
    },
    limparDocker: async (qual) => {
      await esperar(800);
      const tipo = { sobras: 'Containers', parados: 'Containers', imagens_sem_uso: 'Images', cache_de_build: 'Build Cache', volumes: 'Local Volumes' }[qual];
      const cat = armazenamento.docker!.categorias.find((x) => x.tipo === tipo)!;
      const liberado = cat.recuperavel_bytes;
      cat.total_bytes -= liberado;
      cat.recuperavel_bytes = 0;
      registrar('armazenamento', 'docker-desktop', `O app limpou o Docker: ${(liberado / GIB).toFixed(1).replace('.', ',')} GB liberados`);
      return liberado;
    },
    compactarDisco: async (caminho) => {
      await esperar(2500);
      const disco = armazenamento.discos.find((d) => d.caminho === caminho)!;
      const antes = disco.tamanho_bytes;
      const depois = disco.usado_bytes !== null ? disco.usado_bytes + 0.4 * GIB : antes;
      disco.tamanho_bytes = depois;
      for (const d of distros) d.rodando = false;
      publicar();
      registrar('wsl', null, `O app desligou o WSL para compactar o disco de ${disco.nome}`);
      registrar('armazenamento', disco.nome, `O app compactou o disco de ${disco.nome}: de ${(antes / GIB).toFixed(1).replace('.', ',')} GB para ${(depois / GIB).toFixed(1).replace('.', ',')} GB no Windows`);
      return { antes_bytes: antes, depois_bytes: depois };
    },
    limparParadosDoProjeto: async (projeto) => {
      await esperar(700);
      const p = armazenamento.docker!.projetos.find((x) => x.nome === projeto)!;
      const liberado = p.containers_bytes;
      p.containers -= p.parados;
      p.parados = 0;
      p.containers_bytes = 0;
      registrar('armazenamento', 'docker-desktop', `O app apagou os containers parados do projeto ${projeto}: ${(liberado / GIB).toFixed(1).replace('.', ',')} GB liberados`);
      return liberado;
    },
    definirLimpeza: async (limpeza) => {
      armazenamento.limpeza = { ...limpeza };
    },
    limitesDaMaquina: async () => ({
      ram_bytes: 31.7 * GIB,
      processadores: 8,
      discos: [
        { letra: 'C', livre_bytes: 15.4 * GIB },
        { letra: 'E', livre_bytes: 359.6 * GIB },
      ],
    }),
    definirDevolucao: async (automatico, minimoLivrePct, cacheMaximoGb) => {
      estado.devolverCacheAutomatico = automatico;
      estado.gatilho = { minimo_livre_pct: minimoLivrePct, cache_maximo_bytes: cacheMaximoGb === null ? null : cacheMaximoGb * GIB };
    },
    pararConteineres: (ids, rotulo) => agirEm(ids, false, rotulo),
    iniciarConteineres: (ids, rotulo) => agirEm(ids, true, rotulo),
    reiniciarConteineres: async (ids, rotulo) => {
      await esperar(900);
      for (const c of conteineres.filter((c) => ids.includes(c.id))) {
        if (c.saude === 'unhealthy') c.saude = 'healthy';
        c.memoria_bytes = Math.min(c.memoria_bytes, 120 * 1024 ** 2);
      }
      publicarRecursos();
      registrar('docker', 'docker-desktop', `O app reiniciou ${rotulo} (${ids.length} container(s))`);
    },
    resumoMemoria: async () => ({
      limite_bytes: limiteGb * GIB,
      limite_configurado: true,
      ram_host_bytes: 32 * GIB,
      minutos_de_dados: 3 * 24 * 60,
      pico_bytes: 7.4 * GIB,
      sugestao_bytes: 10 * GIB,
    }),
    definirLimiteMemoria: async (gb, reiniciar) => {
      await esperar(400);
      registrar('config', null, `Limite de memória do WSL: ${limiteGb}GB para ${gb}GB`);
      limiteGb = gb;
      if (reiniciar) {
        for (const d of distros) d.rodando = false;
        publicar();
        registrar('wsl', null, 'O app desligou o WSL para aplicar a configuração');
      }
    },
    // `?atualizacao` na URL simula uma versão nova, para ver o aviso fora do app.
    procurarAtualizacao: async () =>
      new URLSearchParams(location.search).has('atualizacao')
        ? { versao: '9.9.9', notas: null, instalavel: true }
        : null,
    instalarAtualizacao: async () => {
      await esperar(1500);
      throw new Error('Modo demonstração: nada foi instalado.');
    },
    abrirPaginaDaVersao: async () => {},
    lerLog: async () => [
      '[2026-10-29][09:12:00][INFO][toca_do_pinguim_lib] Toca do Pinguim 0.2.0 iniciado',
      '[2026-10-29][09:12:01][INFO][toca_do_pinguim_lib] wsl --version: Versão do WSL: 2.6.1.0 Versão do kernel: 6.6.87.2-1',
      '[2026-10-29][09:12:02][INFO][toca_do_pinguim_lib::agentes] [Ubuntu] agente iniciado (wsl.exe PID 4120, 1000 ms)',
      '[2026-10-29][11:36:39][ERROR][toca_do_pinguim_lib::wsl] wsl.exe -d Ubuntu -u root --exec test -x /tmp/toca-do-pinguim/agente-4c1d: falhou em 30112 ms com Wsl/Service/0x8007274c: Uma tentativa de conexão falhou porque o componente conectado não respondeu.',
      '[2026-10-29][11:36:39][WARN][toca_do_pinguim_lib::wsl] WSL deixou de responder (Ubuntu): Wsl/Service/0x8007274c',
      '[2026-10-29][11:45:42][INFO][toca_do_pinguim_lib::wsl] WSL voltou a responder (Ubuntu) depois de 543 s e 12 falha(s); a última: Wsl/Service/0x8007274c',
    ],
    abrirPastaDeLogs: async () => {},
    lerWslconfig: async () => ({
      caminho: String.raw`C:\Users\voce\.wslconfig`,
      existe: true,
      valores: wslconfig.map((v) => [...v] as [string, string, string]),
    }),
    gravarWslconfig: async (mudancas, reiniciar) => {
      await esperar(400);
      for (const m of mudancas) {
        const i = wslconfig.findIndex(([s, k]) => s === m.secao && k.toLowerCase() === m.chave.toLowerCase());
        if (m.valor === null) {
          if (i >= 0) wslconfig.splice(i, 1);
        } else if (i >= 0) wslconfig[i][2] = m.valor;
        else wslconfig.push([m.secao, m.chave, m.valor]);
      }
      registrar('config', null, `.wslconfig: ${mudancas.map((m) => `${m.chave} para ${m.valor ?? 'padrão'}`).join('; ')}`);
      if (reiniciar) {
        for (const d of distros) d.rodando = false;
        publicar();
      }
    },
    versaoApp: async () => 'demonstração',
    lerAtalhoTeclado: async () => ({ tecla: teclaAtalho, caminho: 'C:\Users\voce\...\Toca do Pinguim.lnk', existe: teclaAtalho !== '' }),
    lerSegundoPlano: async () => ({ ...segundoPlano }),
    definirSegundoPlano: async (fecharParaBandeja, iniciarComWindows) => {
      segundoPlano = { fechar_para_bandeja: fecharParaBandeja, iniciar_com_windows: iniciarComWindows };
    },
    definirAtalhoTeclado: async (tecla) => {
      await esperar(300);
      teclaAtalho = tecla;
    },
    lerWslconf: async (nome) => {
      const d = achar(nome);
      await esperar(300);
      d.rodando = true;
      publicar();
      const valores = wslconf[nome] ?? [];
      return { caminho: `${nome}:/etc/wsl.conf`, existe: valores.length > 0, valores: valores.map((v) => [...v] as [string, string, string]) };
    },
    gravarWslconf: async (nome, mudancas, reiniciar) => {
      const d = achar(nome);
      await esperar(400);
      const valores = (wslconf[nome] ??= []);
      for (const m of mudancas) {
        const i = valores.findIndex(([s, k]) => s === m.secao && k.toLowerCase() === m.chave.toLowerCase());
        if (m.valor === null) {
          if (i >= 0) valores.splice(i, 1);
        } else if (i >= 0) valores[i][2] = m.valor;
        else valores.push([m.secao, m.chave, m.valor]);
      }
      registrar('config', nome, `wsl.conf de ${nome}: ${mudancas.map((m) => `${m.chave} para ${m.valor ?? 'padrão'}`).join('; ')}`);
      if (reiniciar) {
        d.rodando = false;
        publicar();
      }
    },
  };
}
