use slint::{ComponentHandle, Model, SharedString, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

slint::include_modules!();

struct Preset {
    label: &'static str,
    identifier: &'static str,
    kind: i32,
    #[cfg(target_os = "linux")]
    mime: &'static str,
}

impl Preset {
    fn platform_identifier(&self) -> &'static str {
        #[cfg(target_os = "linux")]
        {
            if self.kind == 1 {
                self.identifier
            } else {
                self.mime
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.identifier
        }
    }
}

struct Category {
    label: &'static str,
    presets: &'static [Preset],
}

const CATEGORIES: &[Category] = &[
    Category {
        label: "文档与文本",
        presets: &[
            Preset {
                label: "PDF 文档 (.pdf)",
                identifier: "com.adobe.pdf",
                #[cfg(target_os = "linux")]
                mime: "application/pdf",
                kind: 0,
            },
            Preset {
                label: "纯文本 (.txt)",
                identifier: "public.plain-text",
                #[cfg(target_os = "linux")]
                mime: "text/plain",
                kind: 0,
            },
            Preset {
                label: "富文本 (.rtf)",
                identifier: "public.rtf",
                #[cfg(target_os = "linux")]
                mime: "application/rtf",
                kind: 0,
            },
            Preset {
                label: "HTML 文档 (.html)",
                identifier: "public.html",
                #[cfg(target_os = "linux")]
                mime: "text/html",
                kind: 0,
            },
        ],
    },
    Category {
        label: "图片",
        presets: &[
            Preset {
                label: "JPEG 图片 (.jpg)",
                identifier: "public.jpeg",
                #[cfg(target_os = "linux")]
                mime: "image/jpeg",
                kind: 0,
            },
            Preset {
                label: "PNG 图片 (.png)",
                identifier: "public.png",
                #[cfg(target_os = "linux")]
                mime: "image/png",
                kind: 0,
            },
            Preset {
                label: "GIF 图片 (.gif)",
                identifier: "com.compuserve.gif",
                #[cfg(target_os = "linux")]
                mime: "image/gif",
                kind: 0,
            },
            Preset {
                label: "TIFF 图片 (.tiff)",
                identifier: "public.tiff",
                #[cfg(target_os = "linux")]
                mime: "image/tiff",
                kind: 0,
            },
            Preset {
                label: "HEIC 图片 (.heic)",
                identifier: "public.heic",
                #[cfg(target_os = "linux")]
                mime: "image/heic",
                kind: 0,
            },
        ],
    },
    Category {
        label: "音频",
        presets: &[
            Preset {
                label: "MP3 音频 (.mp3)",
                identifier: "public.mp3",
                #[cfg(target_os = "linux")]
                mime: "audio/mpeg",
                kind: 0,
            },
            Preset {
                label: "MPEG-4 音频 (.m4a)",
                identifier: "public.mpeg-4-audio",
                #[cfg(target_os = "linux")]
                mime: "audio/mp4",
                kind: 0,
            },
            Preset {
                label: "WAV 音频 (.wav)",
                identifier: "com.microsoft.waveform-audio",
                #[cfg(target_os = "linux")]
                mime: "audio/x-wav",
                kind: 0,
            },
            Preset {
                label: "AIFF 音频 (.aiff)",
                identifier: "public.aiff-audio",
                #[cfg(target_os = "linux")]
                mime: "audio/x-aiff",
                kind: 0,
            },
        ],
    },
    Category {
        label: "视频",
        presets: &[
            Preset {
                label: "MP4 视频 (.mp4)",
                identifier: "public.mpeg-4",
                #[cfg(target_os = "linux")]
                mime: "video/mp4",
                kind: 0,
            },
            Preset {
                label: "QuickTime 视频 (.mov)",
                identifier: "com.apple.quicktime-movie",
                #[cfg(target_os = "linux")]
                mime: "video/quicktime",
                kind: 0,
            },
            Preset {
                label: "AVI 视频 (.avi)",
                identifier: "public.avi",
                #[cfg(target_os = "linux")]
                mime: "video/x-msvideo",
                kind: 0,
            },
        ],
    },
    Category {
        label: "压缩文件",
        presets: &[
            Preset {
                label: "ZIP 压缩文件 (.zip)",
                identifier: "public.zip-archive",
                #[cfg(target_os = "linux")]
                mime: "application/zip",
                kind: 0,
            },
            Preset {
                label: "Gzip 压缩文件 (.gz)",
                identifier: "org.gnu.gnu-zip-archive",
                #[cfg(target_os = "linux")]
                mime: "application/gzip",
                kind: 0,
            },
            Preset {
                label: "Tar 归档文件 (.tar)",
                identifier: "public.tar-archive",
                #[cfg(target_os = "linux")]
                mime: "application/x-tar",
                kind: 0,
            },
        ],
    },
    #[cfg(target_os = "linux")]
    Category {
        label: "文件管理器",
        presets: &[Preset {
            label: "打开文件夹（默认文件管理器）",
            identifier: "public.folder",
            mime: "inode/directory",
            kind: 0,
        }],
    },
    Category {
        label: "链接与协议",
        presets: &[
            Preset {
                label: "网页链接 (HTTP)",
                identifier: "http",
                #[cfg(target_os = "linux")]
                mime: "http",
                kind: 1,
            },
            Preset {
                label: "安全网页链接 (HTTPS)",
                identifier: "https",
                #[cfg(target_os = "linux")]
                mime: "https",
                kind: 1,
            },
            Preset {
                label: "邮件链接 (mailto)",
                identifier: "mailto",
                #[cfg(target_os = "linux")]
                mime: "mailto",
                kind: 1,
            },
        ],
    },
];

fn preset_at(category: i32, index: i32) -> Option<&'static Preset> {
    CATEGORIES
        .get(usize::try_from(category).ok()?)?
        .presets
        .get(usize::try_from(index).ok()?)
}

fn select_preset(
    ui: &AppWindow,
    preset: &Preset,
    queried: &RefCell<(String, i32)>,
    generation: &Arc<AtomicU64>,
    custom_apps: &Arc<Mutex<Vec<RawApp>>>,
) {
    let identifier = preset.platform_identifier();
    ui.set_identifier(identifier.into());
    ui.set_kind_index(preset.kind);
    *queried.borrow_mut() = (identifier.to_owned(), preset.kind);
    refresh(ui, identifier, preset.kind, generation, None, custom_apps);
}

#[derive(Clone, Default)]
struct RawApp {
    name: String,
    identifier: String,
    path: String,
    source: String,
    icon: Vec<u8>,
}

impl From<RawApp> for AppItem {
    fn from(app: RawApp) -> Self {
        Self {
            name: app.name.into(),
            identifier: app.identifier.into(),
            path: app.path.into(),
            icon: slint::Image::load_from_data(&app.icon, None)
                .or_else(|_| slint::Image::load_from_data(&app.icon, Some("svg")))
                .unwrap_or_default(),
        }
    }
}

fn normalize_extension(value: &str) -> Result<String, String> {
    let extension = value.trim().strip_prefix('.').unwrap_or(value.trim());
    if extension.is_empty()
        || extension.len() > 128
        || extension.split('.').any(str::is_empty)
        || !extension
            .chars()
            .all(|c| c.is_alphanumeric() || "+-_.".contains(c))
    {
        return Err("请输入扩展名，例如 .foo 或 tar.gz，不要包含路径、空格或通配符。".into());
    }
    Ok(extension.to_lowercase())
}

fn custom_application_path(value: &str) -> Result<std::path::PathBuf, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("请选择应用或输入完整路径。".into());
    }
    let path = if let Some(relative) = value.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .ok_or("无法找到用户主目录。")?
            .join(relative)
    } else {
        std::path::PathBuf::from(value)
    };
    if !path.is_absolute() {
        return Err("请提供应用的完整路径。".into());
    }
    std::fs::canonicalize(path).map_err(|e| format!("无法访问应用：{e}"))
}

fn merge_custom_apps(apps: &mut Vec<RawApp>, custom: &[RawApp]) {
    for app in custom {
        if !apps.iter().any(|existing| {
            existing.identifier == app.identifier
                || (!app.path.is_empty()
                    && (existing.path == app.path || existing.source == app.path))
        }) {
            apps.push(app.clone());
        }
    }
}

fn connect_custom_app_callbacks(ui: &AppWindow, custom_apps: Arc<Mutex<Vec<RawApp>>>) {
    let weak = ui.as_weak();
    ui.on_add_application(move |path| {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_busy() || ui.get_adding_application() {
            return;
        }
        ui.set_adding_application(true);
        ui.set_status("正在读取自定义应用…".into());
        let path = path.to_string();
        let weak = ui.as_weak();
        let custom_apps = custom_apps.clone();
        std::thread::spawn(move || {
            let result =
                custom_application_path(&path).and_then(|path| platform::custom_app(&path));
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else { return };
                ui.set_adding_application(false);
                match result {
                    Ok(app) => {
                        ui.set_application_path(app.path.clone().into());
                        merge_custom_apps(&mut custom_apps.lock().unwrap(), &[app]);
                        ui.invoke_lookup(ui.get_identifier(), ui.get_kind_index());
                    }
                    Err(error) => ui.set_status(format!("添加失败：{error}").into()),
                }
            });
        });
    });
    let weak = ui.as_weak();
    ui.on_browse_application(move || {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_busy() || ui.get_adding_application() {
            return;
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            ui.set_adding_application(true);
            // macOS panels run on the UI thread. Linux portal dialogs run in a
            // worker, keeping the main window responsive while the portal replies.
            #[cfg(target_os = "macos")]
            {
                let selected = rfd::FileDialog::new()
                    .set_title("选择应用")
                    .add_filter("应用", &["app"])
                    .pick_file();
                ui.set_adding_application(false);
                if let Some(path) = selected {
                    ui.set_application_path(path.to_string_lossy().into_owned().into());
                    ui.invoke_add_application(ui.get_application_path());
                }
            }
            #[cfg(target_os = "linux")]
            {
                let weak = ui.as_weak();
                std::thread::spawn(move || {
                    let selected = rfd::FileDialog::new()
                        .set_title("选择 .desktop 文件或可执行文件")
                        .pick_file();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_adding_application(false);
                            if let Some(path) = selected {
                                ui.set_application_path(path.to_string_lossy().into_owned().into());
                                ui.invoke_add_application(ui.get_application_path());
                            }
                        }
                    });
                });
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        ui.set_status("此平台尚未支持自定义应用。".into());
    });
}

fn refresh(
    ui: &AppWindow,
    identifier: &str,
    kind: i32,
    generation: &Arc<AtomicU64>,
    message: Option<String>,
    custom_apps: &Arc<Mutex<Vec<RawApp>>>,
) {
    let request = generation.fetch_add(1, Ordering::Relaxed) + 1;
    let id = identifier.trim().to_owned();
    ui.set_applications(Default::default());
    ui.set_current_name("未找到默认应用".into());
    ui.set_current_id("".into());
    ui.set_current_icon(Default::default());
    if id.is_empty() {
        ui.set_loading(false);
        ui.set_status(format!("请输入{}或 URL 协议。", file_type_label()).into());
        return;
    }
    ui.set_loading(true);
    ui.set_status("正在查询可用应用…".into());
    let custom_apps = custom_apps.lock().unwrap().clone();
    let weak = ui.as_weak();
    let generation = generation.clone();
    std::thread::spawn(move || {
        let result = platform::lookup(&id, kind);
        let _ = slint::invoke_from_event_loop(move || {
            if generation.load(Ordering::Relaxed) != request {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.set_loading(false);
                let (current, mut apps) = match result {
                    Ok(result) => result,
                    Err(error) => {
                        ui.set_status(format!("查询失败：{error}").into());
                        return;
                    }
                };
                if let Some(app) = current {
                    let app = AppItem::from(app);
                    ui.set_current_name(app.name);
                    ui.set_current_id(app.identifier);
                    ui.set_current_icon(app.icon);
                }
                merge_custom_apps(&mut apps, &custom_apps);
                let count = apps.len();
                ui.set_applications(
                    Rc::new(VecModel::from(
                        apps.into_iter().map(AppItem::from).collect::<Vec<_>>(),
                    ))
                    .into(),
                );
                ui.set_status(
                    message
                        .unwrap_or_else(|| {
                            if count == 0 {
                                "没有找到可处理此类型的应用。".into()
                            } else {
                                "选择一个应用以更改默认打开方式。".into()
                            }
                        })
                        .into(),
                );
            }
        });
    });
}

fn file_type_label() -> &'static str {
    if cfg!(target_os = "linux") {
        "MIME 类型"
    } else {
        "UTI"
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.set_file_type_label(
        if cfg!(target_os = "linux") {
            "文件类型 (MIME)"
        } else {
            "文件类型 (UTI)"
        }
        .into(),
    );
    ui.set_identifier_hint(format!("输入{}或 URL 协议", file_type_label()).into());
    ui.set_platform_label(if cfg!(target_os = "linux") {
        "Linux  ·  默认应用".into()
    } else if cfg!(target_os = "macos") {
        "macOS  ·  默认应用".into()
    } else {
        "默认应用".into()
    });
    ui.set_categories(
        Rc::new(VecModel::from(
            CATEGORIES
                .iter()
                .map(|category| SharedString::from(category.label))
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    ui.set_presets(
        Rc::new(VecModel::from(
            CATEGORIES[0]
                .presets
                .iter()
                .map(|preset| SharedString::from(preset.label))
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    let custom_apps = Arc::new(Mutex::new(Vec::<RawApp>::new()));
    let generation = Arc::new(AtomicU64::new(0));
    let queried = Rc::new(RefCell::new((String::new(), 0)));
    let added_apps = custom_apps.clone();
    let state = queried.clone();
    let requests = generation.clone();
    let weak = ui.as_weak();
    ui.on_category_selected(move |index| {
        if let (Some(ui), Some(category)) = (
            weak.upgrade(),
            usize::try_from(index).ok().and_then(|i| CATEGORIES.get(i)),
        ) {
            ui.set_presets(
                Rc::new(VecModel::from(
                    category
                        .presets
                        .iter()
                        .map(|preset| SharedString::from(preset.label))
                        .collect::<Vec<_>>(),
                ))
                .into(),
            );
            ui.set_preset_index(0);
            if let Some(preset) = category.presets.first() {
                select_preset(&ui, preset, &state, &requests, &added_apps);
            }
        }
    });
    let added_apps = custom_apps.clone();
    let state = queried.clone();
    let requests = generation.clone();
    let weak = ui.as_weak();
    ui.on_preset_selected(move |index| {
        if let Some(ui) = weak.upgrade() {
            if let Some(preset) = preset_at(ui.get_category_index(), index) {
                select_preset(&ui, preset, &state, &requests, &added_apps);
            }
        }
    });
    let added_apps = custom_apps.clone();
    let state = queried.clone();
    let requests = generation.clone();
    let weak = ui.as_weak();
    ui.on_lookup(move |id, kind| {
        if let Some(ui) = weak.upgrade() {
            *state.borrow_mut() = (id.trim().to_string(), kind);
            refresh(&ui, &id, kind, &requests, None, &added_apps);
        }
    });
    let added_apps = custom_apps.clone();
    let state = queried.clone();
    let requests = generation.clone();
    let weak = ui.as_weak();
    ui.on_apply(move |id, kind, index| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_busy() || ui.get_loading() {
                return;
            }
            let selected = ui.get_applications().row_data(index as usize);
            match selected {
                Some(app)
                    if *state.borrow() == (id.trim().to_string(), kind)
                        && !id.trim().is_empty() =>
                {
                    ui.set_busy(true);
                    ui.set_status("正在等待系统完成默认应用设置…".into());
                    let weak = ui.as_weak();
                    let target = id.trim().to_string();
                    let request_target = target.clone();
                    let requests = requests.clone();
                    let added_apps = added_apps.clone();
                    platform::set_default(&request_target, kind, &app, move |result| {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = weak.upgrade() {
                                ui.set_busy(false);
                                let message = match result {
                                    Ok(()) => "系统已完成默认应用设置。".to_string(),
                                    Err(e) => format!("设置失败：{e}"),
                                };
                                if ui.get_identifier().trim() == target
                                    && ui.get_kind_index() == kind
                                {
                                    refresh(
                                        &ui,
                                        &target,
                                        kind,
                                        &requests,
                                        Some(message),
                                        &added_apps,
                                    );
                                } else {
                                    ui.set_status(message.into());
                                }
                            }
                        });
                    });
                }
                _ => {
                    *state.borrow_mut() = (id.trim().to_string(), kind);
                    refresh(&ui, &id, kind, &requests, None, &added_apps);
                    ui.set_status("类型或候选应用已变化，请重新选择。".into());
                }
            }
        }
    });
    select_preset(
        &ui,
        &CATEGORIES[0].presets[0],
        &queried,
        &generation,
        &custom_apps,
    );
    connect_custom_app_callbacks(&ui, custom_apps);
    ui.run()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{AppItem, RawApp};
    use std::ffi::{c_char, c_void, CStr, CString};
    type Done = Box<dyn FnOnce(Result<(), String>) + Send>;
    extern "C" {
        fn dh_lookup(
            id: *const c_char,
            kind: i32,
            ctx: *mut c_void,
            callback: extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                *const c_char,
                *const u8,
                usize,
                bool,
            ),
        );
        fn dh_set(
            id: *const c_char,
            kind: i32,
            path: *const c_char,
            bundle: *const c_char,
            ctx: *mut c_void,
            callback: extern "C" fn(*mut c_void, *const c_char),
        );
        fn dh_custom(
            path: *const c_char,
            ctx: *mut c_void,
            callback: extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                *const c_char,
                *const u8,
                usize,
                bool,
            ),
        ) -> *const c_char;
    }
    extern "C" fn app_callback(
        ctx: *mut c_void,
        id: *const c_char,
        name: *const c_char,
        path: *const c_char,
        png: *const u8,
        len: usize,
        current: bool,
    ) {
        unsafe {
            let (default, apps) = &mut *(ctx as *mut (Option<RawApp>, Vec<RawApp>));
            let string = |s| CStr::from_ptr(s).to_string_lossy().into_owned();
            let icon = if png.is_null() || len == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(png, len).to_vec()
            };
            let app = RawApp {
                name: string(name),
                identifier: string(id),
                path: string(path),
                source: String::new(),
                icon,
            };
            if current {
                *default = Some(app);
            } else {
                apps.push(app);
            }
        }
    }
    pub fn lookup(id: &str, kind: i32) -> Result<(Option<RawApp>, Vec<RawApp>), String> {
        let normalized = if kind == 2 {
            super::normalize_extension(id)?
        } else {
            id.trim().to_owned()
        };
        let Ok(id) = CString::new(normalized) else {
            return Err("输入包含无效字符。".into());
        };
        let mut result = (None, Vec::new());
        unsafe {
            dh_lookup(
                id.as_ptr(),
                kind,
                &mut result as *mut _ as *mut c_void,
                app_callback,
            );
        }
        Ok(result)
    }
    pub fn custom_app(path: &std::path::Path) -> Result<RawApp, String> {
        let path = CString::new(path.to_str().ok_or("应用路径不是有效的 UTF-8 文本。")?)
            .map_err(|_| "应用路径包含无效字符。")?;
        let mut result: (Option<RawApp>, Vec<RawApp>) = (None, Vec::new());
        unsafe {
            let error = dh_custom(
                path.as_ptr(),
                &mut result as *mut _ as *mut c_void,
                app_callback,
            );
            if !error.is_null() {
                return Err(CStr::from_ptr(error).to_string_lossy().into_owned());
            }
        }
        result.1.pop().ok_or("无法读取此应用。".into())
    }
    extern "C" fn done_callback(ctx: *mut c_void, error: *const c_char) {
        unsafe {
            let callback = *Box::from_raw(ctx as *mut Done);
            callback(if error.is_null() {
                Ok(())
            } else {
                Err(CStr::from_ptr(error).to_string_lossy().into_owned())
            });
        }
    }
    pub fn set_default(
        id: &str,
        kind: i32,
        app: &AppItem,
        done: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        let normalized = if kind == 2 {
            match super::normalize_extension(id) {
                Ok(ext) => ext,
                Err(error) => {
                    done(Err(error));
                    return;
                }
            }
        } else {
            id.trim().to_owned()
        };
        let values = (
            CString::new(normalized),
            CString::new(app.path.as_str()),
            CString::new(app.identifier.as_str()),
        );
        if let (Ok(id), Ok(path), Ok(bundle)) = values {
            let callback: Done = Box::new(done);
            unsafe {
                dh_set(
                    id.as_ptr(),
                    kind,
                    path.as_ptr(),
                    bundle.as_ptr(),
                    Box::into_raw(Box::new(callback)) as *mut c_void,
                    done_callback,
                );
            }
        } else {
            done(Err("输入包含无效字符。".into()));
        }
    }
}
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use super::{AppItem, RawApp};
    pub fn lookup(_: &str, _: i32) -> Result<(Option<RawApp>, Vec<RawApp>), String> {
        Err("目前支持 macOS 和 Linux。".into())
    }
    pub fn custom_app(_: &std::path::Path) -> Result<RawApp, String> {
        Err("目前支持 macOS 和 Linux。".into())
    }
    pub fn set_default(
        _: &str,
        _: i32,
        _: &AppItem,
        done: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        done(Err("目前支持 macOS 和 Linux。".into()));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn rejects_identifier_with_nul() {
        assert!(super::platform::lookup("https\0invalid", 1).is_err());
    }
}

#[cfg(test)]
mod category_tests {
    use super::{preset_at, CATEGORIES};

    #[test]
    fn category_indices_resolve_only_their_own_presets() {
        for (category_index, category) in CATEGORIES.iter().enumerate() {
            assert!(!category.presets.is_empty());
            for (index, preset) in category.presets.iter().enumerate() {
                assert_eq!(
                    preset_at(category_index as i32, index as i32)
                        .unwrap()
                        .identifier,
                    preset.identifier
                );
                assert_eq!(
                    preset.kind,
                    if category.label == "链接与协议" {
                        1
                    } else {
                        0
                    }
                );
            }
            assert!(preset_at(category_index as i32, category.presets.len() as i32).is_none());
        }
        assert!(preset_at(-1, 0).is_none());
        assert!(preset_at(0, -1).is_none());
    }
}

#[cfg(test)]
mod custom_input_tests {
    use super::*;

    #[test]
    fn extension_input_normalizes_and_rejects_paths_and_patterns() {
        for (input, expected) in [
            (" .FOO ", "foo"),
            ("foo", "foo"),
            ("tar.gz", "tar.gz"),
            (".自定义", "自定义"),
        ] {
            assert_eq!(normalize_extension(input).unwrap(), expected);
        }
        for invalid in [
            "", ".", "..foo", "foo.", "foo..bar", "*.foo", "foo/bar", "foo\\bar", "foo bar",
            "foo\0",
        ] {
            assert!(normalize_extension(invalid).is_err(), "{invalid:?}");
        }
        assert!(normalize_extension(&"a".repeat(129)).is_err());
    }

    #[test]
    fn custom_application_paths_require_existing_absolute_paths() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("An App With Spaces");
        std::fs::write(&path, "fixture").unwrap();
        assert_eq!(
            custom_application_path(path.to_str().unwrap()).unwrap(),
            std::fs::canonicalize(path).unwrap()
        );
        assert!(custom_application_path("relative/app").is_err());
        assert!(custom_application_path("").is_err());
        assert!(custom_application_path(root.path().join("missing").to_str().unwrap()).is_err());
    }
}
