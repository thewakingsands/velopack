//! Prepares the runtime of an app (see `shared::runtime`) on Windows: a real download, the progress
//! dialog of the running operation, and the icon and version of the launcher stub for the runtime's
//! main executable.

use super::brand;
use crate::dialogs::{self, progress::ProgressReporter};
use crate::shared::runtime::{self, Cancelled, Failure, Hooks, RuntimeSpec, Step};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use velopack::{download, locator::VelopackLocator};

pub struct Request<'a> {
    /// The directory to complete: `current`, or the directory that is about to become it.
    pub app_dir: &'a Path,
    /// Carries the manifest of the version in `app_dir`.
    pub locator: &'a VelopackLocator,
    /// An app directory and install root the runtime may be taken from instead of downloading it.
    pub previous_app_dir: Option<&'a Path>,
    pub previous_root: Option<&'a Path>,
    /// The launcher stub of the version in `app_dir`; its icon and version go onto the main executable.
    pub stub: &'a Path,
}

/// The stub vpk made for this app, in `dir`. vpk names it after the app's title, not its main executable.
pub fn stub_path(dir: &Path, locator: &VelopackLocator) -> PathBuf {
    dir.join(format!("{}.exe", locator.get_manifest_title()))
}

/// Whether the package in `app_dir` names a runtime that is not whole there.
pub fn is_missing(app_dir: &Path, locator: &VelopackLocator) -> Result<bool> {
    let manifest = locator.get_manifest();
    Ok(match RuntimeSpec::load(app_dir)? {
        Some(spec) => !runtime::is_complete(app_dir, &spec, &manifest.version.to_string(), &manifest.main_exe),
        None => false,
    })
}

/// Completes the runtime in `request.app_dir`. Does nothing for a package without `runtime.json`.
/// Fails with [`Cancelled`] when the user pressed Cancel on the progress dialog, before or during the work.
pub fn prepare(request: &Request, reporter: &dyn ProgressReporter) -> Result<()> {
    let cancelled = || dialogs::progress::cancel_requested();
    if cancelled() {
        return Err(Cancelled.into());
    }
    let Some(spec) = RuntimeSpec::load(request.app_dir)? else {
        return Ok(());
    };
    let manifest = request.locator.get_manifest();
    let version = manifest.version.to_string();
    let source = runtime::find_source(request.previous_app_dir, request.previous_root, &spec, &manifest.main_exe);
    let scratch = request.locator.get_temp_dir_rand16();

    let mut download = |url: &str, to: &Path, report: &mut dyn FnMut(u64, Option<u64>) -> bool| -> Result<()> {
        if download::download_url_to_file_while(url, to, |done, total| report(done, total))? {
            Ok(())
        } else {
            Err(Cancelled.into())
        }
    };
    let mut brand = |main_exe: &Path| -> Result<()> {
        if !request.stub.is_file() {
            return Err(anyhow!("The package has no launcher stub at {:?} to take the icon and version from.",
                               request.stub));
        }
        brand::copy_icon_and_version(request.stub, main_exe)
    };
    let size = |bytes: u64| format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0));
    let status = |text: String| reporter.set_text(&text);
    let mut step = |step: Step| {
        if cancelled() {
            reporter.set_indeterminate();
            status(dialogs::locale_strings::progress_cancelling());
            return;
        }
        match step {
            Step::Downloading { done, total } => {
                match total.filter(|total| *total > 0) {
                    Some(total) => reporter.set_progress((done.min(total) * 100 / total) as i16),
                    None => reporter.set_indeterminate(),
                }
                // Until the server has answered there is nothing to count.
                status(if done == 0 && total.is_none() {
                           dialogs::locale_strings::runtime_download_starting()
                       } else {
                           dialogs::locale_strings::runtime_downloading(&size(done), total.map(size).as_deref())
                       });
            }
            Step::Verifying => {
                reporter.set_indeterminate();
                status(dialogs::locale_strings::runtime_verifying());
            }
            Step::Unpacking => status(dialogs::locale_strings::runtime_unpacking()),
            Step::Carrying | Step::Branding => {
                reporter.set_indeterminate();
                status(dialogs::locale_strings::runtime_finishing());
            }
        }
    };
    let link = |from: &Path, to: &Path| std::fs::hard_link(from, to);
    let mut hooks = Hooks { download: &mut download,
                            link: &link,
                            brand: &mut brand,
                            step: &mut step,
                            cancelled: &cancelled };
    runtime::prepare(request.app_dir, &spec, &version, &manifest.main_exe, source, &scratch, &mut hooks)
}

/// What to tell the user when [`prepare`] failed: the cause in the system's language, and what to do.
pub fn failure_message(error: &anyhow::Error) -> String {
    match error.downcast_ref::<Failure>() {
        Some(Failure::Download(_)) => dialogs::locale_strings::runtime_failed_download(),
        Some(Failure::Checksum) | Some(Failure::Archive(_)) => dialogs::locale_strings::runtime_failed_invalid(),
        None => dialogs::locale_strings::runtime_failed_other(),
    }
}

/// Called before an installed app is started through its stub: completes a runtime that is missing
/// from `current`, in its own progress dialog, and makes sure the Start menu has a shortcut.
/// Returns false when the app should not be started: the user cancelled or gave up retrying.
pub fn repair_before_start(locator: &VelopackLocator) -> bool {
    let app_dir = locator.get_current_bin_dir();
    if !app_dir.join(runtime::SPEC_FILE).is_file() {
        return true;
    }
    let title = locator.get_manifest_title();
    loop {
        let result = (|| -> Result<()> {
            if !is_missing(&app_dir, locator)? {
                return Ok(());
            }
            info!("The runtime in {:?} is not whole, preparing it.", app_dir);
            let _mutex = locator.try_get_exclusive_lock()?;
            let reporter = dialogs::progress::show_runtime_progress(&title);
            let root = locator.get_root_dir();
            let stub = stub_path(&root, locator);
            let request = Request { app_dir: &app_dir,
                                    locator,
                                    previous_app_dir: None,
                                    previous_root: Some(&root),
                                    stub: &stub };
            let result = prepare(&request, reporter.as_ref());
            reporter.close();
            result
        })();
        match result {
            Ok(()) => break,
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => {
                info!("The user cancelled preparing the runtime.");
                return false;
            }
            Err(e) => {
                error!("Unable to prepare the runtime: {:?}", e);
                if !dialogs::show_runtime_failed(&title, &failure_message(&e)) {
                    return false;
                }
            }
        }
    }
    super::ensure_start_menu_shortcut(locator);
    true
}
