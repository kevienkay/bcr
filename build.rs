// Windows 正式分发：把 assets/bcr.ico 嵌入 bcr.exe（资源节），
// 资源管理器中显示图标、MSI 快捷方式可复用同一份 icon。
// 其他平台无操作。
#[cfg(windows)]
fn main() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/bcr.ico");
    res.set_language(0x0409); // en-US，避免中文 locale 导致资源解析问题
    if let Err(e) = res.compile() {
        eprintln!("winresource compile failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {}
