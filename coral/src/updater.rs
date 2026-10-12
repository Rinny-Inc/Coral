use coral_config::updater::{RestartCommand, UpdateChannel, UpdateMode, UpdaterConfig};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fmt, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::broadcast::Sender;

const REPO: &str = "Rinny-Inc/Coral";
const USER_AGENT: &str = concat!("Coral-Updater/", env!("CARGO_PKG_VERSION"));
const MAX_ASSET_BYTES: u64 = 265 * 1024 * 1024;

pub const BUILD_TAG: Option<&str> = option_env!("CORAL_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestartPlan {
    Binary,
    Command { program: PathBuf, args: Vec<String> },
}

pub fn plan_restart(command: &RestartCommand) -> RestartPlan {
    let Some((program, args)) = command.argv() else {
        return RestartPlan::Binary;
    };

    let path = PathBuf::from(&program);
    let resolved = if path.components().count() > 1 || path.is_absolute() {
        std::fs::canonicalize(&path)
    } else {
        Ok(path.clone())
    };

    let program = match resolved {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "
                [Updater] restart command `{}` cannot be resolved ({e}); update restarts will \
                execute the binary instead!",
                path.display()
            );
            return RestartPlan::Binary;
        }
    };

    #[cfg(unix)]
    let (program, args) = {
        use std::os::unix::fs::PermissionsExt;
        let executable = std::fs::metadata(&program)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if program.is_absolute() && !executable {
            println!(
                "[Updater] restart command `{}` is not executable; it will be run with `sh`!",
                program.display()
            );
            let mut shell_args = vec![program.to_string_lossy().into_owned()];
            shell_args.extend(args);
            (PathBuf::from("/bin/sh"), shell_args)
        } else {
            (program, args)
        }
    };

    println!(
        "[Updater] update restarts will run `{}{}`",
        program.display(),
        args.iter().map(|a| format!(" {a}")).collect::<String>()
    );
    RestartPlan::Command { program, args }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    Alpha,
    Beta,
    Rc,
    Stable,
}
impl Channel {
    fn from_config(c: UpdateChannel) -> Option<Self> {
        match c {
            UpdateChannel::Auto => None,
            UpdateChannel::Alpha => Some(Self::Alpha),
            UpdateChannel::Beta => Some(Self::Beta),
            UpdateChannel::Rc => Some(Self::Rc),
            UpdateChannel::Stable => Some(Self::Stable),
        }
    }
}
impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Channel::Alpha => "alpha",
            Channel::Beta => "beta",
            Channel::Rc => "rc",
            Channel::Stable => "stable",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// `None` for stable
    pub pre: Option<(Channel, u64)>,
}
impl Version {
    pub fn parse(tag: &str) -> Option<Self> {
        let s = tag.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let s = s.strip_prefix('.').unwrap_or(s);

        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };

        let mut nums = core.split('.');
        let major = nums.next()?.parse().ok()?;
        let minor = nums.next()?.parse().ok()?;
        let patch = nums.next()?.parse().ok()?;
        if nums.next().is_some() {
            return None;
        }

        let pre = match pre {
            None => None,
            Some(p) => {
                let p = p.to_ascii_lowercase();
                let (channel, rest) = [
                    ("alpha", Channel::Alpha),
                    ("beta", Channel::Beta),
                    ("rc", Channel::Rc),
                ]
                .into_iter()
                .find_map(|(name, ch)| p.strip_prefix(name).map(|r| (ch, r.to_string())))?;
                let rest = rest.trim_start_matches(['.', '-']);
                let n = if rest.is_empty() {
                    0
                } else {
                    rest.parse().ok()?
                };
                Some((channel, n))
            }
        };

        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn channel(&self) -> Channel {
        self.pre.clone().map_or(Channel::Stable, |(c, _)| c)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.clone(), other.pre.clone()) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a), Some(b)) => a.cmp(&b),
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)?;
        match self.pre.clone() {
            None => Ok(()),
            Some((c, 0)) => write!(f, "-{c}"),
            Some((c, n)) => write!(f, "-{c}{n}"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    pub digest: Option<String>,
}

pub fn asset_name() -> String {
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!(
        "coral-{}-{}.{ext}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn binary_name() -> &'static str {
    if cfg!(windows) { "coral.exe" } else { "coral" }
}

pub fn select<'a>(
    releases: &'a [Release],
    current: &Version,
    min: Channel,
) -> Option<(&'a Release, Version)> {
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| Version::parse(&r.tag_name).map(|v| (r, v)))
        .filter(|(_, v)| v.channel() >= min && v > current)
        .max_by(|a, b| a.1.cmp(&b.1))
}

#[derive(Debug)]
pub enum UpdateError {
    Http(reqwest::Error),
    Io(io::Error),
    NoAsset(String),
    NoDigest(String),
    DigestMismatch { expected: String, actual: String },
    TooLarge(u64),
    NotInArchive(&'static str),
}
impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::Http(e) => write!(f, "http: {e}"),
            UpdateError::Io(e) => write!(f, "io: {e}"),
            UpdateError::NoAsset(n) => write!(f, "release has no asset named {n}"),
            UpdateError::NoDigest(n) => write!(f, "asset {n} has no published sha256 digest"),
            UpdateError::DigestMismatch { expected, actual } => {
                write!(f, "sha256 mismatch: expected {expected}, got {actual}")
            }
            UpdateError::TooLarge(n) => write!(f, "asset is {n} bytes, over the limit"),
            UpdateError::NotInArchive(n) => write!(f, "archive does not contain {n}"),
        }
    }
}

impl From<reqwest::Error> for UpdateError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}
impl From<io::Error> for UpdateError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub async fn run(config: UpdaterConfig, restart_tx: Arc<Sender<PathBuf>>) {
    if !config.enabled {
        return;
    }
    let Some(current) = BUILD_TAG.and_then(Version::parse) else {
        println!("[Updater] Development build, automatic updates disabled!");
        return;
    };
    let channel = Channel::from_config(config.channel).unwrap_or(current.channel());
    println!("[Updater] Running {current}, following the {channel} channel!");

    let client = match reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[Updater] Could not build HTTP client: {e}");
            return;
        }
    };

    match check(&client, &current, channel, config.mode).await {
        Ok(Some(exe)) => {
            if restart_tx.send(exe).is_err() {
                eprintln!(
                    "[Updater] Nothing is listening for the restart request; the update is \
                          installed and will apply on next restart!"
                );
            }
        }
        Ok(None) => {}
        Err(e) => eprintln!("[Updater] Update check failed: {e}"),
    }
}

async fn check(
    client: &reqwest::Client,
    current: &Version,
    channel: Channel,
    mode: UpdateMode,
) -> Result<Option<PathBuf>, UpdateError> {
    let releases: Vec<Release> = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=30"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let Some((release, version)) = select(&releases, current, channel) else {
        println!("[Updater] Up to date!");
        return Ok(None);
    };

    if mode == UpdateMode::Notify {
        println!(
            "[Updater] {version} is available (running {current}): https://github.com/{REPO}/releases/tag/{}",
            release.tag_name
        );
        return Ok(None);
    }

    let name = asset_name();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| UpdateError::NoAsset(name.clone()))?;

    let expected = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or_else(|| UpdateError::NoDigest(name.clone()))?
        .to_ascii_lowercase();

    if asset.size > MAX_ASSET_BYTES {
        return Err(UpdateError::TooLarge(asset.size));
    }

    println!(
        "[Updater] Downloading {version} ({name}, {} KiB)...",
        asset.size / 1024
    );
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.len() as u64 > MAX_ASSET_BYTES {
        return Err(UpdateError::TooLarge(bytes.len() as u64));
    }

    let actual = hex(&Sha256::digest(&bytes));
    if actual != expected {
        return Err(UpdateError::DigestMismatch { expected, actual });
    }

    let exe = tokio::task::spawn_blocking(move || -> Result<PathBuf, UpdateError> {
        let binary = extract_binary(&bytes)?;
        Ok(install(&binary)?)
    })
    .await
    .map_err(|e| UpdateError::Io(io::Error::other(e)))??;

    println!(
        "[Updater] Installed {version} to {}. the previous binary is kept as {}",
        exe.display(),
        backup_path(&exe).display()
    );
    Ok(Some(exe))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xF) as usize] as char);
    }
    s
}

fn extract_binary(archive: &[u8]) -> Result<Vec<u8>, UpdateError> {
    let want = binary_name();

    #[cfg(not(windows))]
    {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
        for entry in tar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            if path.file_name().and_then(|n| n.to_str()) == Some(want)
                && entry.header().entry_type().is_file()
            {
                use std::io::Read;

                let mut out = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut out)?;
                return Ok(out);
            }
        }
    }

    #[cfg(windows)]
    {
        use std::io::Read;
        let mut zip = zip::ZipArchive::new(io::Cursor::new(archive)).map_err(io::Error::other)?;
        if let Ok(mut file) = zip.by_name(want) {
            let mut out = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut out)?;
            return Ok(out);
        }
    }

    Err(UpdateError::NotInArchive(want))
}

fn backup_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    exe.with_file_name(name)
}

fn install(binary: &[u8]) -> io::Result<PathBuf> {
    let exe = std::fs::canonicalize(std::env::current_exe()?)?;
    install_at(&exe, binary)?;
    Ok(exe)
}

fn install_at(exe: &Path, binary: &[u8]) -> io::Result<()> {
    let mut staged_name = std::ffi::OsString::from(".");
    staged_name.push(exe.file_name().unwrap_or_default());
    staged_name.push(".update");
    let staged = exe.with_file_name(staged_name);
    let backup = backup_path(exe);

    std::fs::write(&staged, binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(exe)
            .map(|m| m.permissions().mode())
            .unwrap_or(0o755);
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(mode | 0o111))?;
    }

    std::fs::remove_file(&backup).ok();
    if let Err(e) = std::fs::rename(exe, &backup) {
        std::fs::remove_file(&staged).ok();
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&staged, exe) {
        std::fs::rename(&backup, exe).ok();
        std::fs::remove_file(&staged).ok();
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("unparseable {s}"))
    }

    fn rel(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            assets: vec![],
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("coral-restart-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn script(dir: &Path, name: &str, executable: bool) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\necho launched\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if executable { 0o755 } else { 0o644 };
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        p
    }

    #[test]
    fn nothing_configured_means_execute_the_binary() {
        assert_eq!(
            plan_restart(&RestartCommand::default()),
            RestartPlan::Binary
        );
        assert_eq!(
            plan_restart(&RestartCommand::Line(String::new())),
            RestartPlan::Binary
        );
        assert_eq!(
            plan_restart(&RestartCommand::Line("   ".into())),
            RestartPlan::Binary
        );
        assert_eq!(
            plan_restart(&RestartCommand::Argv(vec![])),
            RestartPlan::Binary
        );
    }

    #[test]
    fn a_line_is_split_into_program_and_arguments() {
        let c = RestartCommand::Line("./start.sh --nogui -Xmx4G".into());
        let (program, args) = c.argv().unwrap();
        assert_eq!(program, "./start.sh");
        assert_eq!(args, ["--nogui", "-Xmx4G"]);
    }

    #[test]
    fn the_array_form_keeps_arguments_whole() {
        let c = RestartCommand::Argv(vec![
            "/opt/my server/start.sh".into(),
            "--world=my world".into(),
        ]);
        let (program, args) = c.argv().unwrap();
        assert_eq!(program, "/opt/my server/start.sh");
        assert_eq!(args, ["--world=my world"]);
    }

    #[cfg(unix)]
    #[test]
    fn an_executable_script_is_planned_as_itself() {
        let dir = tmpdir("exec");
        let sh = script(&dir, "start.sh", true);

        let plan = plan_restart(&RestartCommand::Argv(vec![
            sh.to_string_lossy().into_owned(),
            "--nogui".into(),
        ]));
        match plan {
            RestartPlan::Command { program, args } => {
                assert_eq!(program, std::fs::canonicalize(&sh).unwrap());
                assert_eq!(args, ["--nogui"]);
            }
            other => panic!("expected a command, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_non_executable_script_is_planned_through_sh() {
        let dir = tmpdir("noexec");
        let sh = script(&dir, "start.sh", false);

        match plan_restart(&RestartCommand::Line(sh.to_string_lossy().into_owned())) {
            RestartPlan::Command { program, args } => {
                assert_eq!(program, Path::new("/bin/sh"));
                assert_eq!(args, [sh.canonicalize().unwrap().to_string_lossy()]);
            }
            other => panic!("expected a command, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_launcher_falls_back_to_the_binary() {
        let dir = tmpdir("missing");
        let missing = dir.join("not-here.sh");
        assert_eq!(
            plan_restart(&RestartCommand::Line(
                missing.to_string_lossy().into_owned()
            )),
            RestartPlan::Binary
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_existing_tags() {
        assert_eq!(v("v0.1.2-alpha").pre, Some((Channel::Alpha, 0)));
        assert_eq!(v("v.0.1.0-alpha"), v("v0.1.0-alpha"));
        assert_eq!(v("v0.2.0-rc1").pre, Some((Channel::Rc, 1)));
        assert_eq!(v("v0.2.0-beta.3").pre, Some((Channel::Beta, 3)));
        assert_eq!(v("v1.0.0").channel(), Channel::Stable);
        assert!(Version::parse("nightly").is_none());
        assert!(Version::parse("v1.0").is_none());
        assert!(Version::parse("v1.0.0-gamma").is_none());
    }

    #[test]
    fn ordering() {
        assert!(v("v0.1.2-alpha") > v("v0.1.1-alpha"));
        assert!(v("v0.2.0-alpha") < v("v0.2.0-beta"));
        assert!(v("v0.2.0-beta") < v("v0.2.0-rc1"));
        assert!(v("v0.2.0-rc1") < v("v0.2.0-rc2"));
        assert!(v("v0.2.0-rc2") < v("v0.2.0"));
        assert!(v("v0.2.0") < v("v0.2.1-alpha"));
        assert_eq!(v("v0.2.0-rc").to_string(), "v0.2.0-rc");
    }

    #[test]
    fn channel_selection() {
        let rs = [
            rel("v.0.1.0-alpha"),
            rel("v0.1.1-alpha"),
            rel("v0.1.2-alpha"),
            rel("v0.2.0-beta"),
            rel("v0.2.0"),
            rel("v0.3.0-alpha"),
            rel("garbage"),
        ];
        let pick = |cur: &str, ch| select(&rs, &v(cur), ch).map(|(_, v)| v.to_string());

        assert_eq!(
            pick("v0.1.1-alpha", Channel::Alpha).as_deref(),
            Some("v0.3.0-alpha")
        );
        assert_eq!(
            pick("v0.1.1-alpha", Channel::Beta).as_deref(),
            Some("v0.2.0")
        );
        assert_eq!(
            pick("v0.1.1-alpha", Channel::Stable).as_deref(),
            Some("v0.2.0")
        );
        assert_eq!(pick("v0.2.0", Channel::Stable), None);
        assert_eq!(pick("v0.3.0-alpha", Channel::Alpha), None);
    }

    #[test]
    fn drafts_are_ignored() {
        let mut r = rel("v9.9.9");
        r.draft = true;
        assert!(select(&[r], &v("v0.1.0"), Channel::Alpha).is_none());
    }

    #[test]
    fn asset_name_matches_release_matrix() {
        let expected = [
            "coral-linux-x86_64.tar.gz",
            "coral-linux-aarch64.tar.gz",
            "coral-macos-x86_64.tar.gz",
            "coral-macos-aarch64.tar.gz",
            "coral-windows-x86_64.zip",
        ];
        assert!(
            expected.contains(&asset_name().as_str()),
            "{}",
            asset_name()
        );
    }

    #[test]
    fn sha256_hex() {
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn extract_and_install_roundtrip() {
        use std::io::Write;

        let payload = b"#!/bin/sh\necho new\n".to_vec();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        {
            let mut tar = tar::Builder::new(&mut gz);
            let mut h = tar::Header::new_gnu();
            h.set_size(payload.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            tar.append_data(&mut h, "coral", &payload[..]).unwrap();
            tar.finish().unwrap();
        }
        let archive = gz.finish().unwrap();
        assert_eq!(extract_binary(&archive).unwrap(), payload);

        let dir = std::env::temp_dir().join(format!("coral-upd-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("coral");
        std::fs::File::create(&exe)
            .unwrap()
            .write_all(b"old")
            .unwrap();

        install_at(&exe, &payload).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), payload);
        assert_eq!(std::fs::read(backup_path(&exe)).unwrap(), b"old");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                std::fs::metadata(&exe).unwrap().permissions().mode() & 0o111,
                0
            );
        }
        assert!(!dir.join(".coral.update").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_new_binary_is_at_the_original_path_and_the_old_one_moved() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!("coral-upd-paths-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("coral");
        std::fs::File::create(&exe)
            .unwrap()
            .write_all(b"v1")
            .unwrap();

        install_at(&exe, b"v2").unwrap();

        assert_eq!(std::fs::read(&exe).unwrap(), b"v2", "restart target");
        assert_eq!(
            std::fs::read(backup_path(&exe)).unwrap(),
            b"v1",
            "rollback copy"
        );
        assert_ne!(backup_path(&exe), exe);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_failed_swap_puts_the_original_back() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!("coral-upd-fail-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("coral");
        std::fs::File::create(&exe)
            .unwrap()
            .write_all(b"v1")
            .unwrap();

        // a directory where the staged file should go makes the rename fail
        std::fs::create_dir(dir.join(".coral.update")).unwrap();
        assert!(install_at(&exe, b"v2").is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"v1", "original must survive");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn the_installed_binary_is_runnable() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!("coral-upd-run-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("coral");
        std::fs::File::create(&exe)
            .unwrap()
            .write_all(b"#!/bin/sh\necho v1\n")
            .unwrap();

        install_at(&exe, b"#!/bin/sh\necho v2\n").unwrap();

        let out = std::process::Command::new(&exe)
            .output()
            .expect("installed binary runs");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "v2");
        std::fs::remove_dir_all(&dir).ok();
    }
}
