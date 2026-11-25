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
    version_info();
}

/// The exe's name, product and version for Windows (Properties > Details): antivirus and SmartScreen trust an
/// unsigned exe with none less. Built by the Windows SDK's rc.exe; without the SDK the exe is built without it.
fn version_info() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let Some(rc) = std::fs::read_dir(r"C:\Program Files (x86)\Windows Kits\10\bin").ok().and_then(|d| {
        let mut kits: Vec<_> = d.flatten().map(|e| e.path().join(r"x64\rc.exe")).filter(|p| p.is_file()).collect();
        kits.sort();
        kits.pop()
    }) else {
        println!("cargo:warning=no rc.exe from the Windows SDK: the exe has no version information");
        return;
    };
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let v: Vec<u32> = env!("CARGO_PKG_VERSION").split('.').map(|n| n.parse().unwrap_or(0)).collect();
    let (a, b, c) = (v[0], v[1], v[2]);
    let s = |k: &str, val: &str| format!("      VALUE \"{k}\", \"{val}\"\n");
    let rc_text = format!(
        "1 VERSIONINFO\nFILEVERSION {a},{b},{c},0\nPRODUCTVERSION {a},{b},{c},0\nFILEOS 0x40004\nFILETYPE 0x1\n\
         BEGIN\n  BLOCK \"StringFileInfo\"\n  BEGIN\n    BLOCK \"040904B0\"\n    BEGIN\n{}{}{}{}{}{}{}    END\n  END\n\
         BLOCK \"VarFileInfo\"\n  BEGIN\n    VALUE \"Translation\", 0x409, 1200\n  END\nEND\n",
        s("CompanyName", "TechnoApe"),
        s("FileDescription", "TechnoApe"),
        s("ProductName", "TechnoApe"),
        s("InternalName", "technoape"),
        s("OriginalFilename", "TechnoApe.exe"),
        s("FileVersion", &format!("{a}.{b}.{c}")),
        s("ProductVersion", &format!("{a}.{b}.{c}")),
    );
    std::fs::write(out.join("version.rc"), rc_text).unwrap();
    let res = out.join("version.res");
    let ok = std::process::Command::new(rc).arg("/nologo").arg("/fo").arg(&res).arg(out.join("version.rc")).status();
    if ok.is_ok_and(|s| s.success()) {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=rc.exe failed: the exe has no version information");
    }
}
