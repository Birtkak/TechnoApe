//! The build's name for bug reports and the F3 overlay (ADR 0023): the commit, and "+changes" when files it
//! tracks differ from it.
fn main() {
    let git = |args: &[&str]| {
        let o = std::process::Command::new("git").args(args).output().ok().filter(|o| o.status.success())?;
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    println!("cargo:rustc-env=TECHNOAPE_BUILD={commit}{}", if dirty { "+changes" } else { "" });
    // again on a commit, a checkout or a change to the sources
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        for f in ["HEAD", "index"] {
            println!("cargo:rerun-if-changed={dir}/{f}");
        }
    }
    for d in ["../engine/src", "../server/src", "src", "../../data"] {
        println!("cargo:rerun-if-changed={d}");
    }
}
