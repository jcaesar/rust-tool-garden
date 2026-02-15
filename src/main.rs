use std::{
    ffi::OsString,
    os::unix::net::UnixListener as StdUnixListener,
    path::{Path, PathBuf},
};

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(long, short = 'r')]
    map_root_user: bool,

    #[clap(long, short)]
    listen: PathBuf,
    #[clap(long, short)]
    connect: u16,

    #[clap(required = true)]
    exec: Vec<OsString>,
}

fn main() {
    let Args {
        map_root_user,
        exec,
        listen,
        connect,
    } = clap::Parser::parse();
    let connect = &*format!("127.0.0.1:{connect}").leak();
    use nix::unistd::{getegid, geteuid};
    let (uid, gid) = (geteuid(), getegid());
    let listen = listen_on(&listen);
    netns();
    if map_root_user {
        map_root(uid, gid);
    }
    lo_up().expect("failed to bring up loopback");
    tokio::runtime::Runtime::new()
        .expect("Spawn runtime")
        .block_on(async {
            spawn(exec);
            transfer(listen, connect).await
        });
    unreachable!();
}

fn listen_on(listen: &Path) -> StdUnixListener {
    std::fs::remove_file(&listen).ok();
    let listen = StdUnixListener::bind(listen).expect("Failed to create unix listen socket");
    listen
        .set_nonblocking(true)
        .expect("Couldn't set non blocking");
    listen
}

async fn transfer(listen: StdUnixListener, connect: &'static str) -> ! {
    use tokio::{io, net};
    let listen = net::UnixListener::from_std(listen).expect("Convert listener");
    loop {
        match listen.accept().await {
            Ok((mut stream, _addr)) => {
                tokio::spawn(async move {
                    async move {
                        let mut connect = net::TcpStream::connect(connect).await?;
                        io::copy_bidirectional(&mut connect, &mut stream).await?;
                        io::Result::Ok(())
                    }
                    .await
                    .inspect_err(|e| {
                        eprintln!(
                            "{}: couldnot conenct to {connect}: {e}",
                            env!("CARGO_PKG_NAME")
                        )
                    })
                });
            }
            Err(e) => {
                eprintln!("{e:?}");
            }
        }
    }
}

fn spawn(exec: Vec<OsString>) {
    use std::process::Stdio;
    let exe = exec.get(0).expect("No exe path - required by clap");
    let mut exec = tokio::process::Command::new(exe)
        .args(&exec[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn command");
    tokio::spawn(async move {
        let res = exec.wait().await.expect("Child await failed");
        std::process::exit(res.code().expect("Exit code missing"));
    });
}

fn lo_up() -> Result<(), Box<dyn std::error::Error>> {
    use neli::{
        consts::{
            nl::NlmF,
            rtnl::{RtAddrFamily, Rtm},
            socket::NlFamily,
        },
        nl::NlPayload,
        router::synchronous::NlRouter,
        rtnl::{Ifinfomsg, IfinfomsgBuilder},
        utils::Groups,
    };
    let (rtnl, _) = NlRouter::connect(NlFamily::Route, None, Groups::empty())?;
    rtnl.enable_strict_checking(true)?;
    let msg = IfinfomsgBuilder::default()
        .ifi_index(
            /*lo of a freshly created namespace better always be*/ 1,
        )
        .ifi_family(RtAddrFamily::Unspecified)
        .up()
        .build()?;
    rtnl.send::<_, _, Rtm, Ifinfomsg>(Rtm::Setlink, NlmF::empty(), NlPayload::Payload(msg))?;
    Ok(())
}

fn netns() {
    use nix::sched::{CloneFlags, unshare};
    unshare(CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWUSER).expect("Unshare failed");
}

fn map_root(uid: nix::unistd::Uid, gid: nix::unistd::Gid) {
    use std::fs::write;
    write("/proc/self/uid_map", format!("0 {uid} 1")).expect("User map failed");
    // necessary for gid_map
    write("/proc/self/setgroups", "deny").expect("Deny setgroup failed");
    write("/proc/self/gid_map", format!("0 {gid} 1")).expect("Group map failed");
}
