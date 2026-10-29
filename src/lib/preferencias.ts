// Preferências de tela guardadas no navegador do próprio app: ordem das tabelas, grupos
// recolhidos. Perder isso só volta ao padrão, então toda falha de leitura ou escrita é
// ignorada.

const PREFIXO = 'wsl-tm.';

export function lerPreferencia<T>(chave: string, padrao: T): T {
  try {
    const bruto = localStorage.getItem(PREFIXO + chave);
    return bruto === null ? padrao : (JSON.parse(bruto) as T);
  } catch {
    return padrao;
  }
}

export function salvarPreferencia(chave: string, valor: unknown) {
  try {
    localStorage.setItem(PREFIXO + chave, JSON.stringify(valor));
  } catch {
    // Armazenamento indisponível ou cheio: a preferência vale só até fechar.
  }
}
