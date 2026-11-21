use crate::types::{MSG_CLOSE, MSG_INDEFINITE};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// Set when the user presses Cancel on a progress dialog. One Setup or Update process does one
/// operation, so one flag is enough.
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
/// Whether pressing Cancel still does anything; cleared once an operation can no longer be undone.
static CANCEL_ALLOWED: AtomicBool = AtomicBool::new(true);
/// The text a [`ChannelProgressReporter`] was asked to show: its channel only carries numbers, so
/// whoever shows the dialog picks the text up here.
static CHANNEL_TEXT: Mutex<Option<String>> = Mutex::new(None);

/// Whether the user asked to cancel the running operation.
pub fn cancel_requested() -> bool {
    CANCEL_REQUESTED.load(Ordering::SeqCst)
}

/// From here on the Cancel button of a progress dialog does nothing.
pub fn disallow_cancel() {
    CANCEL_ALLOWED.store(false, Ordering::SeqCst);
}

/// The text last given to a [`ChannelProgressReporter`], once.
pub fn take_channel_text() -> Option<String> {
    CHANNEL_TEXT.lock().ok()?.take()
}

pub trait ProgressReporter: Send {
    fn set_progress(&self, value: i16);
    fn set_indeterminate(&self);
    fn set_text(&self, text: &str);
    fn close(&self);
}

pub struct ChannelProgressReporter {
    tx: Sender<i16>,
}

impl ChannelProgressReporter {
    pub fn new(tx: Sender<i16>) -> Self {
        Self { tx }
    }
}

impl ProgressReporter for ChannelProgressReporter {
    fn set_progress(&self, value: i16) {
        let _ = self.tx.send(value);
    }

    fn set_indeterminate(&self) {
        let _ = self.tx.send(MSG_INDEFINITE);
    }

    fn set_text(&self, text: &str) {
        if let Ok(mut slot) = CHANNEL_TEXT.lock() {
            *slot = Some(text.to_string());
        }
    }

    fn close(&self) {
        let _ = self.tx.send(MSG_CLOSE);
    }
}

struct XDialogProgressReporter {
    proxy: xdialog::ProgressDialogProxy,
}

impl XDialogProgressReporter {
    fn new(proxy: xdialog::ProgressDialogProxy) -> Self {
        Self { proxy }
    }
}

impl ProgressReporter for XDialogProgressReporter {
    fn set_progress(&self, value: i16) {
        let _ = self.proxy.set_value(value as f32 / 100.0);
    }

    fn set_indeterminate(&self) {
        let _ = self.proxy.set_indeterminate();
    }

    fn set_text(&self, text: &str) {
        let _ = self.proxy.set_text(text);
    }

    fn close(&self) {
        let _ = self.proxy.close();
    }
}

pub struct NoopProgressReporter;

impl ProgressReporter for NoopProgressReporter {
    fn set_progress(&self, _value: i16) {}
    fn set_indeterminate(&self) {}
    fn set_text(&self, _text: &str) {}
    fn close(&self) {}
}

fn show_progress_dialog(title: &str, header: &str, body: &str) -> Box<dyn ProgressReporter> {
    show_progress_dialog_ex(title, header, body, false)
}

/// A progress dialog; with `cancellable` it has a Cancel button that sets [`cancel_requested`].
fn show_progress_dialog_ex(title: &str, header: &str, body: &str, cancellable: bool) -> Box<dyn ProgressReporter> {
    if crate::dialogs::get_silent() {
        return Box::new(NoopProgressReporter);
    }
    let icon = crate::dialogs::general_icon(xdialog::XDialogIcon::Information);
    let buttons = if cancellable { vec![crate::locale_strings::btn_cancel()] } else { vec![] };
    let options = crate::dialogs::dialog_options(title, header, body, icon, buttons);
    let shown = if cancellable {
        let cancelling = crate::locale_strings::progress_cancelling();
        xdialog::show_progress_with_callback(options, move |_, proxy| {
            if CANCEL_ALLOWED.load(Ordering::SeqCst) {
                CANCEL_REQUESTED.store(true, Ordering::SeqCst);
                let _ = proxy.set_text(&cancelling);
            }
            // The dialog stays until the operation has wound down and closes it.
            true
        })
    } else {
        xdialog::show_progress_ex(options)
    };
    match shown {
        Ok(proxy) => Box::new(XDialogProgressReporter::new(proxy)),
        Err(e) => {
            warn!("Failed to show progress dialog: {:?}", e);
            Box::new(NoopProgressReporter)
        }
    }
}

pub fn show_apply_progress(app_name: &str, version: &str) -> Box<dyn ProgressReporter> {
    let title = crate::locale_strings::title_update(app_name);
    let header = crate::locale_strings::apply_header();
    let body = crate::locale_strings::apply_body(version);
    show_progress_dialog_ex(&title, &header, &body, true)
}

/// Shown when an app started through its stub has to get its runtime first.
pub fn show_runtime_progress(app_name: &str) -> Box<dyn ProgressReporter> {
    let header = crate::locale_strings::runtime_prepare_header(app_name);
    let body = crate::locale_strings::runtime_finishing();
    show_progress_dialog_ex(app_name, &header, &body, true)
}

pub fn show_splash_progress(app_name: &str, app_version: &str) -> Box<dyn ProgressReporter> {
    let title = crate::locale_strings::title_setup(app_name);
    let header = crate::locale_strings::splash_header(app_name);
    let body = crate::locale_strings::splash_body(app_name, app_version);
    show_progress_dialog_ex(&title, &header, &body, true)
}

pub fn show_uninstall_progress(app_name: &str) -> Box<dyn ProgressReporter> {
    let title = crate::locale_strings::title_uninstall(app_name);
    let header = crate::locale_strings::uninstall_progress_header(app_name);
    let body = crate::locale_strings::uninstall_progress_body();
    let reporter = show_progress_dialog(&title, &header, &body);
    // there is no meaningful progress to report while removing files and running hooks
    reporter.set_indeterminate();
    reporter
}

pub fn show_deps_download_progress(dep_name: &str, is_update: bool) -> Box<dyn ProgressReporter> {
    let title = if is_update {
        crate::locale_strings::title_update(dep_name)
    } else {
        crate::locale_strings::title_setup(dep_name)
    };
    let header = crate::locale_strings::deps_download_header();
    let body = crate::locale_strings::deps_download_body(dep_name);
    show_progress_dialog(&title, &header, &body)
}
