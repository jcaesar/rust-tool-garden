use nix::errno::Errno;
use nix::libc;
use nix::sched::{CloneFlags, setns};
use nix::unistd::{ForkResult, fork};
use sendfd::{RecvWithFd as _, SendWithFd as _};
use std::os::fd::{AsFd, BorrowedFd, FromRawFd as _, IntoRawFd};
use std::process::exit;

fn main() {
    let [_exe, nspid, listen, connect] = std::env::args()
        .collect::<Vec<_>>()
        .try_into()
        .expect("Need exactly 3 arguments");
    let connect = &*connect.leak();
    let nspid = nix::unistd::Pid::from_raw(nspid.parse().expect("Arument 1 does not parse as pid"));
    let (l, r) = std::os::unix::net::UnixStream::pair().expect("create UnixStream pair");
    let fork = unsafe { fork() }.expect("Fork failed");
    let listen = match fork {
        ForkResult::Child => {
            let pidfd =
                pidfd_open(nspid).expect(&format!("Failed to get handle on target pid {nspid}"));
            // nsenter uses NEWNS and I don't get why
            setns(pidfd, CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWUSER)
                .expect("Switch to target namespace");
            let listen = std::net::TcpListener::bind(&listen)
                .expect(&format!("Failed to listen on {listen}"));
            listen
                .set_nonblocking(true)
                .expect("Failed to listen nonblocking");
            let sent_bytes = b"x";
            let sent_fds = [listen.into_raw_fd()];
            assert_eq!(
                l.send_with_fd(&sent_bytes[..], &sent_fds[..])
                    .expect("send should be successful"),
                sent_bytes.len()
            );
            exit(0);
        }
        ForkResult::Parent { .. } => {
            let mut recv_bytes = [0; 2];
            let mut recv_fds = [0; 2];
            assert_eq!(
                r.recv_with_fd(&mut recv_bytes, &mut recv_fds)
                    .expect("recv should be successful"),
                (1, 1),
                "Receive listen FD from namespace"
            );
            unsafe { std::net::TcpListener::from_raw_fd(recv_fds[0]) }
        }
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("Spawn runtime")
        .block_on(async {
            use tokio::{io, net};
            let listen = net::TcpListener::from_std(listen).unwrap();
            loop {
                let (mut listen, _) = listen.accept().await.expect("Failed to accept connection");
                tokio::spawn(async move {
                    async move {
                        let mut connect = net::TcpStream::connect(connect).await?;
                        io::copy_bidirectional(&mut connect, &mut listen).await?;
                        io::Result::Ok(())
                    }
                    .await
                    .inspect_err(|e| {
                        eprintln!(
                            "{}: could not conenct to {connect}: {e}",
                            env!("CARGO_PKG_NAME")
                        )
                    })
                });
            }
        });
}

fn pidfd_open(nspid: nix::unistd::Pid) -> Result<impl AsFd, Errno> {
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, nspid, 0) };
    let pidfd = Errno::result(pidfd)?;
    Ok(unsafe { BorrowedFd::borrow_raw(pidfd as i32) })
}
