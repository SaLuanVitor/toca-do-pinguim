//! Log do kernel (`/dev/kmsg`) a partir do momento em que o agente sobe.
//!
//! Interessa uma coisa só por enquanto: o kernel encerrando processo por falta de memória.
//! Sem isso, um processo some da lista e ninguém sabe por quê.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use protocolo::EventoKernel;

pub struct Kmsg {
    arquivo: File,
}

impl Kmsg {
    pub fn abrir() -> Option<Kmsg> {
        let arquivo = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/kmsg")
            .ok()?;
        // SEEK_END pula o que já estava no buffer: o que aconteceu antes do agente
        // não é novidade e se repetiria a cada reconexão.
        // SAFETY: lseek num descritor válido e aberto acima.
        unsafe { libc::lseek(arquivo.as_raw_fd(), 0, libc::SEEK_END) };
        Some(Kmsg { arquivo })
    }

    /// Cada `read` em `/dev/kmsg` devolve exatamente um registro, até não haver mais.
    pub fn novos(&mut self) -> Vec<EventoKernel> {
        let mut eventos = Vec::new();
        let mut buf = vec![0u8; 8192];
        loop {
            match self.arquivo.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(e) = interpretar(&String::from_utf8_lossy(&buf[..n])) {
                        eventos.push(e);
                    }
                }
                // EPIPE: registros sobrescritos antes da leitura; o próximo `read` segue.
                Err(e) if e.raw_os_error() == Some(libc::EPIPE) => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        eventos
    }
}

/// Registro: `prioridade,seq,microssegundos,flags;mensagem` seguido de linhas de contexto.
fn interpretar(registro: &str) -> Option<EventoKernel> {
    let (cabecalho, resto) = registro.split_once(';')?;
    let mensagem = resto.lines().next()?.trim();
    // "Out of memory: Killed process 123 (node)" e a variante de cgroup terminam iguais.
    if !mensagem.contains("Killed process") {
        return None;
    }
    let seq = cabecalho.split(',').nth(1)?.parse().ok()?;
    Some(EventoKernel { seq, texto: mensagem.to_string() })
}

#[cfg(test)]
mod testes {
    use super::interpretar;

    #[test]
    fn morte_por_falta_de_memoria_vira_evento() {
        let r = "3,1234,987654321,-;Out of memory: Killed process 4821 (node) total-vm:4120332kB, anon-rss:3912000kB\n SUBSYSTEM=memory\n";
        let e = interpretar(r).expect("evento");
        assert_eq!(e.seq, 1234);
        assert_eq!(e.texto, "Out of memory: Killed process 4821 (node) total-vm:4120332kB, anon-rss:3912000kB");
    }

    #[test]
    fn variante_de_cgroup_tambem_conta() {
        let r = "3,77,1,-;Memory cgroup out of memory: Killed process 99 (php-fpm) total-vm:1kB";
        assert_eq!(interpretar(r).map(|e| e.seq), Some(77));
    }

    #[test]
    fn o_resto_do_log_fica_de_fora() {
        assert!(interpretar("6,667,53301612,-;eth0: renamed from veth0f32a4c").is_none());
        assert!(interpretar("4,12,5,-;node invoked oom-killer: gfp_mask=0x140cca").is_none());
        assert!(interpretar("sem cabeçalho").is_none());
    }
}
