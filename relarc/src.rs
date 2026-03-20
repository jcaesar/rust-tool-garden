use std::{
    collections::HashSet,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};
use tar::{Builder, EntryType, Header};

fn main() {
    let argv = std::env::args_os().collect::<Vec<_>>();
    let mut ar = Builder::new(BufWriter::new(
        File::create(&argv[1]).expect("Open arg1 for writing"),
    ));
    let mut jobs = argv[2..]
        .iter()
        .map(|p| Path::new(p).canonicalize().expect("Canon"))
        .collect::<Vec<_>>();

    let mut done = HashSet::<PathBuf>::new();
    while let Some(job) = jobs.pop() {
        if done.contains(&job) {
            continue;
        }
        if job.ends_with(".src") {
            continue;
        }
        done.insert(job.clone());
        eprintln!("{job:?}");
        let dent = &job.strip_prefix("/").unwrap_or(&job);
        if job.is_symlink() {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_size(0);
            let parent = job.parent().expect("Must have a parent to be a symlink");
            let lnk = job.read_link().expect("Readlink");
            let rlnko;
            let rlnk = match lnk.is_absolute() {
                true => {
                    rlnko = pathdiff::diff_paths(&lnk, &parent).expect("No reals");
                    &rlnko
                }
                false => &lnk,
            };
            ar.append_link(&mut header, dent, rlnk).unwrap();
            match parent.join(&lnk).canonicalize() {
                Ok(t) => jobs.push(t),
                Err(e) => eprintln!("{job:?} -> {lnk:?}: {e}"),
            }
        } else if job.is_dir() {
            for entry in job.read_dir().expect("Read dir") {
                jobs.push(entry.expect("Read dir entry").path());
            }
        } else if job.is_file() {
            ar.append_file(dent, &mut File::open(&job).expect("Read file"))
                .expect("Append")
        } else {
            eprintln!("What is {job:?}");
        }
    }
}
