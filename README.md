# Default Helper

使用 Rust 和 Slint 编写的默认应用管理器，支持 macOS 和 Linux（KDE Plasma、GNOME、niri）。可以查询和设置当前用户的文件类型及 URL 协议默认应用。

## 运行

需要 Rust 1.92 或更新版本及 C 编译器。macOS 需要开发工具（`xcode-select --install`）；Linux 需要 `pkg-config` 和 GLib/GIO 2.66 及以上的开发文件。

Linux 上安装构建依赖，例如：

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libglib2.0-dev libfontconfig1-dev libxkbcommon-dev shared-mime-info

# Fedora
sudo dnf install gcc gcc-c++ pkgconf-pkg-config glib2-devel fontconfig-devel libxkbcommon-devel shared-mime-info

# Arch Linux
sudo pacman -S --needed base-devel pkgconf glib2 fontconfig libxkbcommon shared-mime-info
```

```sh
cargo run --release
```

先选择文档、图片、音频、视频、压缩文件或链接分类，再选择常见类型；也可以输入文件类型（macOS 使用 UTI，例如 `com.adobe.pdf`；Linux 使用 MIME，例如 `application/pdf`）或 URL 协议名称（例如 `https`，不含冒号或网址），点击“查询”，在列表中选择应用。HTTP 和 HTTPS 是两个独立协议，需分别设置。应用名称和候选列表由系统提供，Linux 图标从已安装的图标主题或应用声明的绝对路径读取。界面会跟随系统的浅色或深色外观自动切换。

选择“文件管理器”分类可以设置打开文件夹的默认应用：macOS 使用 `public.folder`，Linux 使用 `inode/directory`。Linux 的候选列表也包含已安装的、声明了 `FileManager` 分类的桌面应用，即使它们没有声明文件夹类型。候选应用由系统提供，也可以手动添加应用。此设置影响遵循该关联的打开操作；系统的固定快捷方式或直接启动文件管理器的程序可能不受影响，macOS 的 Finder 也不能通过此设置整体替换。

选择类型下拉框中的“文件扩展名”，可以输入 `.foo`、`foo` 或 `tar.gz`，点击“查询”后选择默认应用。扩展名不区分大小写，不要输入路径或 `*.foo` 通配符。已知扩展名会映射到系统的 MIME / UTI；因此，多个扩展名如果对应同一类型，默认应用设置也会共享。Linux 上尚未注册的扩展名会在点击“设为默认”时创建当前用户的 MIME 定义并运行 `update-mime-database`（由 `shared-mime-info` 提供），仅查询不会创建文件。

在“自定义应用”中输入完整路径，或点击“浏览…”选择应用，然后点击“添加”（浏览选择后会自动添加）。支持 `~/` 和带空格的路径，不需要添加引号；添加后应用会进入候选列表，仍需点击“设为默认”才会写入系统关联。即使应用没有声明支持当前类型，也可以手动添加；应用本身需要能够处理对应文件或链接。

- **macOS**：选择 `.app` 应用包，例如 `/Applications/Visual Studio Code.app`。设置时向 Launch Services 注册该应用。
- **Linux**：选择已安装的 `.desktop` 文件、外部 `.desktop` 文件，或有执行权限的程序。已安装的应用复用原有条目；外部条目在设置时复制到 `$XDG_DATA_HOME/applications`（默认 `~/.local/share/applications`），保留原来的启动参数。可执行文件会生成用户级应用条目：文件关联传递本地文件路径，URL 关联传递 URL，不通过 shell 执行。需要额外启动参数或终端的程序，请先编写包含 `Exec` / `Terminal` 的 `.desktop` 文件后添加。

本次手动添加的候选应用会保留到退出；已经设为默认的关联及 Linux 生成的应用条目持久保存在系统中。重复设置同一应用会复用已生成的条目。Linux 的“浏览…”使用桌面文件选择门户；没有可用门户时可以直接输入路径。

macOS 12 及以上使用 NSWorkspace 和 UTType 接口；更早版本使用 Launch Services 兼容接口。系统可能要求确认更改，确认完成后界面会显示操作结果。设置作用于当前用户。由于 macOS 的类型继承和应用注册情况，特定文件的实际打开方式可能与父类型设置不同。自定义扩展名由系统解析为 UTI，未知类型是否能更改默认应用取决于 macOS 的注册与系统策略。

Linux 使用 [GIO AppInfo](https://docs.gtk.org/gio/iface.AppInfo.html) 读取应用的 `.desktop` 注册信息，按照 [XDG MIME Applications 规范](https://specifications.freedesktop.org/mime-apps/latest-single/) 查询和设置默认应用。设置写入 `$XDG_CONFIG_HOME/mimeapps.list`（默认 `~/.config/mimeapps.list`），已有的当前桌面专用用户配置（例如 `kde-mimeapps.list`、`gnome-mimeapps.list`、`niri-mimeapps.list`）如果覆盖了同一类型，也会同步更新该项；保留其他类型的设置。设置完成后会重新读取实际默认值，写入失败或仍被其他配置覆盖时显示错误。

KDE、GNOME 和 niri 共用此后端，支持 X11 / Wayland。请从当前用户的桌面会话启动，让程序继承 `XDG_CURRENT_DESKTOP`、`XDG_DATA_HOME`、`XDG_DATA_DIRS` 等会话环境；Flatpak 等应用的导出目录也应包含在 `XDG_DATA_DIRS` 中。niri 无需安装 GNOME Shell、KDE 或桌面门户即可管理关联；需有可用的 Wayland 图形库、GLib/GIO、共享 MIME 数据库和已注册的 `.desktop` 应用。需要手动指定图形后端时可使用：

```sh
SLINT_BACKEND=winit cargo run --release

# 图形驱动有问题时使用软件渲染
SLINT_BACKEND=winit-software cargo run --release
```

这里管理文件类型和 URL 协议关联；终端等桌面专用的首选程序设置不属于 MIME 关联范围。其他平台尚未实现默认应用后端。

运行 `cargo test` 可验证类型输入和 Linux 后端。Linux 测试在独立子进程和临时 XDG 目录中模拟 KDE、GNOME、niri 及组合桌面环境，不会修改当前用户的应用关联。
