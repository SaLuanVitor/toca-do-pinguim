import { bytes } from './formato';
import { agrupar } from './projetos';
import type { Conteiner } from './tipos';

export interface Explicacao {
  texto: string;
  /** Só quando o cache de arquivos da VM é o que pesa: aí baixar o teto do WSL resolve. */
  culpaDoCache: boolean;
}

/**
 * Quem ocupa a RAM do Windows quando ela acaba. Mesma regra de `explicar_wsl`, em recursos.rs:
 * o WSL só é apontado quando ocupa mais que o resto do Windows somado.
 */
export function explicarPressao(m: {
  total: number;
  disponivel: number;
  wsl: number | null;
  cache: number | null;
  conteineres: Conteiner[];
}): Explicacao {
  if (m.wsl === null) return { texto: '', culpaDoCache: false };
  const outros = Math.max(0, m.total - m.disponivel - m.wsl);
  if (m.wsl < outros) {
    return {
      texto: `A maior parte está com os programas do Windows (${bytes(outros)}); o WSL ocupa ${bytes(m.wsl)}. Baixar o limite do WSL libera pouco aqui.`,
      culpaDoCache: false,
    };
  }
  const docker = m.conteineres.reduce((s, c) => s + c.memoria_bytes, 0);
  if (m.cache !== null && m.cache >= docker) {
    return {
      texto: `O WSL ocupa ${bytes(m.wsl)}, e a maior parte é cache de arquivos (${bytes(m.cache)}), que o Linux devolve quando precisa mas o Windows conta como ocupado. Baixar o limite de memória do WSL segura esse tamanho.`,
      culpaDoCache: true,
    };
  }
  const maior = agrupar(m.conteineres).find((p) => p.memoria > 0);
  return {
    texto: maior
      ? `O WSL ocupa ${bytes(m.wsl)}; quem mais usa dentro dele é ${maior.nome}, com ${bytes(maior.memoria)}.`
      : `O WSL ocupa ${bytes(m.wsl)}.`,
    culpaDoCache: false,
  };
}
