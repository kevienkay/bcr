//! UI 自动化冒烟测试（egui_kittest）：
//! 证明 bcr 的 GUI 可以在无显示环境下驱动真实交互（点击/输入/断言），
//! 且可在 GitHub Actions 三平台 CI 上运行（纯软件渲染，无需 X/Wayland）。

use egui_kittest::{kittest::Queryable, Harness};

/// 最小可交互 UI：一个按钮 + 点击计数 + 文本输入框
struct CounterApp {
    clicks: u32,
    text: String,
}

impl CounterApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui) {
        if ui.button("点击我").clicked() {
            self.clicks += 1;
        }
        ui.label(format!("计数: {}", self.clicks));
        // 输入框带关联 label（kittest 可通过 label 定位输入框节点）
        ui.horizontal(|ui| {
            ui.label("输入框:");
            ui.text_edit_singleline(&mut self.text);
        });
    }
}

#[test]
fn kittest_click_updates_state() {
    let app = std::cell::RefCell::new(CounterApp {
        clicks: 0,
        text: String::new(),
    });
    let mut harness = Harness::new_ui(|ui| app.borrow_mut().ui(ui));
    harness.run();
    // 标签可查询（控件树存在）
    assert!(harness.query_by_label("计数: 0").is_some());
    // 点击按钮 → 计数 +1（通过控件树断言）
    harness.get_by_label("点击我").click();
    harness.run();
    assert!(harness.query_by_label("计数: 1").is_some());
    // 再点一次 → +2
    harness.get_by_label("点击我").click();
    harness.run();
    assert!(harness.query_by_label("计数: 2").is_some());
    // 内部状态同步更新
    assert_eq!(app.borrow().clicks, 2);
}

#[test]
fn kittest_text_input_via_type_text() {
    let app = std::cell::RefCell::new(CounterApp {
        clicks: 0,
        text: String::new(),
    });
    let mut harness = Harness::new_ui(|ui| app.borrow_mut().ui(ui));
    harness.run();
    // 用 role 精确定位单行输入框（label 查询可能命中 label 节点）
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("kittest-input");
    harness.run();
    // 输入框内容在控件树中以 value 暴露；直接断言内部状态更稳
    assert_eq!(app.borrow().text, "kittest-input");
}

#[test]
fn kittest_renders_and_queries_widgets() {
    let app = std::cell::RefCell::new(CounterApp {
        clicks: 0,
        text: "hello".to_string(),
    });
    let mut harness = Harness::new_ui(|ui| app.borrow_mut().ui(ui));
    harness.run();
    // 按钮与计数标签都在控件树中（label 查询）
    assert!(harness.query_by_label("点击我").is_some());
    assert!(harness.query_by_label("计数: 0").is_some());
    // 输入框内容通过内部状态断言
    assert_eq!(app.borrow().text, "hello");
}

#[test]
fn kittest_headless_no_panic() {
    // 无头环境（无显示服务器）渲染多帧不 panic = CI 可跑
    let app = std::cell::RefCell::new(CounterApp {
        clicks: 0,
        text: "多帧渲染".to_string(),
    });
    let mut harness = Harness::new_ui(|ui| app.borrow_mut().ui(ui));
    for _ in 0..5 {
        harness.run();
    }
    assert!(harness.query_by_label("点击我").is_some());
}
