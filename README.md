# Default Helper

使用 Rust 和 Slint 编写的默认应用管理器，目前支持 macOS。可以查询和设置当前用户的文件类型及 URL 协议默认应用。

## 运行

需要 Rust 工具链和 macOS 开发工具（`xcode-select --install`）。

```sh
cargo run --release
```

先选择文档、图片、音频、视频、压缩文件或链接分类，再选择常见类型；也可以输入文件类型 UTI（例如 `com.adobe.pdf`）/ URL 协议（例如 `https`），点击“查询”，在列表中选择应用。HTTP 和 HTTPS 是两个独立协议，需分别设置。应用名称、图标和候选列表由系统提供。界面会跟随系统的浅色或深色外观自动切换。

macOS 12 及以上使用 NSWorkspace 和 UTType 接口；更早版本使用 Launch Services 兼容接口。系统可能要求确认更改，确认完成后界面会显示操作结果。设置作用于当前用户。由于 macOS 的类型继承和应用注册情况，特定文件的实际打开方式可能与父类型设置不同。

非 macOS 平台可以编译界面，但尚未实现平台后端。
