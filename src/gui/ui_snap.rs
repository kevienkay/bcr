//! 无头 UI 截图工具（开发/验收用，非 CI 常驻测试）。
//!
//! 渲染关键界面到 PNG，便于查看 UI 效果与美化前后对比。
//! 用法：
//! ```bash
//! BCR_SNAP_DIR=/tmp/bcr-ui cargo test --lib gui::ui_snap -- --ignored
//! ```
//! 所有用例默认 `#[ignore]`（不写文件、不影响 CI）。

use super::*;
use eframe::egui;
use egui_kittest::Harness;

/// 渲染当前帧并保存 PNG 到 BCR_SNAP_DIR（未设置环境变量时静默跳过）
fn save<State>(h: &mut Harness<'_, State>, name: &str) {
    // 多跑几帧：build_eframe 下首帧视口尺寸尚未应用，面板（含底部状态栏）
    // 要到后续帧才落到最终位置；run_steps 而非 run() 避免 spinner 超限 panic
    h.run_steps(12);
    let Ok(dir) = std::env::var("BCR_SNAP_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&dir);
    match h.render() {
        Ok(img) => {
            let p = dir.join(format!("{name}.png"));
            img.save(&p).unwrap();
            eprintln!("saved {}", p.display());
        }
        Err(e) => eprintln!("render {name} failed: {e}"),
    }
}

/// 临时目录里写两个文本文件，返回 (左路径, 右路径, 保持目录存活的句柄)
/// 注意：tempdir 句柄必须存活到截图结束，否则路径指向的文件会被删除
fn write_pair(prefix: &str, left: &str, right: &str) -> (tempfile::TempDir, String, String) {
    let d = tempfile::tempdir().unwrap();
    let l = d.path().join(format!("{prefix}-l.txt"));
    let r = d.path().join(format!("{prefix}-r.txt"));
    std::fs::write(&l, left).unwrap();
    std::fs::write(&r, right).unwrap();
    (
        d,
        l.to_str().unwrap().to_string(),
        r.to_str().unwrap().to_string(),
    )
}

const SAMPLE_L: &str = "// 左侧文件（演示差异）\nfn main() {\n    println!(\"hello\");\n    let x = 1;\n    println!(\"world\");\n}\n";
const SAMPLE_R: &str = "// 右侧文件（演示差异）\nfn main() {\n    println!(\"hello world\");\n    let x = 42;\n    let y = x + 1;\n    println!(\"world!\");\n}\n";

// ---- 欢迎页（空会话）----

#[test]
#[ignore]
fn snap_welcome_dark() {
    let mut app = DiffApp::new(Settings::default());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_ui(|ui| app.welcome_ui(ui));
    save(&mut h, "welcome_dark");
}

#[test]
#[ignore]
fn snap_welcome_light() {
    let mut app = DiffApp::new(Settings::default());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .with_theme(egui::Theme::Light)
        .build_ui(|ui| app.welcome_ui(ui));
    save(&mut h, "welcome_light");
}

// ---- 完整应用：菜单栏 + 标签栏 + 内容 + 状态栏 ----

#[test]
#[ignore]
fn snap_app_difftab() {
    let (_d, l, r) = write_pair("snap-diff", SAMPLE_L, SAMPLE_R);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            let mut t = crate::gui::difftab::DiffTab::new();
            t.load_pair(&l, &r, ViewOptions::default());
            app.add_tab(Tab::Diff(t));
            app
        });
    save(&mut h, "app_difftab");
}

#[test]
#[ignore]
fn snap_app_difftab_light() {
    let (_d, l, r) = write_pair("snap-diff-l", SAMPLE_L, SAMPLE_R);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .with_theme(egui::Theme::Light)
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            let mut t = crate::gui::difftab::DiffTab::new();
            t.load_pair(&l, &r, ViewOptions::default());
            app.add_tab(Tab::Diff(t));
            app
        });
    save(&mut h, "app_difftab_light");
}

#[test]
#[ignore]
fn snap_app_dirtab() {
    let d = tempfile::tempdir().unwrap();
    let (l, r) = (d.path().join("left"), d.path().join("right"));
    std::fs::create_dir_all(&l).unwrap();
    std::fs::create_dir_all(&r).unwrap();
    std::fs::write(l.join("same.txt"), "x").unwrap();
    std::fs::write(l.join("only_left.txt"), "a").unwrap();
    std::fs::write(r.join("same.txt"), "x").unwrap();
    std::fs::write(r.join("only_right.txt"), "b").unwrap();
    std::fs::create_dir_all(l.join("sub")).unwrap();
    std::fs::create_dir_all(r.join("sub")).unwrap();
    std::fs::write(l.join("sub/deep.txt"), "deep").unwrap();
    std::fs::write(r.join("sub/deep.txt"), "DEEP!").unwrap();

    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            app.add_tab(Tab::Dir(crate::gui::dirtab::DirTab::new(
                l.to_str().unwrap(),
                r.to_str().unwrap(),
            )));
            app
        });
    // 多跑几帧等后台扫描完成
    for _ in 0..30 {
        h.run_steps(2);
    }
    save(&mut h, "app_dirtab");
}

// ---- 空状态 ----

#[test]
#[ignore]
fn snap_app_empty_diff() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            app.add_tab(Tab::Diff(crate::gui::difftab::DiffTab::new()));
            app
        });
    save(&mut h, "app_empty_diff");
}

// ---- 三路合并冲突 ----

#[test]
#[ignore]
fn snap_app_mergetab() {
    let d = tempfile::tempdir().unwrap();
    let (b, l, r) = (
        d.path().join("base.txt"),
        d.path().join("left.txt"),
        d.path().join("right.txt"),
    );
    std::fs::write(&b, "line1\nshared\nbase-only\nline4\n").unwrap();
    std::fs::write(&l, "line1\nLEFT change\nbase-only\nline4\n").unwrap();
    std::fs::write(&r, "line1\nRIGHT change\nbase-only\nline4\n").unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            app.add_tab(Tab::Merge(crate::gui::mergetab::MergeTab::new(
                b.to_str().unwrap(),
                l.to_str().unwrap(),
                r.to_str().unwrap(),
            )));
            app
        });
    save(&mut h, "app_mergetab");
}

// ---- P54：CSV 表格视图 ----

#[test]
#[ignore]
fn snap_app_csvtab() {
    let d = tempfile::tempdir().unwrap();
    let (l, r) = (d.path().join("l.csv"), d.path().join("r.csv"));
    std::fs::write(
        &l,
        "id,name,age\n1,alice,30\n2,bob,25\n3,carol,40\n",
    )
    .unwrap();
    std::fs::write(
        &r,
        "id,name,age\n1,alice,31\n2,bob,25\n4,dave,22\n",
    )
    .unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            app.add_tab(Tab::Csv(crate::gui::csvtab::CsvTab::new(
                l.to_str().unwrap(),
                r.to_str().unwrap(),
            )));
            app
        });
    for _ in 0..6 {
        h.run_steps(2);
    }
    save(&mut h, "app_csvtab");
}

#[test]
#[ignore]
fn snap_app_csvtab_light() {
    let d = tempfile::tempdir().unwrap();
    let (l, r) = (d.path().join("l.csv"), d.path().join("r.csv"));
    std::fs::write(&l, "id,name\n1,alice\n2,bob\n").unwrap();
    std::fs::write(&r, "id,name\n1,alice\n2,BOB\n").unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .with_theme(egui::Theme::Light)
        .build_eframe(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let mut app = DiffApp::new(Settings::default());
            app.add_tab(Tab::Csv(crate::gui::csvtab::CsvTab::new(
                l.to_str().unwrap(),
                r.to_str().unwrap(),
            )));
            app
        });
    for _ in 0..6 {
        h.run_steps(2);
    }
    save(&mut h, "app_csvtab_light");
}

// ---- P54：欢迎页（带会话数据，验证会话列表图标）----

#[test]
#[ignore]
fn snap_welcome_with_sessions() {
    // 临时 HOME 注入会话文件，验证会话列表 📁 图标
    let home = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", home.path());
    std::fs::create_dir_all(home.path().join(".bcr")).unwrap();
    std::fs::write(
        home.path().join(".bcr-sessions.toml"),
        "[sessions.backup]\nleft = \"/Users/alice/projects/app/src\"\nright = \"/Users/alice/backups/app-src-2026-08-01\"\n",
    )
    .unwrap();
    let mut app = DiffApp::new(Settings::default());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_ui(|ui| app.welcome_ui(ui));
    save(&mut h, "welcome_sessions");
    std::env::remove_var("HOME");
}
