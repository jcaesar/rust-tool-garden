use std::{
    ffi::OsString,
    io::{PipeWriter, Read, Write as _, pipe},
    os::unix::net::UnixListener as StdUnixListener,
    path::{Path, PathBuf},
};

use tokio::time::sleep;

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

    #[clap(long, short)]
    daemonize: bool,

    #[clap(short = 'H', long)]
    health: Option<String>,
}

fn main() {
    let Args {
        map_root_user,
        exec,
        listen,
        connect,
        daemonize,
        health,
    } = clap::Parser::parse();
    let daemonize = daemonize.then(|| unsafe { fork() });
    let connect = &*format!("127.0.0.1:{connect}").leak();
    use nix::unistd::{getegid, geteuid};
    let (uid, gid) = (geteuid(), getegid());
    let listen = listen_on(&listen);
    netns();
    if map_root_user {
        map_root(uid, gid);
    }
    lo_up().expect("failed to bring up loopback");
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("Spawn runtime")
        .block_on(async {
            spawn(exec);
            if let Some(health) = health {
                check_health(connect, health).await;
            };
            if let Some(mut daemonize) = daemonize {
                tokio::task::spawn_blocking(|| {
                    daemonize.write_all(b"0").ok();
                    drop(daemonize);
                })
                .await
                .ok();
            }
            transfer(listen, connect).await
        });
    unreachable!();
}

async fn check_health(connect: &str, health: String) {
    loop {
        let res = reqwest::get(format!("http://{connect}{health}")).await;
        if let Ok(res) = res {
            if res.status().is_success() {
                break;
            }
        }
        sleep(std::time::Duration::from_millis(333)).await;
    }
}

/// Safety: program may not be multithreaded
// (we could check this…)
// would probably be better to just re-spawn self
unsafe fn fork() -> PipeWriter {
    use nix::{
        sys::wait::{WaitStatus, waitpid},
        unistd::{ForkResult, fork},
    };
    use std::process::exit;
    let (mut reader, writer) = pipe().expect("Create pipe");
    let pid = unsafe { fork() }.expect("Fork failed");
    let child = match pid {
        ForkResult::Child => return writer,
        ForkResult::Parent { child } => child,
    };
    drop(writer);
    let mut data = Vec::new();
    let res = reader.read_to_end(&mut data);
    if res.is_ok() && matches!(data.as_slice(), b"0") {
        exit(0)
    };
    let wait = waitpid(child, None).expect("Wait failed");
    match wait {
        WaitStatus::Exited(_pid, c) => exit(c),
        _ => panic!("Weird wait result: {wait:?}"),
    };
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
