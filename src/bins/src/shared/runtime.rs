//! The runtime of an app whose package carries no main executable: an Electron distribution named by
//! `runtime.json` in the app directory. It is downloaded, verified and unpacked into the app directory
//! on install, carried over from the previous app directory on update, and repaired when the app is
//! started through the stub. Nothing here calls a platform API, so the tests run everywhere.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

/// Written by the application's build, shipped in the package.
pub const SPEC_FILE: &str = "runtime.json";
/// Written last by [`prepare`]: its presence says the runtime in this directory is whole.
pub const MARKER_FILE: &str = "runtime.installed.json";
/// Where the releases before this layout kept the runtime, next to `current`.
const LEGACY_CACHE: &str = "runtime-cache";
const LEGACY_MARKER: &str = ".complete.json";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSpec {
    pub schema_version: u32,
    /// Names one runtime: two app versions with the same id run on the same files.
    pub id: String,
    pub url: String,
    pub sha256: String,
    /// The main executable inside the archive; it is renamed to the package's main executable.
    pub executable: String,
    /// The only `locales/*.pak` files to unpack.
    pub locales: Vec<String>,
}

impl RuntimeSpec {
    /// `None` when the package has no `runtime.json`: it carries its own main executable.
    pub fn load(app_dir: &Path) -> Result<Option<Self>> {
        let file = app_dir.join(SPEC_FILE);
        let bytes = match fs::read(&file) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("Unable to read {:?}", file)),
        };
        let spec: Self = serde_json::from_slice(&bytes).with_context(|| format!("Unable to parse {:?}", file))?;
        spec.validate()?;
        Ok(Some(spec))
    }

    pub fn validate(&self) -> Result<()> {
        let plain = |name: &str| !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':']);
        if self.schema_version != 2 {
            bail!("runtime.json has schema version {}, this updater reads version 2", self.schema_version);
        }
        if !plain(&self.id) {
            bail!("runtime.json: 'id' must be a plain name");
        }
        if !self.url.starts_with("https://") || self.url.len() <= "https://".len() {
            bail!("runtime.json: 'url' must be an HTTPS address");
        }
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("runtime.json: 'sha256' must be 64 hexadecimal digits");
        }
        if !plain(&self.executable) {
            bail!("runtime.json: 'executable' must be a file name");
        }
        if self.locales.is_empty() || !self.locales.iter().all(|locale| plain(locale)) {
            bail!("runtime.json: 'locales' must list at least one locale name");
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Installed {
    id: String,
    app_version: String,
    /// Every runtime file, relative to the app directory, with `/` separators. Includes the main executable.
    files: Vec<String>,
}

#[derive(Deserialize)]
struct LegacyComplete {
    sha256: String,
    files: Vec<String>,
}

fn read_marker(app_dir: &Path) -> Option<Installed> {
    serde_json::from_slice(&fs::read(app_dir.join(MARKER_FILE)).ok()?).ok()
}

fn all_present(dir: &Path, files: &[String]) -> bool {
    !files.is_empty() && files.iter().all(|name| dir.join(name).is_file())
}

/// Whether the app directory holds the whole runtime, prepared for this app version. Only presence is
/// checked: hashing the files on every start would cost more than the damage it could find.
pub fn is_complete(app_dir: &Path, spec: &RuntimeSpec, app_version: &str, main_exe: &str) -> bool {
    read_marker(app_dir).is_some_and(|marker| {
                            marker.id == spec.id
                            && marker.app_version == app_version
                            && marker.files.iter().any(|f| f == main_exe)
                            && all_present(app_dir, &marker.files)
                        })
}

/// A whole copy of the wanted runtime that is already on this machine.
#[derive(Debug)]
pub struct Source {
    dir: PathBuf,
    /// Without the main executable.
    files: Vec<String>,
}

/// Looks for the runtime `spec` names in a previous app directory, then in the cache directory of the
/// releases before this layout (`<root>/runtime-cache/<id>`).
pub fn find_source(previous_app_dir: Option<&Path>, previous_root: Option<&Path>, spec: &RuntimeSpec, main_exe: &str) -> Option<Source> {
    let without_main = |files: Vec<String>| files.into_iter().filter(|f| f != main_exe).collect();
    if let Some(dir) = previous_app_dir {
        if let Some(marker) = read_marker(dir) {
            if marker.id == spec.id && marker.files.iter().any(|f| f == main_exe) && all_present(dir, &marker.files) {
                return Some(Source { dir: dir.to_path_buf(),
                                     files: without_main(marker.files) });
            }
        }
    }
    let dir = previous_root?.join(LEGACY_CACHE).join(&spec.id);
    let legacy: LegacyComplete = serde_json::from_slice(&fs::read(dir.join(LEGACY_MARKER)).ok()?).ok()?;
    if !legacy.sha256.eq_ignore_ascii_case(&spec.sha256) || !legacy.files.iter().any(|f| f == main_exe) || !all_present(&dir, &legacy.files) {
        return None;
    }
    // That layout unpacked the runtime's own resources directory too; here it belongs to the application.
    let files = legacy.files.into_iter().filter(|f| !f.starts_with("resources/")).collect();
    Some(Source { dir,
                  files: without_main(files) })
}

/// Removes the cache directory of the releases before this layout.
pub fn remove_legacy_cache(root: &Path) {
    let dir = root.join(LEGACY_CACHE);
    if dir.is_dir() {
        info!("Removing the runtime cache of the previous layout: {:?}", dir);
        if let Err(e) = remove_dir_all::remove_dir_all(&dir) {
            warn!("Failed to remove {:?}: {}", dir, e);
        }
    }
}

pub enum Step {
    /// Taking the runtime from a previous app directory.
    Carrying,
    Downloading {
        done: u64,
        total: Option<u64>,
    },
    Verifying,
    Unpacking,
    /// Giving the main executable the application's icon and version.
    Branding,
}

/// What [`prepare`] needs from its caller.
pub struct Hooks<'a> {
    /// Downloads a URL to a file, reporting (bytes so far, total bytes); stops when the report returns false.
    pub download: &'a mut dyn FnMut(&str, &Path, &mut dyn FnMut(u64, Option<u64>) -> bool) -> Result<()>,
    /// Links a file to a second name; when it fails the file is copied instead.
    pub link: &'a dyn Fn(&Path, &Path) -> io::Result<()>,
    /// Stamps the application's icon and version on the main executable.
    pub brand: &'a mut dyn FnMut(&Path) -> Result<()>,
    pub step: &'a mut dyn FnMut(Step),
    pub cancelled: &'a dyn Fn() -> bool,
}

/// The error of an operation the user cancelled.
#[derive(Debug, thiserror::Error)]
#[error("Cancelled by the user.")]
pub struct Cancelled;

/// Why the runtime could not be obtained, as far as a user can act on it. The text is for the log;
/// dialogs show their own message for each kind.
#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error("The runtime download did not complete: {0}")]
    Download(String),
    #[error("The downloaded runtime does not match its SHA-256 checksum.")]
    Checksum,
    #[error("The downloaded runtime cannot be used: {0}")]
    Archive(String),
}

fn check_cancelled(hooks: &Hooks) -> Result<()> {
    if (hooks.cancelled)() {
        return Err(Cancelled.into());
    }
    Ok(())
}

/// Makes the app directory hold the whole runtime, with `main_exe` as its main executable.
///
/// `source` is a copy to take instead of downloading. `scratch` is a directory on the volume of the app
/// directory for the download; it is removed again. On failure no runtime file is left in the app
/// directory except what [`Hooks::link`] or a copy had already put there, and no marker.
pub fn prepare(app_dir: &Path,
               spec: &RuntimeSpec,
               app_version: &str,
               main_exe: &str,
               source: Option<Source>,
               scratch: &Path,
               hooks: &mut Hooks)
               -> Result<()> {
    let _ = fs::remove_file(app_dir.join(MARKER_FILE));
    let mut files = match source {
        Some(source) => {
            info!("Taking runtime {} from {:?}", spec.id, source.dir);
            (hooks.step)(Step::Carrying);
            carry(&source, app_dir, main_exe, hooks)?
        }
        None => {
            info!("Downloading runtime {} from {}", spec.id, spec.url);
            let result = obtain(app_dir, spec, main_exe, scratch, hooks);
            let _ = remove_dir_all::remove_dir_all(scratch);
            result?
        }
    };
    check_cancelled(hooks)?;
    (hooks.step)(Step::Branding);
    // The executable gets its name only when it is finished: shortcuts lead to that name, and whatever
    // follows them (the shell reading the icon, a scanner) would otherwise open the file while it is
    // still being written.
    let unfinished = app_dir.join(unfinished_name(main_exe));
    (hooks.brand)(&unfinished)?;
    rename_patiently(&unfinished, &app_dir.join(main_exe))?;
    files.push(main_exe.to_string());
    let marker = Installed { id: spec.id.clone(),
                             app_version: app_version.to_string(),
                             files };
    fs::write(app_dir.join(MARKER_FILE), serde_json::to_vec(&marker)?)?;
    Ok(())
}

/// The name the main executable has in the app directory until [`prepare`] has finished it.
fn unfinished_name(main_exe: &str) -> String {
    format!("{main_exe}.unfinished")
}

/// Renames a file, trying again for a few seconds when something else has it open for a moment.
fn rename_patiently(from: &Path, to: &Path) -> Result<()> {
    let _ = fs::remove_file(to);
    let mut attempt = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 40 => {
                attempt += 1;
                debug!("Renaming {:?} failed ({}), trying again.", from, e);
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            Err(e) => return Err(e).with_context(|| format!("Unable to rename {:?} to {:?}", from, to)),
        }
    }
}

fn carry(source: &Source, app_dir: &Path, main_exe: &str, hooks: &Hooks) -> Result<Vec<String>> {
    for name in &source.files {
        check_cancelled(hooks)?;
        let from = source.dir.join(name);
        let to = app_dir.join(name);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::remove_file(&to);
        if (hooks.link)(&from, &to).is_err() {
            fs::copy(&from, &to).with_context(|| format!("Unable to copy {:?}", from))?;
        }
    }
    // The main executable gets this version's resources written into it, so it is never shared.
    let _ = fs::remove_file(app_dir.join(main_exe));
    fs::copy(source.dir.join(main_exe), app_dir.join(unfinished_name(main_exe)))?;
    Ok(source.files.clone())
}

fn obtain(app_dir: &Path, spec: &RuntimeSpec, main_exe: &str, scratch: &Path, hooks: &mut Hooks) -> Result<Vec<String>> {
    let _ = remove_dir_all::remove_dir_all(scratch);
    fs::create_dir_all(scratch)?;
    let archive = scratch.join("runtime.zip");
    let unpacked = scratch.join("runtime");

    (hooks.step)(Step::Downloading { done: 0, total: None });
    let cancelled = hooks.cancelled;
    let step = &mut *hooks.step;
    let downloaded = (hooks.download)(&spec.url, &archive, &mut |done, total| {
        step(Step::Downloading { done, total });
        !cancelled()
    });
    check_cancelled(hooks)?;
    downloaded.map_err(|e| Failure::Download(format!("{:#}", e)))?;

    (hooks.step)(Step::Verifying);
    verify_sha256(&archive, &spec.sha256)?;
    check_cancelled(hooks)?;

    (hooks.step)(Step::Unpacking);
    let files = unpack(&archive, &unpacked, spec, main_exe, hooks.cancelled).map_err(|e| match e.downcast::<Cancelled>() {
                                                                                Ok(cancelled) => cancelled.into(),
                                                                                Err(e) => anyhow::Error::from(Failure::Archive(format!("{:#}", e))),
                                                                            })?;
    for name in &files {
        let to = if name == main_exe {
            app_dir.join(unfinished_name(main_exe))
        } else {
            app_dir.join(name)
        };
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::remove_file(app_dir.join(name));
        let _ = fs::remove_file(&to);
        fs::rename(unpacked.join(name), &to).with_context(|| format!("Unable to place {:?}", to))?;
    }
    Ok(files.into_iter().filter(|f| f != main_exe).collect())
}

fn verify_sha256(file: &Path, expected: &str) -> Result<()> {
    let mut reader = File::open(file)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let actual: String = digest.finalize().iter().map(|byte| format!("{:02x}", byte)).collect();
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(Failure::Checksum.into());
    }
    Ok(())
}

/// Unpacks the archive, leaving out the locales not asked for and the runtime's own `resources`
/// directory, and renames the main executable. Returns the files written, `/` separated.
fn unpack(archive: &Path, destination: &Path, spec: &RuntimeSpec, main_exe: &str, cancelled: &dyn Fn() -> bool) -> Result<Vec<String>> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    let mut files = Vec::new();
    for index in 0..zip.len() {
        if cancelled() {
            return Err(Cancelled.into());
        }
        let mut entry = zip.by_index(index)?;
        let raw = entry.name()?.to_string();
        let is_link = entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000);
        let name = entry.enclosed_name()
                        .filter(|path| path.components().all(|part| matches!(part, Component::Normal(_))));
        let Some(name) = name.filter(|_| !is_link && !raw.contains(['\\', ':'])) else {
            bail!("The runtime archive has an entry this updater refuses: {:?}", raw);
        };
        let mut parts: Vec<String> = name.iter().map(|part| part.to_string_lossy().into_owned()).collect();
        if entry.is_dir() || parts.is_empty() || parts[0] == "resources" {
            continue;
        }
        if parts[0] == "locales" && !spec.locales.iter().any(|locale| parts.len() == 2 && parts[1] == format!("{locale}.pak")) {
            continue;
        }
        if parts.len() == 1 && parts[0] == spec.executable {
            parts[0] = main_exe.to_string();
        }
        let relative = parts.join("/");
        let output = destination.join(&relative);
        fs::create_dir_all(output.parent().ok_or_else(|| anyhow!("Missing parent of {:?}", output))?)?;
        io::copy(&mut entry, &mut File::create(&output)?)?;
        files.push(relative);
    }
    if !files.iter().any(|f| f == main_exe) {
        bail!("The runtime archive has no {:?}.", spec.executable);
    }
    for locale in &spec.locales {
        if !files.contains(&format!("locales/{locale}.pak")) {
            bail!("The runtime archive is missing locale {}.", locale);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        io::Write,
    };
    use zip::{write::SimpleFileOptions, ZipWriter};

    const MAIN: &str = "App.exe";

    fn archive(file: &Path, entries: &[(&str, &str)]) -> String {
        let mut writer = ZipWriter::new(File::create(file).unwrap());
        for (name, content) in entries {
            writer.start_file(*name, SimpleFileOptions::default()).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        Sha256::digest(fs::read(file).unwrap()).iter()
                                               .map(|byte| format!("{:02x}", byte))
                                               .collect()
    }

    fn spec(sha256: &str) -> RuntimeSpec {
        RuntimeSpec { schema_version: 2,
                      id: "44.7.2-win32-x64".into(),
                      url: "https://example.invalid/electron.zip".into(),
                      sha256: sha256.into(),
                      executable: "electron.exe".into(),
                      locales: vec!["en-US".into(), "zh-CN".into()] }
    }

    const ENTRIES: &[(&str, &str)] = &[("electron.exe", "exe"),
                                       ("ffmpeg.dll", "dll"),
                                       ("resources.pak", "pak"),
                                       ("locales/en-US.pak", "en"),
                                       ("locales/zh-CN.pak", "zh"),
                                       ("locales/fr.pak", "fr"),
                                       ("resources/default_app.asar", "default")];

    struct Fixture {
        root: tempfile::TempDir,
        zip: PathBuf,
        spec: RuntimeSpec,
        downloads: Cell<u32>,
        fail_download: Cell<bool>,
        fail_brand: Cell<bool>,
        steps: RefCell<Vec<&'static str>>,
        cancel_after_download: Cell<bool>,
        fail_link: Cell<bool>,
    }

    impl Fixture {
        fn new(entries: &[(&str, &str)]) -> Self {
            let root = tempfile::tempdir().unwrap();
            let zip = root.path().join("served.zip");
            let sha256 = archive(&zip, entries);
            Self { root,
                   zip,
                   spec: spec(&sha256),
                   downloads: Cell::new(0),
                   fail_download: Cell::new(false),
                   fail_brand: Cell::new(false),
                   steps: RefCell::new(Vec::new()),
                   cancel_after_download: Cell::new(false),
                   fail_link: Cell::new(false) }
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.root.path().join(name);
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        fn prepare(&self, app_dir: &Path, version: &str, source: Option<Source>) -> Result<()> {
            let cancelled = Cell::new(false);
            let mut download = |_: &str, to: &Path, report: &mut dyn FnMut(u64, Option<u64>) -> bool| -> Result<()> {
                self.downloads.set(self.downloads.get() + 1);
                if self.fail_download.get() {
                    bail!("connection reset");
                }
                fs::copy(&self.zip, to)?;
                report(1, Some(1));
                cancelled.set(self.cancel_after_download.get());
                Ok(())
            };
            let link = |from: &Path, to: &Path| {
                if self.fail_link.get() {
                    Err(io::Error::other("no links"))
                } else {
                    fs::hard_link(from, to)
                }
            };
            let mut brand = |exe: &Path| -> Result<()> {
                // The executable is finished under another name and only then becomes the main executable.
                assert!(!app_dir.join(MAIN).exists());
                assert_ne!(exe, app_dir.join(MAIN));
                if self.fail_brand.get() {
                    bail!("the file is in use");
                }
                fs::write(exe, format!("{} branded {version}", fs::read_to_string(exe)?.split(' ').next().unwrap()))?;
                Ok(())
            };
            let mut step = |step: Step| {
                self.steps.borrow_mut().push(match step {
                                                 Step::Carrying => "carry",
                                                 Step::Downloading { .. } => "download",
                                                 Step::Verifying => "verify",
                                                 Step::Unpacking => "unpack",
                                                 Step::Branding => "brand",
                                             })
            };
            let is_cancelled = || cancelled.get();
            let mut hooks = Hooks { download: &mut download,
                                    link: &link,
                                    brand: &mut brand,
                                    step: &mut step,
                                    cancelled: &is_cancelled };
            prepare(app_dir, &self.spec, version, MAIN, source, &self.root.path().join("scratch"), &mut hooks)
        }
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut files: Vec<String> = crate::shared::fastzip::enumerate_files_relative(dir).into_iter()
                                                                                          .map(|path| path.to_string_lossy().replace('\\', "/"))
                                                                                          .collect();
        files.sort();
        files
    }

    #[test]
    fn spec_is_validated() {
        let good = spec(&"a".repeat(64));
        assert!(good.validate().is_ok());
        let broken: [fn(&mut RuntimeSpec); 8] = [|s| s.schema_version = 1,
                                                 |s| s.id = "../x".into(),
                                                 |s| s.id = String::new(),
                                                 |s| s.url = "http://example.invalid/e.zip".into(),
                                                 |s| s.sha256 = "abc".into(),
                                                 |s| s.sha256 = "g".repeat(64),
                                                 |s| s.executable = "bin/electron.exe".into(),
                                                 |s| s.locales.clear()];
        for change in broken {
            let mut spec = good.clone();
            change(&mut spec);
            assert!(spec.validate().is_err());
        }
    }

    #[test]
    fn spec_is_read_from_the_app_directory_and_ignores_fields_it_does_not_use() {
        let dir = tempfile::tempdir().unwrap();
        assert!(RuntimeSpec::load(dir.path()).unwrap().is_none());
        let text = format!(r#"{{"schemaVersion":2,"id":"1.0.0-win32-x64","url":"https://example.invalid/e.zip","sha256":"{}","executable":"electron.exe","locales":["en-US"],"updateUrl":"https://example.invalid/updates"}}"#,
                           "0".repeat(64));
        fs::write(dir.path().join(SPEC_FILE), text).unwrap();
        assert_eq!(RuntimeSpec::load(dir.path()).unwrap().unwrap().id, "1.0.0-win32-x64");
        fs::write(dir.path().join(SPEC_FILE), "{").unwrap();
        assert!(RuntimeSpec::load(dir.path()).is_err());
    }

    #[test]
    fn downloads_unpacks_and_marks_the_runtime() {
        let fixture = Fixture::new(ENTRIES);
        let app = fixture.dir("current");
        fs::create_dir_all(app.join("resources/app")).unwrap();
        fs::write(app.join("resources/app/package.json"), "{}").unwrap();
        assert!(!is_complete(&app, &fixture.spec, "1.0.0", MAIN));

        fixture.prepare(&app, "1.0.0", None).unwrap();

        assert_eq!(files_in(&app),
                   ["App.exe",
                    "ffmpeg.dll",
                    "locales/en-US.pak",
                    "locales/zh-CN.pak",
                    "resources.pak",
                    "resources/app/package.json",
                    MARKER_FILE]);
        assert_eq!(fs::read_to_string(app.join(MAIN)).unwrap(), "exe branded 1.0.0");
        assert_eq!(*fixture.steps.borrow(), ["download", "download", "verify", "unpack", "brand"]);
        assert!(is_complete(&app, &fixture.spec, "1.0.0", MAIN));
        assert!(!fixture.root.path().join("scratch").exists());
    }

    #[test]
    fn a_runtime_is_incomplete_for_another_id_another_version_or_a_missing_file() {
        let fixture = Fixture::new(ENTRIES);
        let app = fixture.dir("current");
        fixture.prepare(&app, "1.0.0", None).unwrap();

        assert!(!is_complete(&app, &fixture.spec, "1.0.1", MAIN));
        let mut other = fixture.spec.clone();
        other.id = "45.0.0-win32-x64".into();
        assert!(!is_complete(&app, &other, "1.0.0", MAIN));
        fs::remove_file(app.join("ffmpeg.dll")).unwrap();
        assert!(!is_complete(&app, &fixture.spec, "1.0.0", MAIN));
        fs::write(app.join("ffmpeg.dll"), "dll").unwrap();
        assert!(is_complete(&app, &fixture.spec, "1.0.0", MAIN));
        fs::remove_file(app.join(MARKER_FILE)).unwrap();
        assert!(!is_complete(&app, &fixture.spec, "1.0.0", MAIN));
    }

    #[test]
    fn a_wrong_checksum_an_interrupted_download_and_a_cancel_leave_the_app_directory_alone() {
        let fixture = Fixture::new(ENTRIES);
        let app = fixture.dir("current");
        fs::write(app.join("sq.version"), "manifest").unwrap();

        let mut wrong = Fixture::new(ENTRIES);
        wrong.spec.sha256 = "0".repeat(64);
        let error = wrong.prepare(&app, "1.0.0", None).unwrap_err();
        assert!(matches!(error.downcast_ref::<Failure>(), Some(Failure::Checksum)));
        assert_eq!(files_in(&app), ["sq.version"]);

        let mut truncated = Fixture::new(ENTRIES);
        let bytes = fs::read(&truncated.zip).unwrap();
        fs::write(&truncated.zip, &bytes[..bytes.len() / 2]).unwrap();
        assert!(truncated.prepare(&app, "1.0.0", None).is_err());
        truncated.spec.sha256 = Sha256::digest(&bytes[..bytes.len() / 2]).iter().map(|b| format!("{:02x}", b)).collect();
        let error = truncated.prepare(&app, "1.0.0", None).unwrap_err();
        assert!(matches!(error.downcast_ref::<Failure>(), Some(Failure::Archive(_))));
        assert_eq!(files_in(&app), ["sq.version"]);

        fixture.fail_download.set(true);
        let error = fixture.prepare(&app, "1.0.0", None).unwrap_err();
        assert!(matches!(error.downcast_ref::<Failure>(), Some(Failure::Download(reason)) if reason.contains("connection reset")));
        assert_eq!(files_in(&app), ["sq.version"]);
        fixture.fail_download.set(false);

        fixture.cancel_after_download.set(true);
        let error = fixture.prepare(&app, "1.0.0", None).unwrap_err();
        assert!(error.downcast_ref::<Cancelled>().is_some());
        assert_eq!(files_in(&app), ["sq.version"]);
        assert!(!fixture.root.path().join("scratch").exists());
    }

    #[test]
    fn refuses_archives_that_escape_or_lack_what_the_spec_names() {
        let app_dir = tempfile::tempdir().unwrap();
        for entries in [&[("electron.exe", "exe"),
                          ("../outside", "x"),
                          ("locales/en-US.pak", "en"),
                          ("locales/zh-CN.pak", "zh")][..],
                        &[("electron.exe", "exe"),
                          ("C:/outside", "x"),
                          ("locales/en-US.pak", "en"),
                          ("locales/zh-CN.pak", "zh")][..],
                        &[("electron.exe", "exe"), ("locales/en-US.pak", "en")][..],
                        &[("other.exe", "exe"), ("locales/en-US.pak", "en"), ("locales/zh-CN.pak", "zh")][..]]
        {
            let fixture = Fixture::new(entries);
            let app = app_dir.path().join("current");
            fs::create_dir_all(&app).unwrap();
            assert!(fixture.prepare(&app, "1.0.0", None).is_err());
            assert!(files_in(&app).is_empty());
            assert!(!fixture.root.path().join("outside").exists());
        }
    }

    #[test]
    fn an_update_takes_the_runtime_from_the_previous_app_directory_without_downloading() {
        let fixture = Fixture::new(ENTRIES);
        let old = fixture.dir("current");
        fixture.prepare(&old, "1.0.0", None).unwrap();
        let new = fixture.dir("new");

        let source = find_source(Some(&old), Some(fixture.root.path()), &fixture.spec, MAIN).unwrap();
        fixture.prepare(&new, "1.1.0", Some(source)).unwrap();

        assert_eq!(fixture.downloads.get(), 1);
        assert!(is_complete(&new, &fixture.spec, "1.1.0", MAIN));
        assert_eq!(fs::read_to_string(new.join(MAIN)).unwrap(), "exe branded 1.1.0");
        // The previous directory still runs: its executable was copied, not rewritten.
        assert_eq!(fs::read_to_string(old.join(MAIN)).unwrap(), "exe branded 1.0.0");
        assert!(is_complete(&old, &fixture.spec, "1.0.0", MAIN));
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(old.join("ffmpeg.dll")).unwrap().ino(),
                       fs::metadata(new.join("ffmpeg.dll")).unwrap().ino());
            assert_ne!(fs::metadata(old.join(MAIN)).unwrap().ino(), fs::metadata(new.join(MAIN)).unwrap().ino());
        }
    }

    #[test]
    fn an_executable_that_could_not_be_finished_never_gets_its_name() {
        let fixture = Fixture::new(ENTRIES);
        let old = fixture.dir("current");
        fixture.prepare(&old, "1.0.0", None).unwrap();
        for source in [true, false] {
            let new = fixture.dir(if source {
                                      "carried"
                                  } else {
                                      "downloaded"
                                  });
            fixture.fail_brand.set(true);
            let source = source.then(|| find_source(Some(&old), None, &fixture.spec, MAIN).unwrap());
            assert!(fixture.prepare(&new, "1.1.0", source).is_err());
            assert!(!new.join(MAIN).exists());
            assert!(!is_complete(&new, &fixture.spec, "1.1.0", MAIN));
            // Trying again in the same directory finishes it.
            fixture.fail_brand.set(false);
            fixture.prepare(&new, "1.1.0", None).unwrap();
            assert!(is_complete(&new, &fixture.spec, "1.1.0", MAIN));
            assert!(!new.join(unfinished_name(MAIN)).exists());
            assert_eq!(fs::read_to_string(new.join(MAIN)).unwrap(), "exe branded 1.1.0");
        }
    }

    #[test]
    fn files_are_copied_where_they_cannot_be_linked() {
        let fixture = Fixture::new(ENTRIES);
        let old = fixture.dir("current");
        fixture.prepare(&old, "1.0.0", None).unwrap();
        let new = fixture.dir("new");
        fixture.fail_link.set(true);

        let source = find_source(Some(&old), None, &fixture.spec, MAIN).unwrap();
        fixture.prepare(&new, "1.1.0", Some(source)).unwrap();

        assert!(is_complete(&new, &fixture.spec, "1.1.0", MAIN));
        assert_eq!(fs::read_to_string(new.join("locales/zh-CN.pak")).unwrap(), "zh");
    }

    #[test]
    fn a_previous_directory_with_another_runtime_or_a_missing_file_is_not_a_source() {
        let fixture = Fixture::new(ENTRIES);
        let old = fixture.dir("current");
        assert!(find_source(Some(&old), None, &fixture.spec, MAIN).is_none());
        fixture.prepare(&old, "1.0.0", None).unwrap();

        let mut other = fixture.spec.clone();
        other.id = "45.0.0-win32-x64".into();
        assert!(find_source(Some(&old), None, &other, MAIN).is_none());
        fs::remove_file(old.join("resources.pak")).unwrap();
        assert!(find_source(Some(&old), None, &fixture.spec, MAIN).is_none());
    }

    #[test]
    fn the_cache_of_the_previous_layout_is_a_source_when_its_checksum_matches() {
        let fixture = Fixture::new(ENTRIES);
        let root = fixture.dir("install");
        let cache = root.join(LEGACY_CACHE).join(&fixture.spec.id);
        let files = ["App.exe",
                     "ffmpeg.dll",
                     "resources.pak",
                     "locales/en-US.pak",
                     "locales/zh-CN.pak",
                     "resources/default_app.asar"];
        for name in files {
            fs::create_dir_all(cache.join(name).parent().unwrap()).unwrap();
            fs::write(cache.join(name), name).unwrap();
        }
        let marker = |sha256: &str| serde_json::json!({ "sha256": sha256, "files": files }).to_string();
        // The bootstrap of that layout had no marker of ours in `current`.
        let old = root.join("current");
        fs::create_dir_all(&old).unwrap();

        fs::write(cache.join(LEGACY_MARKER), marker(&"0".repeat(64))).unwrap();
        assert!(find_source(Some(&old), Some(&root), &fixture.spec, MAIN).is_none());

        fs::write(cache.join(LEGACY_MARKER), marker(&fixture.spec.sha256.to_uppercase())).unwrap();
        let source = find_source(Some(&old), Some(&root), &fixture.spec, MAIN).unwrap();
        let new = fixture.dir("new");
        fixture.prepare(&new, "2.0.0", Some(source)).unwrap();

        assert_eq!(fixture.downloads.get(), 0);
        assert_eq!(files_in(&new),
                   ["App.exe",
                    "ffmpeg.dll",
                    "locales/en-US.pak",
                    "locales/zh-CN.pak",
                    "resources.pak",
                    MARKER_FILE]);
        assert!(is_complete(&new, &fixture.spec, "2.0.0", MAIN));

        remove_legacy_cache(&root);
        assert!(!root.join(LEGACY_CACHE).exists());
        assert_eq!(fs::read_to_string(new.join("ffmpeg.dll")).unwrap(), "ffmpeg.dll");
    }
}
