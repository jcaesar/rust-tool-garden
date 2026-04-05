use std::{
    collections::HashSet,
    fs::File,
    io::BufWriter,
    path::{Component, PathBuf},
};
use tar::{Builder, EntryType, Header};

fn main() {
    let argv = std::env::args_os().collect::<Vec<_>>();
    let mut ar = Builder::new(BufWriter::new(std::io::stdout().lock()));
    let cwd = std::env::current_dir()
        .expect("pwd")
        .canonicalize()
        .expect("canon pwd");
    let mut jobs = argv[1..]
        .iter()
        .map(|p| canon(cwd.join(p)))
        .collect::<Vec<_>>();

    let mut done = HashSet::<PathBuf>::new();
    while let Some(job) = jobs.pop() {
        if done.contains(&job) {
            continue;
        }
        done.insert(job.clone());
        eprintln!("{job:?}");
        let dent = arthur(&job);
        if job.is_symlink() {
            let parent = job.parent().expect("/ can't be a symlink");
            let lnk = job.read_link().expect("Readlink");
            let rlnko;
            let rlnk = match lnk.is_absolute() {
                true => {
                    rlnko = pathdiff::diff_paths(&lnk, &parent).expect("No reals");
                    &rlnko
                }
                false => &lnk,
            };
            ar.append_link(&mut lnkhdr(), dent, rlnk).unwrap();
            match parent.join(&lnk).canonicalize() {
                Ok(t) => jobs.push(t),
                Err(e) => eprintln!("{job:?} -> {lnk:?}: {e}"),
            }
        } else if job.is_dir() {
            for entry in job.read_dir().expect("Read dir") {
                jobs.push(entry.expect("Read dir entry").path());
            }
            ar.append_dir(dent, &job).unwrap();
        } else if job.is_file() {
            ar.append_file(dent, &mut File::open(&job).expect("Read file"))
                .expect("Append")
        } else {
            eprintln!("What is {job:?}");
        }
    }
    argv[1..].iter().for_each(|p| {
        let pc = canon(PathBuf::from(p));
        let true = pc.is_relative() else {
            return;
        };
        let Some(parent) = pc.parent() else {
            return;
        };
        if pc.is_relative() {
            let dent = arthur(&cwd.join(&p));
            let rlnk = pathdiff::diff_paths(&dent, &parent).expect("No reals");
            ar.append_link(&mut lnkhdr(), &pc, rlnk).unwrap();
        }
    });
}

fn canon(p: PathBuf) -> PathBuf {
    let mut ret = Vec::new();
    for c in p.components() {
        if is_root(&c) {
            ret = [c].to_vec();
        } else if matches!(c, Component::ParentDir) {
            if !ret.last().map_or(false, is_root) {
                ret.pop();
            }
        } else if matches!(c, Component::CurDir) {
            // pass
        } else {
            ret.push(c);
        }
    }
    ret.into_iter().collect()
}

fn is_root(c: &std::path::Component<'_>) -> bool {
    matches!(c, Component::RootDir | Component::Prefix(_))
}

fn lnkhdr() -> Header {
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Symlink);
    header.set_size(0);
    header
}

fn arthur(job: &PathBuf) -> PathBuf {
    let dent = job.strip_prefix("/").unwrap_or(job);
    let dent = PathBuf::from(".relarc").join(dent);
    assert!(
        dent.starts_with(".relarc"),
        "{dent:?} weird, couldn't relativize into .relarc"
    );
    dent
}
