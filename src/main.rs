use slint::{ComponentHandle, Model, SharedString, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

slint::include_modules!();

struct Preset {
    label: &'static str,
    identifier: &'static str,
    kind: i32,
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
                kind: 0,
            },
            Preset {
                label: "纯文本 (.txt)",
                identifier: "public.plain-text",
                kind: 0,
            },
            Preset {
                label: "富文本 (.rtf)",
                identifier: "public.rtf",
                kind: 0,
            },
            Preset {
                label: "HTML 文档 (.html)",
                identifier: "public.html",
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
                kind: 0,
            },
            Preset {
                label: "PNG 图片 (.png)",
                identifier: "public.png",
                kind: 0,
            },
            Preset {
                label: "GIF 图片 (.gif)",
                identifier: "com.compuserve.gif",
                kind: 0,
            },
            Preset {
                label: "TIFF 图片 (.tiff)",
                identifier: "public.tiff",
                kind: 0,
            },
            Preset {
                label: "HEIC 图片 (.heic)",
                identifier: "public.heic",
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
                kind: 0,
            },
            Preset {
                label: "MPEG-4 音频 (.m4a)",
                identifier: "public.mpeg-4-audio",
                kind: 0,
            },
            Preset {
                label: "WAV 音频 (.wav)",
                identifier: "com.microsoft.waveform-audio",
                kind: 0,
            },
            Preset {
                label: "AIFF 音频 (.aiff)",
                identifier: "public.aiff-audio",
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
                kind: 0,
            },
            Preset {
                label: "QuickTime 视频 (.mov)",
                identifier: "com.apple.quicktime-movie",
                kind: 0,
            },
            Preset {
                label: "AVI 视频 (.avi)",
                identifier: "public.avi",
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
                kind: 0,
            },
            Preset {
                label: "Gzip 压缩文件 (.gz)",
                identifier: "org.gnu.gnu-zip-archive",
                kind: 0,
            },
            Preset {
                label: "Tar 归档文件 (.tar)",
                identifier: "public.tar-archive",
                kind: 0,
            },
        ],
    },
    Category {
        label: "链接与协议",
        presets: &[
            Preset {
                label: "网页链接 (HTTP)",
                identifier: "http",
                kind: 1,
            },
            Preset {
                label: "安全网页链接 (HTTPS)",
                identifier: "https",
                kind: 1,
            },
            Preset {
                label: "邮件链接 (mailto)",
                identifier: "mailto",
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
) {
    ui.set_identifier(preset.identifier.into());
    ui.set_kind_index(preset.kind);
    *queried.borrow_mut() = (preset.identifier.to_owned(), preset.kind);
    refresh(ui, preset.identifier, preset.kind, generation, None);
}

#[derive(Default)]
struct RawApp {
    name: String,
    identifier: String,
    path: String,
    icon: Vec<u8>,
}

impl From<RawApp> for AppItem {
    fn from(app: RawApp) -> Self {
        Self {
            name: app.name.into(),
            identifier: app.identifier.into(),
            path: app.path.into(),
            icon: slint::Image::load_from_data(&app.icon, Some("png")).unwrap_or_default(),
        }
    }
}

fn refresh(
    ui: &AppWindow,
    identifier: &str,
    kind: i32,
    generation: &Arc<AtomicU64>,
    message: Option<String>,
) {
    let request = generation.fetch_add(1, Ordering::Relaxed) + 1;
    let id = identifier.trim().to_owned();
    ui.set_applications(Default::default());
    ui.set_current_name("未找到默认应用".into());
    ui.set_current_id("".into());
    ui.set_current_icon(Default::default());
    if id.is_empty() {
        ui.set_loading(false);
        ui.set_status("请输入 UTI 或 URL 协议。".into());
        return;
    }
    ui.set_loading(true);
    ui.set_status("正在查询可用应用…".into());
    let weak = ui.as_weak();
    let generation = generation.clone();
    std::thread::spawn(move || {
        let (current, apps) = platform::lookup(&id, kind == 1);
        let _ = slint::invoke_from_event_loop(move || {
            if generation.load(Ordering::Relaxed) != request {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.set_loading(false);
                if let Some(app) = current {
                    let app = AppItem::from(app);
                    ui.set_current_name(app.name);
                    ui.set_current_id(app.identifier);
                    ui.set_current_icon(app.icon);
                }
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

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
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
    let generation = Arc::new(AtomicU64::new(0));
    let queried = Rc::new(RefCell::new((String::new(), 0)));
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
                select_preset(&ui, preset, &state, &requests);
            }
        }
    });
    let state = queried.clone();
    let requests = generation.clone();
    let weak = ui.as_weak();
    ui.on_preset_selected(move |index| {
        if let Some(ui) = weak.upgrade() {
            if let Some(preset) = preset_at(ui.get_category_index(), index) {
                select_preset(&ui, preset, &state, &requests);
            }
        }
    });
    let state = queried.clone();
    let requests = generation.clone();
    let weak = ui.as_weak();
    ui.on_lookup(move |id, kind| {
        if let Some(ui) = weak.upgrade() {
            *state.borrow_mut() = (id.trim().to_string(), kind);
            refresh(&ui, &id, kind, &requests, None);
        }
    });
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
                    ui.set_status("正在等待 macOS 完成设置，可能需要系统确认…".into());
                    let weak = ui.as_weak();
                    let target = id.trim().to_string();
                    let request_target = target.clone();
                    let requests = requests.clone();
                    platform::set_default(&request_target, kind == 1, &app, move |result| {
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
                                    refresh(&ui, &target, kind, &requests, Some(message));
                                } else {
                                    ui.set_status(message.into());
                                }
                            }
                        });
                    });
                }
                _ => {
                    *state.borrow_mut() = (id.trim().to_string(), kind);
                    refresh(&ui, &id, kind, &requests, None);
                    ui.set_status("类型或候选应用已变化，请重新选择。".into());
                }
            }
        }
    });
    select_preset(&ui, &CATEGORIES[0].presets[0], &queried, &generation);
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
            scheme: bool,
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
            scheme: bool,
            path: *const c_char,
            bundle: *const c_char,
            ctx: *mut c_void,
            callback: extern "C" fn(*mut c_void, *const c_char),
        );
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
                icon,
            };
            if current {
                *default = Some(app);
            } else {
                apps.push(app);
            }
        }
    }
    pub fn lookup(id: &str, scheme: bool) -> (Option<RawApp>, Vec<RawApp>) {
        let Ok(id) = CString::new(id) else {
            return (None, vec![]);
        };
        let mut result = (None, Vec::new());
        unsafe {
            dh_lookup(
                id.as_ptr(),
                scheme,
                &mut result as *mut _ as *mut c_void,
                app_callback,
            );
        }
        result
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
        scheme: bool,
        app: &AppItem,
        done: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        let values = (
            CString::new(id),
            CString::new(app.path.as_str()),
            CString::new(app.identifier.as_str()),
        );
        if let (Ok(id), Ok(path), Ok(bundle)) = values {
            let callback: Done = Box::new(done);
            unsafe {
                dh_set(
                    id.as_ptr(),
                    scheme,
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
#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{AppItem, RawApp};
    pub fn lookup(_: &str, _: bool) -> (Option<RawApp>, Vec<RawApp>) {
        (None, vec![])
    }
    pub fn set_default(
        _: &str,
        _: bool,
        _: &AppItem,
        done: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        done(Err("目前仅支持 macOS".into()));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn rejects_identifier_with_nul() {
        let (current, apps) = super::platform::lookup("https\0invalid", true);
        assert!(current.is_none());
        assert!(apps.is_empty());
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
