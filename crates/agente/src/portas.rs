//! Sockets em escuta e o processo dono de cada um.
//!
//! `/proc/net/*` dá o endereço e o inode do socket; o dono só aparece varrendo os
//! descritores de cada processo (`/proc/<pid>/fd/* -> socket:[inode]`). A varredura é a
//! parte cara, por isso o agente a roda a cada poucas amostras e não em todas.

use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};

use protocolo::Porta;

/// `st` de `/proc/net/tcp*`: 0A é LISTEN. Em UDP não há escuta; 07 é o socket preso a um
/// endereço sem par, que é o equivalente.
const TCP_LISTEN: &str = "0A";
const UDP_LIVRE: &str = "07";

pub fn coletar() -> Vec<Porta> {
    let mut sockets: Vec<(Porta, u64)> = Vec::new();
    ler_tabela("/proc/net/tcp", "tcp", TCP_LISTEN, &mut sockets);
    ler_tabela("/proc/net/tcp6", "tcp", TCP_LISTEN, &mut sockets);
    ler_tabela("/proc/net/udp", "udp", UDP_LIVRE, &mut sockets);
    ler_tabela("/proc/net/udp6", "udp", UDP_LIVRE, &mut sockets);
    if sockets.is_empty() {
        return Vec::new();
    }

    let mut por_inode: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, (_, inode)) in sockets.iter().enumerate() {
        por_inode.entry(*inode).or_default().push(i);
    }
    let mut faltam = por_inode.len();
    if let Ok(dir) = fs::read_dir("/proc") {
        'pids: for pid in dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse::<i32>().ok()) {
            let Ok(fds) = fs::read_dir(format!("/proc/{pid}/fd")) else { continue };
            for fd in fds.flatten() {
                let Ok(alvo) = fs::read_link(fd.path()) else { continue };
                let Some(inode) = alvo
                    .to_str()
                    .and_then(|s| s.strip_prefix("socket:["))
                    .and_then(|s| s.strip_suffix(']'))
                    .and_then(|s| s.parse::<u64>().ok())
                else {
                    continue;
                };
                if let Some(indices) = por_inode.remove(&inode) {
                    for i in indices {
                        sockets[i].0.pid = Some(pid);
                    }
                    faltam -= 1;
                    if faltam == 0 {
                        break 'pids;
                    }
                }
            }
        }
    }
    sockets.into_iter().map(|(p, _)| p).collect()
}

fn ler_tabela(arquivo: &str, protocolo: &str, estado: &str, saida: &mut Vec<(Porta, u64)>) {
    let Ok(texto) = fs::read_to_string(arquivo) else { return };
    for linha in texto.lines().skip(1) {
        let c: Vec<&str> = linha.split_whitespace().collect();
        if c.get(3) != Some(&estado) {
            continue;
        }
        let (Some(local), Some(inode)) = (c.get(1), c.get(9).and_then(|i| i.parse::<u64>().ok())) else {
            continue;
        };
        if let Some((endereco, porta)) = endereco_local(local) {
            saida.push((
                Porta { protocolo: protocolo.to_string(), endereco, porta, pid: None },
                inode,
            ));
        }
    }
}

/// `0100007F:1F90` vira (`127.0.0.1`, 8080). Cada palavra de 32 bits do endereço é impressa
/// pelo kernel na ordem de bytes da máquina, por isso `to_le_bytes`.
fn endereco_local(campo: &str) -> Option<(String, u16)> {
    let (ip, porta) = campo.split_once(':')?;
    let porta = u16::from_str_radix(porta, 16).ok()?;
    let palavras: Option<Vec<u32>> = (0..ip.len() / 8)
        .map(|i| u32::from_str_radix(&ip[i * 8..i * 8 + 8], 16).ok())
        .collect();
    let bytes: Vec<u8> = palavras?.into_iter().flat_map(u32::to_le_bytes).collect();
    let endereco = match bytes.len() {
        4 => Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]).to_string(),
        16 => Ipv6Addr::from(<[u8; 16]>::try_from(bytes).ok()?).to_string(),
        _ => return None,
    };
    Some((endereco, porta))
}

#[cfg(test)]
mod testes {
    use super::endereco_local;

    #[test]
    fn ipv4_vem_com_os_bytes_invertidos() {
        assert_eq!(endereco_local("0100007F:1F90"), Some(("127.0.0.1".into(), 8080)));
        assert_eq!(endereco_local("00000000:0050"), Some(("0.0.0.0".into(), 80)));
        // 10.255.255.254:53, o DNS interno do WSL visto na medição.
        assert_eq!(endereco_local("FEFFFF0A:0035"), Some(("10.255.255.254".into(), 53)));
    }

    #[test]
    fn ipv6_inverte_cada_palavra_de_32_bits() {
        assert_eq!(endereco_local("00000000000000000000000000000000:0BB9"), Some(("::".into(), 3001)));
        assert_eq!(endereco_local("00000000000000000000000001000000:1538"), Some(("::1".into(), 5432)));
        assert_eq!(
            endereco_local("B80D0120000000000000000001000000:0016"),
            Some(("2001:db8::1".into(), 22))
        );
    }

    #[test]
    fn campo_estranho_nao_vira_porta() {
        assert_eq!(endereco_local("lixo"), None);
        assert_eq!(endereco_local("0100007F:ZZZZ"), None);
        assert_eq!(endereco_local("0100:0050"), None);
    }
}
