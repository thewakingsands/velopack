//! Gives the main executable of a downloaded runtime the application's icon and version information.
//!
//! vpk has already written both into the launcher stub. They are copied from there as they are, so
//! nothing here has to build an icon directory or a version block: the resources of these three types
//! are removed from the target, and the stub's icon and its version block for the main executable are
//! written in their place. Everything else in the
//! target, its manifest included, stays.

use anyhow::{bail, Context, Result};
use std::{ffi::c_void, path::Path};
use velopack::wide_strings::string_to_wide;
use windows::{
    core::{BOOL, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{
            BeginUpdateResourceW, EndUpdateResourceW, EnumResourceLanguagesW, EnumResourceNamesW, FindResourceExW, LoadLibraryExW, LoadResource,
            LockResource, SizeofResource, UpdateResourceW, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        },
        UI::WindowsAndMessaging::{RT_GROUP_ICON, RT_ICON, RT_VERSION},
    },
};

/// The name of the version resource in the launcher stub that is meant for the main executable.
const MAIN_VERSION_NAME: u16 = 2;

/// A resource type or name: a number, or a string.
#[derive(Clone, PartialEq)]
enum Ident {
    Id(u16),
    Text(Vec<u16>),
}

impl Ident {
    unsafe fn read(value: PCWSTR) -> Self {
        // MAKEINTRESOURCE: a pointer whose upper bits are zero carries a number.
        if value.0 as usize >> 16 == 0 {
            Ident::Id(value.0 as usize as u16)
        } else {
            let mut text = value.as_wide().to_vec();
            text.push(0);
            Ident::Text(text)
        }
    }

    fn as_pcwstr(&self) -> PCWSTR {
        match self {
            Ident::Id(id) => PCWSTR(*id as usize as *const u16),
            Ident::Text(text) => PCWSTR(text.as_ptr()),
        }
    }
}

struct Resource {
    kind: Ident,
    name: Ident,
    language: u16,
    data: Vec<u8>,
}

impl Resource {
    fn same_place(&self, other: &Resource) -> bool {
        self.kind == other.kind && self.name == other.name && self.language == other.language
    }
}

unsafe extern "system" fn collect_name(_: HMODULE, _: PCWSTR, name: PCWSTR, names: isize) -> BOOL {
    (*(names as *mut Vec<Ident>)).push(Ident::read(name));
    true.into()
}

unsafe extern "system" fn collect_language(_: HMODULE, _: PCWSTR, _: PCWSTR, language: u16, languages: isize) -> BOOL {
    (*(languages as *mut Vec<u16>)).push(language);
    true.into()
}

/// Reads the icon and version resources of an executable, without running anything in it.
fn read_resources(file: &Path) -> Result<Vec<Resource>> {
    let wide = string_to_wide(file);
    unsafe {
        let module = LoadLibraryExW(wide.as_pcwstr(), None, LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE).with_context(|| {
                                                                                                                          format!("Unable to open \
                                                                                                                                   {:?} to read its \
                                                                                                                                   resources",
                                                                                                                                  file)
                                                                                                                      })?;
        let result = (|| -> Result<Vec<Resource>> {
            let mut resources = Vec::new();
            for kind in [RT_GROUP_ICON, RT_ICON, RT_VERSION] {
                let mut names: Vec<Ident> = Vec::new();
                // Fails when the file has no resource of this type, which leaves the list empty.
                let _ = EnumResourceNamesW(Some(module), kind, Some(collect_name), &mut names as *mut _ as isize);
                for name in names {
                    let mut languages: Vec<u16> = Vec::new();
                    let _ = EnumResourceLanguagesW(Some(module),
                                                   kind,
                                                   name.as_pcwstr(),
                                                   Some(collect_language),
                                                   &mut languages as *mut _ as isize);
                    for language in languages {
                        let found = FindResourceExW(Some(module), kind, name.as_pcwstr(), language);
                        if found.is_invalid() {
                            bail!("Unable to find a resource of {:?} that was just listed", file);
                        }
                        let size = SizeofResource(Some(module), found) as usize;
                        let pointer = LockResource(LoadResource(Some(module), found)?) as *const u8;
                        if pointer.is_null() {
                            bail!("Unable to read a resource of {:?}", file);
                        }
                        resources.push(Resource { kind: Ident::read(kind),
                                                  name: name.clone(),
                                                  language,
                                                  data: std::slice::from_raw_parts(pointer, size).to_vec() });
                    }
                }
            }
            Ok(resources)
        })();
        let _ = FreeLibrary(module);
        result
    }
}

/// Replaces the icon and version information of `target` with those of `donor`.
///
/// A file that has just been written is often opened by something else for a moment (a virus scanner,
/// the shell reading its icon), and the resource update needs the file to itself: a failed attempt is
/// repeated for a while before it counts.
pub fn copy_icon_and_version(donor: &Path, target: &Path) -> Result<()> {
    // The stub describes itself as the launcher in its own version resource (name 1) and carries the
    // main executable's under name 2, which Windows ignores there.
    let version = Ident::Id(RT_VERSION.0 as usize as u16);
    let mut wanted = read_resources(donor)?;
    wanted.retain(|resource| resource.kind != version || resource.name == Ident::Id(MAIN_VERSION_NAME));
    for resource in wanted.iter_mut().filter(|resource| resource.kind == version) {
        resource.name = Ident::Id(1);
    }
    if !wanted.iter().any(|resource| resource.kind == Ident::Id(RT_GROUP_ICON.0 as usize as u16)) {
        bail!("{:?} has no icon to give to {:?}", donor, target);
    }
    const ATTEMPTS: u32 = 40;
    let mut attempt = 1;
    loop {
        match write_resources(&wanted, target) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < ATTEMPTS => {
                warn!("Writing the resources of {:?} failed, attempt {} of {}: {:#}",
                      target, attempt, ATTEMPTS, e);
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(e) => return Err(e),
        }
    }
}

fn write_resources(wanted: &[Resource], target: &Path) -> Result<()> {
    let present = read_resources(target)?;
    info!("Writing {} icon and version resources into {:?} (replacing {}).",
          wanted.len(),
          target,
          present.len());

    let wide = string_to_wide(target);
    unsafe {
        let update = BeginUpdateResourceW(wide.as_pcwstr(), false).with_context(|| format!("Unable to open {:?} to change its resources", target))?;
        let result = (|| -> Result<()> {
            for old in present.iter().filter(|old| !wanted.iter().any(|new| new.same_place(old))) {
                UpdateResourceW(update, old.kind.as_pcwstr(), old.name.as_pcwstr(), old.language, None, 0)?;
            }
            for new in wanted {
                let data = Some(new.data.as_ptr() as *const c_void);
                UpdateResourceW(update,
                                new.kind.as_pcwstr(),
                                new.name.as_pcwstr(),
                                new.language,
                                data,
                                new.data.len() as u32)?;
            }
            Ok(())
        })();
        // Discards every change when one of them failed, so the file is never left half changed.
        EndUpdateResourceW(update, result.is_err()).with_context(|| format!("Unable to write the resources of {:?}", target))?;
        result
    }
}
