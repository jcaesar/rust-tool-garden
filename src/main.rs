fn main() {
    use nix::unistd::execvp;
    use std::ffi::CString;
    netns();
    lo_up().expect("failed to bring up loopback");
    execvp::<CString>(&CString::new("nu").unwrap(), &[]).expect("Exec failed");
    // execvp::<CString>(&CString::new("true").unwrap(), &[]).expect("Exec failed");
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
    use nix::{
        sched::{CloneFlags, unshare},
        unistd::{getegid, geteuid},
    };
    use std::fs::write;
    let uid = geteuid();
    let gid = getegid();
    unshare(CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWUSER).expect("Unshare failed");
    write("/proc/self/uid_map", format!("0 {uid} 1")).expect("User map failed");
    // necessary for gid_map
    write("/proc/self/setgroups", "deny").expect("Deny setgroup failed");
    write("/proc/self/gid_map", format!("0 {gid} 1")).expect("Group map failed");
}
