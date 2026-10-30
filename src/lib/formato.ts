const numero1 = new Intl.NumberFormat('pt-BR', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const numero0 = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 0 });

export function bytes(n: number): string {
  if (n < 1024 * 1024) return `${numero0.format(n / 1024)} KB`;
  if (n < 1024 * 1024 * 1024) return `${numero1.format(n / 1024 / 1024)} MB`;
  return `${numero1.format(n / 1024 / 1024 / 1024)} GB`;
}

export function pct(n: number): string {
  return `${numero1.format(n)}%`;
}

export function duracao(segundos: number): string {
  const d = Math.floor(segundos / 86400);
  const h = Math.floor((segundos % 86400) / 3600);
  const m = Math.floor((segundos % 3600) / 60);
  if (d > 0) return `${d} d ${h} h`;
  if (h > 0) return `${h} h ${m} min`;
  return `${m} min`;
}

const ESTADOS: Record<string, string> = {
  R: 'Executando',
  S: 'Aguardando',
  D: 'Aguardando E/S',
  Z: 'Zumbi',
  T: 'Parado',
  t: 'Em depuração',
  I: 'Ocioso',
  X: 'Encerrado',
};

const SUSPEITAS: Record<string, string> = {
  aguardando_es: 'Preso esperando disco ou rede',
  zumbi: 'Zumbi: o processo pai não o recolheu',
  cpu_constante: 'Um núcleo inteiro sem parar',
};

/** "há 3 min", a partir de um instante em milissegundos Unix. */
export function ha(desdeMs: number): string {
  const min = Math.max(1, Math.floor((Date.now() - desdeMs) / 60_000));
  return min < 60 ? `há ${min} min` : `há ${Math.floor(min / 60)} h ${min % 60} min`;
}

export function rotuloSuspeita(motivo: string): string {
  return SUSPEITAS[motivo] ?? motivo;
}

export function rotuloEstado(c: string): string {
  return ESTADOS[c] ?? c;
}
