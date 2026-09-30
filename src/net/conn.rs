//! TCP connections (a reader and a writer thread each, so the game never blocks on the
//! network), the host's server socket and the LAN announcements and discovery.

use super::{Msg, PROTOCOL};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// First TCP port tried for a LAN game (the next few are tried if it is taken).
pub const DEFAULT_PORT: u16 = 25565;
/// UDP port the LAN announcements go to.
const DISCOVERY_PORT: u16 = 4446;
const AD_PREFIX: &str = "RUSTCRAFT_LAN";
const MAX_FRAME: usize = 64 << 20;
/// How long a connection may go without sending (or a player hear nothing from the host)
/// before it counts as lost.
const SILENT_LIMIT: Duration = Duration::from_secs(60);

/// One TCP connection with a reader and a writer thread, so the game never blocks on it.
pub struct Conn {
    out: Option<Sender<Vec<u8>>>,
    rx: Receiver<Msg>,
    closed: Arc<AtomicBool>,
}

fn read_frame(s: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut buf = vec![0; len];
    s.read_exact(&mut buf)?;
    Ok(buf)
}

impl Conn {
    pub fn new(stream: TcpStream) -> io::Result<Conn> {
        stream.set_nodelay(true)?;
        // Unable to send for this long, the other end is gone or stuck (the writer thread ends
        // instead of waiting forever, and the game sees the connection closed).
        stream.set_write_timeout(Some(SILENT_LIMIT))?;
        let closed = Arc::new(AtomicBool::new(false));
        let (out_tx, out_rx) = channel::<Vec<u8>>();
        let (in_tx, in_rx) = channel::<Msg>();

        let mut reader = stream.try_clone()?;
        let flag = closed.clone();
        std::thread::Builder::new()
            .name("net-read".into())
            .spawn(move || {
                while let Ok(frame) = read_frame(&mut reader) {
                    match Msg::decode(&frame) {
                        Some(m) => {
                            if in_tx.send(m).is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                flag.store(true, Ordering::Release);
            })?;

        let mut writer = stream;
        let flag = closed.clone();
        std::thread::Builder::new()
            .name("net-write".into())
            .spawn(move || {
                // Batch whatever is queued into one write.
                while let Ok(first) = out_rx.recv() {
                    let mut buf = first;
                    while let Ok(more) = out_rx.try_recv() {
                        buf.extend(more);
                    }
                    if writer.write_all(&buf).is_err() {
                        break;
                    }
                }
                // Everything was sent (or the peer is gone): close both directions, which
                // also ends the reader thread.
                let _ = writer.flush();
                let _ = writer.shutdown(std::net::Shutdown::Both);
                flag.store(true, Ordering::Release);
            })?;

        Ok(Conn {
            out: Some(out_tx),
            rx: in_rx,
            closed,
        })
    }

    pub fn connect(addr: &str) -> io::Result<Conn> {
        use std::net::ToSocketAddrs;
        let target = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address"))?;
        let stream = TcpStream::connect_timeout(&target, Duration::from_secs(4))?;
        // The host sends many times a second: nothing for this long, it is gone (a player
        // may be silent a while itself, loading, so only this end waits for the other).
        stream.set_read_timeout(Some(SILENT_LIMIT))?;
        Conn::new(stream)
    }

    pub fn send(&self, m: &Msg) {
        if let Some(out) = &self.out {
            let body = m.encode();
            let mut frame = Vec::with_capacity(body.len() + 4);
            frame.extend((body.len() as u32).to_le_bytes());
            frame.extend(body);
            let _ = out.send(frame);
        }
    }

    /// Messages received so far, and whether the connection is still open.
    pub fn poll(&self) -> (Vec<Msg>, bool) {
        // Whether it closed is read first: everything the reader got before closing is then in
        // the queue (the last message, e.g. why the host refused, is not lost).
        let closed = self.closed.load(Ordering::Acquire);
        let mut v = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(m) => v.push(m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return (v, false),
            }
        }
        let open = !closed || !v.is_empty();
        (v, open)
    }

    /// Sends what is queued, then closes.
    pub fn close(&mut self) {
        self.out = None;
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.close();
    }
}

// ---------------------------------------------------------------------------- server

/// The host's listening socket and LAN announcer.
pub struct Server {
    pub port: u16,
    incoming: Receiver<TcpStream>,
    stop: Arc<AtomicBool>,
}

impl Server {
    /// Starts listening (on the first free port from `DEFAULT_PORT`) and announcing
    /// `world` hosted by `host` on the LAN.
    pub fn start(world: &str, host: &str) -> io::Result<Server> {
        let mut last_err = None;
        let mut listener = None;
        for port in DEFAULT_PORT..DEFAULT_PORT + 10 {
            match TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
                Ok(l) => {
                    listener = Some(l);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        let listener = listener.ok_or_else(|| last_err.unwrap())?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));

        let (tx, rx) = channel();
        let flag = stop.clone();
        std::thread::Builder::new()
            .name("net-accept".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((s, _)) => {
                            let _ = s.set_nonblocking(false);
                            if tx.send(s).is_err() {
                                break;
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(200)),
                    }
                }
            })?;

        // "RUSTCRAFT_LAN;<protocol>;<port>;<world>;<host>" (';' is removed from the names).
        let clean = |s: &str| s.replace(';', " ");
        let ad = format!(
            "{AD_PREFIX};{PROTOCOL};{port};{};{}",
            clean(world),
            clean(host)
        );
        let flag = stop.clone();
        std::thread::Builder::new()
            .name("net-announce".into())
            .spawn(move || {
                let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
                    return;
                };
                let _ = sock.set_broadcast(true);
                while !flag.load(Ordering::Relaxed) {
                    let _ = sock.send_to(ad.as_bytes(), (Ipv4Addr::BROADCAST, DISCOVERY_PORT));
                    // Also to this computer (a second game window here).
                    let _ = sock.send_to(ad.as_bytes(), (Ipv4Addr::LOCALHOST, DISCOVERY_PORT));
                    for _ in 0..15 {
                        if flag.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            })?;
        Ok(Server {
            port,
            incoming: rx,
            stop,
        })
    }

    /// New connections since the last call.
    pub fn accept(&self) -> Vec<TcpStream> {
        self.incoming.try_iter().collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// This computer's address on the local network (for showing to others), if any.
pub fn local_ip() -> Option<std::net::IpAddr> {
    // Connecting a UDP socket sends nothing; it only picks the outgoing interface.
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    s.connect((Ipv4Addr::new(192, 168, 0, 1), 9)).ok()?;
    s.local_addr().ok().map(|a| a.ip())
}

// ---------------------------------------------------------------------------- discovery

/// A LAN game heard on the network.
#[derive(Clone, Debug)]
pub struct LanGame {
    pub addr: SocketAddr,
    pub world: String,
    pub host: String,
    pub compatible: bool,
    pub seen: Instant,
}

/// Listens for LAN announcements while the multiplayer screen is open.
pub struct Finder {
    rx: Receiver<LanGame>,
    stop: Arc<AtomicBool>,
    pub games: Vec<LanGame>,
    /// Listening failed (the port is taken, e.g. by another game window on this computer).
    pub error: bool,
}

impl Finder {
    pub fn start() -> Finder {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT));
        let error = sock.is_err();
        if let Ok(sock) = sock {
            let _ = sock.set_read_timeout(Some(Duration::from_millis(200)));
            let _ = std::thread::Builder::new()
                .name("net-find".into())
                .spawn(move || {
                    let mut buf = [0u8; 512];
                    while !flag.load(Ordering::Relaxed) {
                        let Ok((n, from)) = sock.recv_from(&mut buf) else {
                            continue;
                        };
                        let Ok(text) = std::str::from_utf8(&buf[..n]) else {
                            continue;
                        };
                        if let Some(g) = parse_ad(text, from) {
                            if tx.send(g).is_err() {
                                break;
                            }
                        }
                    }
                });
        }
        Finder {
            rx,
            stop,
            games: Vec::new(),
            error,
        }
    }

    /// Takes in new announcements and forgets games not heard from for 5 seconds.
    pub fn update(&mut self) {
        for g in self.rx.try_iter() {
            match self.games.iter_mut().find(|x| x.addr == g.addr) {
                Some(x) => *x = g,
                None => self.games.push(g),
            }
        }
        self.games
            .retain(|g| g.seen.elapsed() < Duration::from_secs(5));
    }
}

impl Drop for Finder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn parse_ad(text: &str, from: SocketAddr) -> Option<LanGame> {
    let mut parts = text.splitn(5, ';');
    if parts.next()? != AD_PREFIX {
        return None;
    }
    let proto: u16 = parts.next()?.parse().ok()?;
    let port: u16 = parts.next()?.parse().ok()?;
    let world = parts.next()?.to_string();
    let host = parts.next()?.to_string();
    Some(LanGame {
        addr: SocketAddr::new(from.ip(), port),
        world,
        host,
        compatible: proto == PROTOCOL,
        seen: Instant::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_over_loopback() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let client = Conn::connect(&addr.to_string()).unwrap();
        let (server_side, _) = listener.accept().unwrap();
        let server = Conn::new(server_side).unwrap();
        client.send(&Msg::Hello {
            proto: PROTOCOL,
            name: "a".into(),
        });
        client.send(&Msg::Ready);
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut got = Vec::new();
        while got.len() < 2 && Instant::now() < deadline {
            got.extend(server.poll().0);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[1], Msg::Ready);
        // Closing one side is seen by the other.
        drop(client);
        let deadline = Instant::now() + Duration::from_secs(3);
        while server.poll().1 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!server.poll().1);
    }

    #[test]
    fn announcement_parses() {
        let from: SocketAddr = "192.168.1.20:5000".parse().unwrap();
        let ad = format!("RUSTCRAFT_LAN;{PROTOCOL};25565;Új világ;Albi");
        let g = parse_ad(&ad, from).unwrap();
        assert_eq!(g.addr, "192.168.1.20:25565".parse().unwrap());
        assert_eq!(g.world, "Új világ");
        assert!(g.compatible);
        assert!(parse_ad("MOTD whatever", from).is_none());
    }
}
