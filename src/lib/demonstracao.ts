// Backend simulado para desenvolver a tela no navegador, sem Tauri e sem WSL.

import type { Api } from './api';
import { estado } from './estado.svelte';
import type { Distro, Processo } from './tipos';

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
      estado: carga > 1 ? 'R' : 'S',
      nome: n,
      cmdline,
      cpu_pct: carga,
      rss: rssMb * 1024 * 1024,
    }));
  }

  function publicar() {
    estado.receberDistros(distros.map((d) => ({ ...d })));
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
        mem_disponivel: 9.1 * 1024 ** 3 - Math.random() * 200 * 1024 ** 2,
        swap_total: 4 * 1024 ** 3,
        swap_livre: 3.9 * 1024 ** 3,
        processos: lista.map((p) => ({ ...p })),
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
  setTimeout(tick, 50);

  const esperar = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const achar = (nome: string) => {
    const d = distros.find((x) => x.nome === nome);
    if (!d) throw `distro ${nome} não existe`;
    return d;
  };

  return {
    listarDistros: async () => distros.map((d) => ({ ...d })),
    iniciarDistro: async (nome) => {
      await esperar(900);
      achar(nome).rodando = true;
      ligar(nome);
      publicar();
    },
    terminarDistro: async (nome) => {
      await esperar(300);
      achar(nome).rodando = false;
      publicar();
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
    },
    enviarSinal: async (distro, pid) => {
      if (pid <= 1) throw 'PID 1 é protegido';
      processos[distro] = processos[distro].filter((p) => p.pid !== pid);
    },
    definirIntervalo: async (ms) => {
      intervalo = ms;
      agendar();
    },
  };
}
