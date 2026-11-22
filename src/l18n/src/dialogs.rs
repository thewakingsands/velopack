use crate::locale_strings;
use anyhow::{bail, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use xdialog::{XDialogError, XDialogIcon, XDialogIconSource, XDialogOptions, XDialogResult};

static SILENT: AtomicBool = AtomicBool::new(false);
static DIALOG_TIMEOUT_MS: AtomicU64 = AtomicU64::new(0);
static APP_ICON: OnceLock<&'static [u8]> = OnceLock::new();

/// Sets the app icon (an `.ico`, `.png` or `.icns` image) used as the window / taskbar icon of
/// every dialog, and shown in place of the information icon in general dialogs. Call once at startup.
pub fn set_app_icon(icon: Vec<u8>) {
    let _ = APP_ICON.set(icon.leak());
}

fn app_icon_source() -> Option<XDialogIconSource> {
    APP_ICON.get().map(|icon| XDialogIconSource::Bytes((*icon).into()))
}

/// The icon for general (non-warning, non-error) dialogs: the app icon if one is set, else `fallback`.
pub(crate) fn general_icon(fallback: XDialogIcon) -> XDialogIcon {
    if APP_ICON.get().is_some() {
        XDialogIcon::Custom
    } else {
        fallback
    }
}

pub(crate) fn dialog_options(title: &str, header: &str, body: &str, icon: XDialogIcon, buttons: Vec<String>) -> XDialogOptions {
    XDialogOptions {
        title: title.to_string(),
        main_instruction: header.to_string(),
        message: body.to_string(),
        icon,
        icon_source: app_icon_source(),
        buttons,
    }
}

fn show_message(title: &str, header: &str, body: &str, icon: XDialogIcon, buttons: Vec<String>) -> Result<XDialogResult, XDialogError> {
    let dialog = xdialog::show_message(dialog_options(title, header, body, icon, buttons));
    match get_dialog_timeout() {
        Some(timeout) => dialog.wait_timeout(timeout),
        None => dialog.wait(),
    }
}

pub fn set_silent(silent: bool) {
    SILENT.store(silent, Ordering::Relaxed);
    xdialog::set_silent_mode(silent);
}

pub fn get_silent() -> bool {
    SILENT.load(Ordering::Relaxed)
}

pub fn set_dialog_timeout(timeout: Option<Duration>) {
    let ms = timeout.map(|d| d.as_millis() as u64).unwrap_or(0);
    DIALOG_TIMEOUT_MS.store(ms, Ordering::Relaxed);
}

pub fn get_dialog_timeout() -> Option<Duration> {
    match DIALOG_TIMEOUT_MS.load(Ordering::Relaxed) {
        0 => None,
        ms => Some(Duration::from_millis(ms)),
    }
}

fn show_error(title: &str, header: &str, body: &str) {
    if get_silent() {
        return;
    }
    let _ = show_message(title, header, body, XDialogIcon::Error, vec![locale_strings::btn_ok()]);
}

fn show_warn(title: &str, header: &str, body: &str) {
    if get_silent() {
        return;
    }
    let _ = show_message(title, header, body, XDialogIcon::Warning, vec![locale_strings::btn_ok()]);
}

fn show_info(title: &str, header: &str, body: &str) {
    if get_silent() {
        return;
    }
    let _ = show_message(
        title,
        header,
        body,
        general_icon(XDialogIcon::Information),
        vec![locale_strings::btn_ok()],
    );
}

fn show_ok_cancel(title: &str, header: &str, body: &str, ok_text: Option<&str>) -> bool {
    if get_silent() {
        return false;
    }
    let ok_label = ok_text.map(|s| s.to_string()).unwrap_or_else(locale_strings::btn_ok);
    let cancel_label = locale_strings::btn_cancel();
    let result = show_message(title, header, body, general_icon(XDialogIcon::Warning), vec![ok_label, cancel_label]);
    if matches!(result, Ok(XDialogResult::TimeoutElapsed)) {
        warn!("Dialog timed out, treating as cancel.");
    }
    matches!(result, Ok(XDialogResult::ButtonPressed(0)))
}

pub fn ask_user_to_elevate(app_title: &str, new_version: &str) -> Result<()> {
    if get_silent() {
        bail!("Not allowed to ask for elevated permissions because --silent flag is set.");
    }

    let title = locale_strings::title_update(app_title);
    let header = locale_strings::elevate_header(app_title);
    let body = locale_strings::elevate_body(app_title, new_version);
    let ok_text = locale_strings::btn_install_update();

    info!("Showing user elevation prompt?");
    if show_ok_cancel(&title, &header, &body, Some(&ok_text)) {
        info!("User answered yes to elevation...");
        Ok(())
    } else {
        bail!("User cancelled elevation prompt.");
    }
}

pub fn show_restart_required(app_name: &str, _app_version: &str) {
    let title = locale_strings::title_setup(app_name);
    let header = locale_strings::restart_header(app_name);
    let body = locale_strings::restart_body();
    show_warn(&title, &header, &body);
}

pub fn show_update_missing_dependencies_dialog(app_name: &str, dependency_string: &str, _from_ver: &str, _to_ver: &str) -> bool {
    if get_silent() {
        warn!("Cancelling pre-requisite installation because silent flag is true.");
        return false;
    }

    let title = locale_strings::title_update(app_name);
    let header = locale_strings::missing_deps_header(app_name);
    let body = locale_strings::missing_deps_body(app_name, dependency_string);
    let button = locale_strings::btn_install();
    show_ok_cancel(&title, &header, &body, Some(&button))
}

pub fn show_setup_missing_dependencies_dialog(app_name: &str, _app_version: &str, dependency_string: &str) -> bool {
    if get_silent() {
        return true;
    }

    let title = locale_strings::title_setup(app_name);
    let header = locale_strings::missing_deps_header(app_name);
    let body = locale_strings::missing_deps_body(app_name, dependency_string);
    let button = locale_strings::btn_install();
    show_ok_cancel(&title, &header, &body, Some(&button))
}

pub fn show_uninstall_complete_with_errors_dialog(app_title: &str, log_path: Option<&Path>) {
    if get_silent() {
        return;
    }

    let title = locale_strings::title_uninstall(app_title);
    let header = locale_strings::uninstall_errors_header(app_title);
    let body = locale_strings::uninstall_errors_body(app_title);

    let has_log = log_path.map(|p| p.exists()).unwrap_or(false);
    if has_log {
        let log_str = log_path.unwrap().to_string_lossy().to_string();
        let footer = locale_strings::uninstall_errors_log(&log_str);
        let open_log_label = locale_strings::btn_open_log();
        let ok_label = locale_strings::btn_ok();
        let full_body = format!("{}\n\n{}", body, footer);
        let result = show_message(&title, &header, &full_body, XDialogIcon::Warning, vec![ok_label, open_log_label]);
        if matches!(result, Ok(XDialogResult::ButtonPressed(1))) {
            open_path(Path::new(&log_str));
        }
    } else {
        show_warn(&title, &header, &body);
    }
}

/// What the user chose when Setup found the application already installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingInstall {
    /// Replace the existing installation with this package.
    Install,
    /// Leave the installation as it is and start the application.
    Start,
    Cancel,
}

/// Asks what to do with an existing installation of the same or a newer version, or one whose
/// version cannot be read (`installed_version` is `None`: files are missing).
pub fn show_existing_install_dialog(
    app_title: &str,
    app_version: &semver::Version,
    root_path: &Path,
    installed_version: Option<&semver::Version>,
) -> ExistingInstall {
    if get_silent() {
        return ExistingInstall::Install;
    }

    let title = locale_strings::title_setup(app_title);
    let instruction;
    let body;
    let install_label;
    match installed_version {
        Some(old_version) if old_version > app_version => {
            instruction = locale_strings::overwrite_newer_installed(app_title);
            body = locale_strings::overwrite_downgrade_body(app_title, &old_version.to_string());
            install_label = locale_strings::btn_downgrade();
        }
        Some(old_version) => {
            instruction = locale_strings::overwrite_already_installed(app_title);
            body = locale_strings::overwrite_repair_body(app_title, &old_version.to_string());
            install_label = locale_strings::btn_repair();
        }
        None => {
            instruction = locale_strings::overwrite_incomplete_header(app_title);
            body = locale_strings::overwrite_incomplete_body(app_title);
            install_label = locale_strings::btn_repair();
        }
    }
    let footer = locale_strings::overwrite_footer(&root_path.display().to_string());
    let full_body = format!("{}\n\n{}", body, footer);

    // Buttons appear in reverse order and the last one is the default: starting what is installed
    // comes first whenever it can be started.
    let mut buttons = vec![locale_strings::btn_cancel(), install_label];
    if installed_version.is_some() {
        buttons.push(locale_strings::btn_start());
    }
    match show_message(&title, &instruction, &full_body, general_icon(XDialogIcon::Warning), buttons) {
        Ok(XDialogResult::ButtonPressed(1)) => ExistingInstall::Install,
        Ok(XDialogResult::ButtonPressed(2)) => ExistingInstall::Start,
        Ok(XDialogResult::TimeoutElapsed) => {
            warn!("Existing installation dialog timed out, treating as cancel.");
            ExistingInstall::Cancel
        }
        _ => ExistingInstall::Cancel,
    }
}

// --- Helper functions that encapsulate locale_strings calls ---

pub fn show_generic_error(program_name: &str, error_string: &str) {
    let title = locale_strings::error_title(program_name);
    let header = locale_strings::error_header();
    show_error(&title, &header, error_string);
}

pub fn show_install_hook_warning(app_title: &str) {
    let title = locale_strings::title_setup(app_title);
    let header = locale_strings::install_hook_header(app_title);
    let body = locale_strings::install_hook_body();
    show_warn(&title, &header, &body);
}

pub fn show_uninstall_complete(app_title: &str) {
    let title = locale_strings::title_uninstall(app_title);
    let header = locale_strings::uninstall_header(app_title);
    let body = locale_strings::uninstall_body();
    show_info(&title, &header, &body);
}

pub fn show_setup_error(app_title: &str, error_string: &str) {
    let title = locale_strings::title_setup(app_title);
    let header = locale_strings::setup_error_header(app_title);
    show_error(&title, &header, error_string);
}

/// The runtime of an app started through its stub could not be prepared; `body` says why. True when
/// the user wants to retry.
pub fn show_runtime_failed(app_title: &str, body: &str) -> bool {
    if get_silent() {
        return false;
    }
    let header = locale_strings::runtime_failed_header(app_title);
    // The last button is the default one: trying again is what most people are here for.
    let buttons = vec![locale_strings::btn_cancel(), locale_strings::btn_retry()];
    matches!(show_message(app_title, &header, body, XDialogIcon::Error, buttons), Ok(XDialogResult::ButtonPressed(1)))
}

pub fn show_start_corrupt_error(app_title: &str) {
    let header = locale_strings::start_corrupt_header(app_title);
    let body = locale_strings::start_corrupt_body();
    show_error(app_title, &header, &body);
}

fn open_path(path: &Path) {
    let path_str = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(&path_str).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&path_str).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&path_str).spawn();
    }
}

#[test]
#[ntest::timeout(2000)]
fn test_no_dialogs_show_if_silent() {
    crate::init();
    set_silent(true);
    show_generic_error("TestApp", "This is an error.");
    show_restart_required("TestApp", "1.0.0");
    show_uninstall_complete("TestApp");
    show_setup_error("TestApp", "This is a setup error.");
}

#[test]
#[ignore]
fn test_show_all_dialogs() {
    crate::init();
    set_silent(false);
    show_generic_error("TestApp", "This is an error.");
    show_restart_required("TestApp", "1.0.0");
    show_uninstall_complete("TestApp");
    show_setup_error("TestApp", "This is a setup error.");
}

#[test]
fn test_dialog_timeout_getter_setter() {
    set_dialog_timeout(None);
    assert_eq!(get_dialog_timeout(), None);

    set_dialog_timeout(Some(Duration::from_secs(300)));
    assert_eq!(get_dialog_timeout(), Some(Duration::from_secs(300)));

    set_dialog_timeout(Some(Duration::from_millis(500)));
    assert_eq!(get_dialog_timeout(), Some(Duration::from_millis(500)));

    set_dialog_timeout(None);
    assert_eq!(get_dialog_timeout(), None);
}
