use std::ffi::CString;

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(long)]
    map_root_user: bool,

    #[clap(required = true)]
    exec: Vec<CString>,
}

fn main() {
    let Args {
        map_root_user,
        exec,
    } = dbg!(clap::Parser::parse());
    use nix::unistd::{execvp, getegid, geteuid};
    use std::ffi::CString;
    let (uid, gid) = (geteuid(), getegid());
    netns();
    if map_root_user {
        map_root(uid, gid);
    }
    lo_up().expect("failed to bring up loopback");
    let exe = exec.get(0).expect("No exe path");
    execvp::<CString>(exe, &exec).expect("Exec failed");
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
