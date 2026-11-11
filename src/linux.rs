use super::{AppItem, RawApp};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    path::{Path, PathBuf},
    sync::LazyLock,
};

extern "C" {
    fn dh_linux_lookup(
        mime: *const c_char,
        context: *mut c_void,
        callback: extern "C" fn(
            *mut c_void,
            *const c_char,
            *const c_char,
            *const c_char,
            *const c_char,
            *const *const c_char,
            c_int,
        ),
    );
    fn dh_linux_set(mime: *const c_char, desktop_id: *const c_char) -> *mut c_char;
    fn dh_linux_extension_type(extension: *const c_char) -> *mut c_char;
    fn dh_linux_register_extension(extension: *const c_char, mime: *const c_char) -> *mut c_char;
    fn dh_linux_custom(
        path: *const c_char,
        context: *mut c_void,
        callback: extern "C" fn(
            *mut c_void,
            *const c_char,
            *const c_char,
            *const c_char,
            *const c_char,
            *const *const c_char,
            c_int,
        ),
    ) -> *mut c_char;
    fn dh_linux_set_custom(mime: *const c_char, path: *const c_char, scheme: c_int) -> *mut c_char;
    fn dh_linux_free(message: *mut c_char);
}

unsafe fn native_result(error: *mut c_char) -> Result<(), String> {
    if error.is_null() {
        return Ok(());
    }
    let message = CStr::from_ptr(error).to_string_lossy().into_owned();
    dh_linux_free(error);
    Err(message)
}

fn target_type(id: &str, kind: i32) -> Result<CString, String> {
    match kind {
        0 | 1 => content_type(id, kind == 1),
        2 => {
            let extension = CString::new(super::normalize_extension(id)?).unwrap();
            unsafe {
                let value = dh_linux_extension_type(extension.as_ptr());
                if value.is_null() {
                    return Err("无法识别扩展名。".into());
                }
                let result = CStr::from_ptr(value).to_owned();
                dh_linux_free(value);
                Ok(result)
            }
        }
        _ => Err("无效的类型选项。".into()),
    }
}

pub fn custom_app(path: &Path) -> Result<RawApp, String> {
    let path = path.to_str().ok_or("应用路径不是有效的 UTF-8 文本。")?;
    let path = CString::new(path).map_err(|_| "应用路径包含无效字符。")?;
    let mut result: (Option<RawApp>, Vec<RawApp>) = (None, Vec::new());
    unsafe {
        native_result(dh_linux_custom(
            path.as_ptr(),
            &mut result as *mut _ as *mut c_void,
            app_callback,
        ))?;
    }
    result.1.pop().ok_or("无法读取此应用。".into())
}

fn content_type(id: &str, scheme: bool) -> Result<CString, String> {
    let id = id.trim();
    let value = if scheme {
        if !id.starts_with(|c: char| c.is_ascii_alphabetic())
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"+.-".contains(&c))
        {
            return Err("请输入 URL 协议名称，例如 https 或 mailto（不含冒号或网址）。".into());
        }
        format!("x-scheme-handler/{}", id.to_ascii_lowercase())
    } else {
        let valid_part = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'+-.^_`|~".contains(&c))
        };
        if !id
            .split_once('/')
            .is_some_and(|(a, b)| valid_part(a) && valid_part(b))
        {
            return Err("请输入 MIME 类型，例如 application/pdf 或 text/plain。".into());
        }
        id.to_ascii_lowercase()
    };
    CString::new(value).map_err(|_| "输入包含无效字符。".into())
}

// hicolor is the mandatory application-icon fallback. Try the user's GTK icon
// theme as well; KDE applications usually install their app icons in hicolor.
static ICON_THEME: LazyLock<String> =
    LazyLock::new(|| freedesktop_icons::default_theme_gtk().unwrap_or_else(|| "hicolor".into()));

fn icon_path(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_owned());
    }
    freedesktop_icons::lookup(name)
        .with_size(64)
        .with_theme(&ICON_THEME)
        .with_cache()
        .find()
}

extern "C" fn app_callback(
    context: *mut c_void,
    id: *const c_char,
    name: *const c_char,
    path: *const c_char,
    source: *const c_char,
    icons: *const *const c_char,
    current: c_int,
) {
    // All strings and the icon array are borrowed for this synchronous callback.
    unsafe {
        let string = |p: *const c_char| {
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        };
        let mut icon = Vec::new();
        if !icons.is_null() {
            let mut entry = icons;
            while !(*entry).is_null() {
                if let Some(path) = icon_path(&string(*entry)) {
                    if std::fs::metadata(&path).is_ok_and(|m| m.len() <= 16 * 1024 * 1024) {
                        if let Ok(bytes) = std::fs::read(path) {
                            icon = bytes;
                            break;
                        }
                    }
                }
                entry = entry.add(1);
            }
        }
        let app = RawApp {
            name: string(name),
            identifier: string(id),
            path: string(path),
            source: string(source),
            icon,
        };
        let (default, apps) = &mut *(context as *mut (Option<RawApp>, Vec<RawApp>));
        if current != 0 {
            *default = Some(app);
        } else if !apps
            .iter()
            .any(|existing| existing.identifier == app.identifier)
        {
            apps.push(app);
        }
    }
}

pub fn lookup(id: &str, kind: i32) -> Result<(Option<RawApp>, Vec<RawApp>), String> {
    let mime = target_type(id, kind)?;
    let mut result: (Option<RawApp>, Vec<RawApp>) = (None, Vec::new());
    unsafe {
        dh_linux_lookup(
            mime.as_ptr(),
            &mut result as *mut _ as *mut c_void,
            app_callback,
        );
    }
    // Some desktops return a default separately from the recommended handlers.
    if let Some(current) = &result.0 {
        if !result
            .1
            .iter()
            .any(|app| app.identifier == current.identifier)
        {
            result.1.push(current.clone());
        }
    }
    result.1.sort_by_key(|app| app.name.to_lowercase());
    Ok(result)
}

fn set_default_sync(id: &str, kind: i32, desktop_id: &str) -> Result<(), String> {
    let mime = target_type(id, kind)?;
    let custom = Path::new(desktop_id).is_absolute();
    if !custom && (!desktop_id.ends_with(".desktop") || desktop_id.contains(['/', '\\'])) {
        return Err("无效的 desktop 应用标识，请重新查询。".into());
    }
    let desktop_id = CString::new(desktop_id).map_err(|_| "应用标识包含无效字符。")?;
    unsafe {
        if kind == 2 {
            let extension = CString::new(super::normalize_extension(id)?).unwrap();
            native_result(dh_linux_register_extension(
                extension.as_ptr(),
                mime.as_ptr(),
            ))?;
        }
        if custom {
            native_result(dh_linux_set_custom(
                mime.as_ptr(),
                desktop_id.as_ptr(),
                i32::from(kind == 1),
            ))
        } else {
            native_result(dh_linux_set(mime.as_ptr(), desktop_id.as_ptr()))
        }
    }
}

pub fn set_default(
    id: &str,
    kind: i32,
    app: &AppItem,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    let id = id.to_owned();
    let desktop_id = app.identifier.to_string();
    std::thread::spawn(move || done(set_default_sync(&id, kind, &desktop_id)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};

    #[test]
    fn validates_mime_types_and_url_schemes() {
        for category in crate::CATEGORIES {
            for preset in category.presets {
                assert!(content_type(preset.platform_identifier(), preset.kind == 1).is_ok());
            }
        }
        for mime in [
            "application/pdf",
            "text/plain",
            "image/svg+xml",
            "inode/directory",
        ] {
            assert_eq!(content_type(mime, false).unwrap().to_str().unwrap(), mime);
        }
        assert_eq!(
            content_type(" HTTPS ", true).unwrap().to_str().unwrap(),
            "x-scheme-handler/https"
        );
        assert_eq!(
            content_type("web+demo", true).unwrap().to_str().unwrap(),
            "x-scheme-handler/web+demo"
        );
        for invalid in [
            "",
            "public.png",
            "image/",
            "/png",
            "image/png/extra",
            "text/plain;charset=utf8",
            "image/*",
            "text/plain\0",
        ] {
            assert!(lookup(invalid, 0).is_err(), "accepted {invalid:?}");
        }
        for invalid in [
            "",
            "https:",
            "https://example.com",
            "1http",
            "mail to",
            "https\0",
        ] {
            assert!(lookup(invalid, 1).is_err(), "accepted {invalid:?}");
        }
    }

    // GIO caches XDG paths and the desktop at process startup. Each environment
    // therefore gets its own subprocess, with all writes confined to TempDir.
    #[test]
    fn desktop_backends_in_isolated_xdg_environments() {
        for desktop in ["KDE", "GNOME", "niri", "niri:GNOME", ""] {
            let root = tempfile::tempdir().unwrap();
            let root = root.path();
            for dir in [
                "config",
                "system-config",
                "data/applications/nested",
                "system-data/applications",
                "cache",
            ] {
                fs::create_dir_all(root.join(dir)).unwrap();
            }
            std::os::unix::fs::symlink("/usr/share/mime", root.join("system-data/mime")).unwrap();
            let entry = |name: &str| {
                format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec=/bin/true %u\nMimeType=application/pdf;x-scheme-handler/https;\nNoDisplay=true\n"
            )
            };
            let icon = root.join("test icon.svg");
            fs::write(&icon, "  <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"32\"><rect width=\"32\" height=\"32\" fill=\"red\"/></svg>").unwrap();
            fs::write(
                root.join("data/applications/first.desktop"),
                entry("First App"),
            )
            .unwrap();
            fs::write(
                root.join("data/applications/nested/second.desktop"),
                format!("{}Icon={}\n", entry("Second App"), icon.display()),
            )
            .unwrap();
            fs::write(
                root.join("system-data/applications/hidden.desktop"),
                entry("Hidden App"),
            )
            .unwrap();
            fs::write(
                root.join("data/applications/hidden.desktop"),
                format!("{}Hidden=true\n", entry("Hidden User Override")),
            )
            .unwrap();
            fs::write(root.join("data/applications/mimeinfo.cache"), "[MIME Cache]\napplication/pdf=first.desktop;nested-second.desktop;hidden.desktop;\nx-scheme-handler/https=first.desktop;nested-second.desktop;\n").unwrap();
            fs::write(
                root.join("system-data/applications/mimeinfo.cache"),
                "[MIME Cache]\napplication/pdf=hidden.desktop;\n",
            )
            .unwrap();
            fs::write(root.join("config/mimeapps.list"), "# Keep this comment\n[Default Applications]\napplication/pdf=first.desktop;\nx-scheme-handler/https=first.desktop;\ntext/plain=untouched.desktop;\n[Added Associations]\napplication/pdf=first.desktop;nested-second.desktop;\n").unwrap();
            for component in desktop.split(':').filter(|s| !s.is_empty()) {
                fs::write(root.join(format!("config/{}-mimeapps.list", component.to_ascii_lowercase())), "# Desktop comment\n[Default Applications]\napplication/pdf=first.desktop;\nx-scheme-handler/https=first.desktop;\ntext/plain=desktop-untouched.desktop;\n").unwrap();
            }
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "platform::tests::isolated_xdg_worker",
                    "--nocapture",
                ])
                .env("DEFAULT_HELPER_TEST_ROOT", root)
                .env("HOME", root)
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("XDG_CONFIG_DIRS", root.join("system-config"))
                .env("XDG_DATA_HOME", root.join("data"))
                .env("XDG_DATA_DIRS", root.join("system-data"))
                .env("XDG_CACHE_HOME", root.join("cache"))
                .env("XDG_CURRENT_DESKTOP", desktop)
                .env("GIO_USE_VFS", "local")
                .env("LC_ALL", "C")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{desktop}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn isolated_xdg_worker() {
        let Some(root) = std::env::var_os("DEFAULT_HELPER_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        if std::env::var_os("DEFAULT_HELPER_VERIFY_EXTENSION").is_some() {
            let (_, apps) = lookup(".DefaultHelperTest947", 2).unwrap();
            assert_eq!(apps.len(), 1);
            assert!(fs::read_to_string(&apps[0].path)
                .unwrap()
                .contains("X-DefaultHelper-Source="));
            return;
        }
        let (current, apps) = lookup("application/pdf", 0).unwrap();
        assert_eq!(current.unwrap().identifier, "first.desktop");
        let ids = apps
            .iter()
            .map(|app| app.identifier.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["first.desktop", "nested-second.desktop"]);
        assert_eq!(apps[1].name, "Second App");
        assert!(Path::new(&apps[1].path).is_absolute());
        assert!(apps[1].path.ends_with("nested/second.desktop"));
        assert_eq!(apps[1].icon, fs::read(root.join("test icon.svg")).unwrap());
        assert!(set_default_sync("application/pdf", 0, "missing.desktop").is_err());
        assert!(set_default_sync("application/pdf", 0, "../first.desktop").is_err());
        assert!(set_default_sync("application/pdf", 0, "bad\0.desktop").is_err());
        set_default_sync("application/pdf", 0, "nested-second.desktop").unwrap();
        assert_eq!(
            lookup("application/pdf", 0).unwrap().0.unwrap().identifier,
            "nested-second.desktop"
        );
        assert_eq!(
            lookup("https", 1).unwrap().0.unwrap().identifier,
            "first.desktop"
        );
        set_default_sync("HTTPS", 1, "nested-second.desktop").unwrap();
        assert_eq!(
            lookup("https", 1).unwrap().0.unwrap().identifier,
            "nested-second.desktop"
        );
        for component in std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap()
            .split(':')
            .filter(|s| !s.is_empty())
        {
            let contents = fs::read_to_string(root.join(format!(
                "config/{}-mimeapps.list",
                component.to_ascii_lowercase()
            )))
            .unwrap();
            assert!(contents.contains("# Desktop comment"));
            assert!(contents.contains("application/pdf=nested-second.desktop;"));
            assert!(contents.contains("x-scheme-handler/https=nested-second.desktop;"));
            assert!(contents.contains("text/plain=desktop-untouched.desktop;"));
        }
        let contents = fs::read_to_string(root.join("config/mimeapps.list")).unwrap();
        assert!(contents.contains("text/plain=untouched.desktop;"));
        assert!(lookup("application/x-default-helper-unregistered", 0)
            .unwrap()
            .1
            .is_empty());

        check_custom_applications_and_extensions(&root);

        // Invalid higher-priority user configuration must produce an error,
        // leaving that file untouched instead of silently replacing it.
        if let Some(component) = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap()
            .split(':')
            .find(|s| !s.is_empty())
        {
            let path = root.join(format!(
                "config/{}-mimeapps.list",
                component.to_ascii_lowercase()
            ));
            fs::write(&path, "invalid key file").unwrap();
            assert!(set_default_sync("application/pdf", 0, "first.desktop").is_err());
            assert_eq!(fs::read_to_string(path).unwrap(), "invalid key file");
        }
        let path = root.join("config/mimeapps.list");
        fs::write(&path, "invalid generic key file").unwrap();
        assert!(set_default_sync("application/pdf", 0, "first.desktop").is_err());
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "invalid generic key file"
        );
    }

    fn check_custom_applications_and_extensions(root: &Path) {
        assert_eq!(
            target_type(".PDF", 2).unwrap().to_str().unwrap(),
            "application/pdf"
        );
        assert!(lookup("*.bad", 2).is_err());
        assert!(lookup("foo", 99).is_err());
        let extension = ".DefaultHelperTest947";
        assert!(lookup(extension, 2).unwrap().0.is_none());
        assert!(
            !root.join("data/mime").exists(),
            "query must not register types"
        );

        let executable = root.join("My Editor $ `% \"'\\ 947");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$DEFAULT_HELPER_TEST_ROOT/arguments\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let app = custom_app(&executable).unwrap();
        assert_eq!(app.name, "My Editor $ `% \"'\\ 947");
        assert_eq!(app.identifier, executable.to_str().unwrap());
        assert!(!fs::read_dir(root.join("data/applications"))
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("userapp-")));
        set_default_sync(extension, 2, &app.identifier).unwrap();
        let current = lookup(extension, 2).unwrap().0.unwrap();
        assert!(fs::read_to_string(&current.path)
            .unwrap()
            .contains("X-DefaultHelper-Source="));
        let entry = fs::read_to_string(&current.path).unwrap();
        assert!(
            entry.contains("%%"),
            "literal percent must not become a field code"
        );
        assert!(entry.contains("%f"));
        let mut displayed = vec![current.clone()];
        super::super::merge_custom_apps(&mut displayed, std::slice::from_ref(&app));
        assert_eq!(
            displayed.len(),
            1,
            "registered custom app must not appear twice"
        );
        assert!(root.join("data/mime/mime.cache").is_file());
        assert!(
            !root.join("arguments").exists(),
            "adding and setting must not execute the app"
        );
        let verify = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::tests::isolated_xdg_worker",
                "--nocapture",
            ])
            .env("DEFAULT_HELPER_VERIFY_EXTENSION", "1")
            .output()
            .unwrap();
        assert!(
            verify.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&verify.stdout),
            String::from_utf8_lossy(&verify.stderr)
        );

        // Where the GIO CLI is installed, exercise the saved Exec entry with a
        // harmless fixture to verify exact argument handling, including %%.
        let document = root.join("File $ % 947.defaulthelpertest947");
        fs::write(&document, "fixture").unwrap();
        match Command::new("gio")
            .arg("launch")
            .arg(&current.path)
            .arg(&document)
            .output()
        {
            Ok(output) => {
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    fs::read_to_string(root.join("arguments")).unwrap(),
                    format!("{}\n", document.display())
                );
                let info = Command::new("gio")
                    .args(["info", "-a", "standard::content-type"])
                    .arg(&document)
                    .output()
                    .unwrap();
                assert!(info.status.success());
                let expected = target_type(extension, 2).unwrap();
                assert!(
                    String::from_utf8_lossy(&info.stdout).contains(expected.to_str().unwrap()),
                    "the registered extension must be recognized by a fresh GIO process"
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("could not run GIO fixture: {error}"),
        }

        // Reapplying reuses the same registered desktop entry.
        set_default_sync(extension, 2, &app.identifier).unwrap();
        assert_eq!(
            lookup(extension, 2).unwrap().0.unwrap().identifier,
            current.identifier
        );
        set_default_sync("custom-helper-test947", 1, &app.identifier).unwrap();
        let uri_app = lookup("custom-helper-test947", 1).unwrap().0.unwrap();
        assert!(fs::read_to_string(&uri_app.path).unwrap().contains("%u"));
        assert_ne!(uri_app.identifier, current.identifier);

        let source = root.join("External App.desktop");
        let desktop = "[Desktop Entry]\nType=Application\nName=External App\nExec=/bin/true --view %u\nTerminal=false\n";
        fs::write(&source, desktop).unwrap();
        let app = custom_app(&source).unwrap();
        set_default_sync("application/x-helper-custom947", 0, &app.identifier).unwrap();
        let copied = lookup("application/x-helper-custom947", 0)
            .unwrap()
            .0
            .unwrap();
        assert!(fs::read_to_string(copied.path)
            .unwrap()
            .contains("Exec=/bin/true --view %u"));
        assert_eq!(fs::read_to_string(&source).unwrap(), desktop);

        let registered = root.join("data/applications/nested/second.desktop");
        let app = custom_app(&registered).unwrap();
        set_default_sync("application/x-helper-installed947", 0, &app.identifier).unwrap();
        assert_eq!(
            lookup("application/x-helper-installed947", 0)
                .unwrap()
                .0
                .unwrap()
                .identifier,
            "nested-second.desktop"
        );

        assert!(custom_app(&root.join("missing.desktop")).is_err());
        assert!(custom_app(root).is_err());
        let invalid = root.join("not executable");
        fs::write(&invalid, "plain text").unwrap();
        assert!(custom_app(&invalid).is_err());
        fs::remove_file(&executable).unwrap();
        assert!(set_default_sync(extension, 2, &executable.to_string_lossy()).is_err());
    }
}
