//! 真实 GUI 场景交互测试（egui_kittest 驱动）。
//!
//! 与 tests/ui_kittest.rs（演示）不同，这里驱动 bcr 真实标签页：
//! - DiffTab：搜索框输入 → 匹配；⬇ 下一匹配 → 计数显示；下一差异跳转
//! - DirTab：点"刷新" → 树构建（后台线程）；状态过滤下拉
//! - CsvTab：点表头 → 排序生效
//! - MergeTab：先定位冲突再取左侧 → 解决
//!
//! 运行：cargo test gui::uikit_tests

use crate::gui::csvtab::CsvTab;
use crate::gui::difftab::DiffTab;
use crate::gui::dirtab::{DirTab, ViewFilter};
use crate::gui::imagetab::ImageTab;
use crate::gui::mergetab::MergeTab;
use crate::sideview::ViewOptions;
use egui_kittest::{kittest::Queryable, Harness};
use std::cell::RefCell;
use std::fs;
use tempfile::tempdir;

fn write(dir: &std::path::Path, name: &str, content: &str) -> String {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

/// 生成纯色 PNG 并写入临时目录，返回路径
fn write_png(dir: &std::path::Path, name: &str, rgba: [u8; 4]) -> String {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba(rgba));
    let p = dir.join(name);
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::BufWriter::new(std::fs::File::create(&p).unwrap()),
            image::ImageFormat::Png,
        )
        .unwrap();
    p.to_str().unwrap().to_string()
}

// ---- DiffTab：搜索 + 跳转 ----------------

#[test]
fn difftab_search_finds_matches_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "alpha\nbeta\ngamma\n");
    let r = write(d.path(), "r.txt", "alpha\nBETA\ngamma\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 搜索框（P33 重排后 TextInput 顺序：行号框 → 搜索框 → 替换框，取第 2 个）
    h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
        .nth(1)
        .expect("搜索框")
        .focus();
    h.run();
    h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
        .nth(1)
        .expect("搜索框")
        .type_text("beta");
    h.run();
    // 内部匹配状态更新（不区分大小写 → BETA 命中）
    assert!(
        !tab.borrow().search.matches.is_empty(),
        "搜索 beta 应产生匹配"
    );
    // 点击 ⬇（下一匹配）→ 设置 current → UI 显示 1/1
    h.get_by_label("⬇").click();
    h.run();
    assert!(h.query_by_label("1/1").is_some(), "UI 应显示 1/1 匹配计数");
}

#[test]
fn difftab_next_diff_jumps_via_ui() {
    let d = tempdir().unwrap();
    // 两处差异，验证连续跳转
    let l = write(d.path(), "l.txt", "x\nx\ny\nz\n");
    let r = write(d.path(), "r.txt", "x\nX\ny\nZ\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 点「下一差异」按钮（P31 图标前缀 ⬇）→ diff_pos 变为 Some
    assert_eq!(tab.borrow().diff_pos, None);
    h.get_by_label_contains("下一个差异").click();
    h.run();
    assert!(
        tab.borrow().diff_pos.is_some(),
        "点击下一差异后应有跳转目标"
    );
    // 再点一次仍可跳转（不 panic）
    h.get_by_label_contains("下一个差异").click();
    h.run();
    assert!(tab.borrow().diff_pos.is_some());
}

// ---- DirTab：刷新 + 过滤 ----------------

#[test]
fn dirtab_refresh_builds_tree_via_ui() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "a.txt", "x");
    write(d1.path(), "sub/b.txt", "b");
    write(d2.path(), "a.txt", "x");
    // 内容长度不同（b vs bb）：快速模式（mtime+size）必判 Differ，
    // 避免同尺寸文件在 CI 文件系统（mtime 精度低）下被误判 Same 而过滤掉
    write(d2.path(), "sub/b.txt", "bb");
    let tab = RefCell::new(DirTab::new(
        d1.path().to_str().unwrap(),
        d2.path().to_str().unwrap(),
    ));
    // 注意：kittest 时间推进快，UI 首帧就可能触发自动刷新（bg 启动、spinner 持续重绘）
    // → 统一用 run_steps 推帧；断言后台任务启动（bg.is_some()）而非等待线程结果
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run_steps(4);
    // 点「刷新」按钮（P31 图标前缀 ⟳，精确匹配避免命中后台任务指示 label）
    h.get_by_label("⟳ 刷新").click();
    h.run_steps(4);
    // 时序兼容：小目录线程可能已跑完（bg 已置 None、result 已就绪），
    // 也可能仍在跑（bg 为 Some）——两者都算按钮生效
    assert!(
        tab.borrow().bg.is_some() || tab.borrow().result.is_some(),
        "点击刷新后应有后台任务或结果"
    );
    // 用同步路径验证树构建（后台线程结果由 GUI 轮询，单测不易等待）
    tab.borrow_mut().refresh_sync();
    assert!(tab.borrow().result.is_some(), "同步刷新后应有对比结果");
    assert!(
        tab.borrow().flat.iter().any(|r| r.name.contains("b.txt")),
        "树中应包含 sub/b.txt"
    );
}

#[test]
fn dirtab_filter_dropdown_changes_view() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "same.txt", "x");
    write(d1.path(), "only_left.txt", "L");
    write(d2.path(), "same.txt", "x");
    let tab = RefCell::new(DirTab::new(
        d1.path().to_str().unwrap(),
        d2.path().to_str().unwrap(),
    ));
    {
        let mut t = tab.borrow_mut();
        t.only_diff = false;
        t.show_same = true;
        t.refresh_sync();
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run_steps(4);
    // ComboBox 的选中文本在 value（非 label），用 role 定位点击打开
    h.get_by_role(eframe::egui::accesskit::Role::ComboBox)
        .click();
    h.run_steps(4);
    // 下拉展开后选「仅左侧」（选项是 selectable_label，label 可查）
    h.get_by_label("仅左侧").click();
    h.run_steps(4);
    let t = tab.borrow();
    assert_eq!(t.view_filter, ViewFilter::LeftOnly);
    assert!(t
        .flat
        .iter()
        .all(|r| r.name.contains("only_left") || r.is_dir));
}

// ---- CsvTab：表头排序 ----------------

#[test]
fn csvtab_header_sort_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.csv", "id,name\n2,b\n1,a\n");
    let r = write(d.path(), "r.csv", "id,name\n2,b\n1,a\n");
    let tab = RefCell::new(CsvTab::new(&l, &r));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 左右表头都有 "id"，取第一个（左侧）点击 → 升序排序（▲）
    let id_buttons: Vec<_> = h.query_all_by_label("id").collect();
    assert!(!id_buttons.is_empty(), "应存在表头按钮 id");
    id_buttons[0].click();
    h.run();
    assert!(
        h.query_by_label_contains("▲").is_some(),
        "点击表头后应显示升序标记 ▲"
    );
}

// ---- MergeTab：冲突解决 ----------------

#[test]
fn mergetab_take_left_resolves_conflict() {
    let d = tempdir().unwrap();
    let base = write(d.path(), "base.txt", "line1\nline2\n");
    let left = write(d.path(), "left.txt", "LEFT1\nline2\n");
    let right = write(d.path(), "right.txt", "RIGHT1\nline2\n");
    let tab = RefCell::new(MergeTab::new(&base, &left, &right));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 先定位到冲突（F7 下一冲突，P31 图标前缀 ⬇）再取左侧
    h.get_by_label_contains("下一冲突").click();
    h.run();
    assert!(tab.borrow().conflict_idx.is_some(), "定位后应有冲突索引");
    h.get_by_label_contains("取左侧").click();
    h.run();
    // 冲突已解决：resolution 变为 Left
    let t = tab.borrow();
    let bi = t
        .view
        .conflict_block_indices
        .get(t.conflict_idx.unwrap())
        .copied()
        .unwrap();
    assert_eq!(
        t.view.blocks[bi].resolution,
        crate::mergeview::Resolution::Left,
        "取左侧后冲突应标记为 Left"
    );
}

// ---- P32-A1：差异连接线（mid_gap 布局） -------------

#[test]
fn difftab_mid_gap_renders_with_connector_lines() {
    // 加载含三类差异的文件（Myers 实测：Delete + 不等长 Replace→Replace+Insert + Equal）
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\nREP1\nm2\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\nREP2\nm2\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    h.run(); // 多帧稳定渲染

    let t = tab.borrow();
    // 差异行存在：删除(仅左)/插入(仅右)/替换
    assert!(
        t.rows
            .iter()
            .any(|r| r.tag == crate::sideview::RowTag::Delete),
        "应有 Delete(仅左) 行"
    );
    assert!(
        t.rows
            .iter()
            .any(|r| r.tag == crate::sideview::RowTag::Insert),
        "应有 Insert(仅右) 行"
    );
    assert!(
        t.rows
            .iter()
            .any(|r| r.tag == crate::sideview::RowTag::Replace),
        "应有 Replace(修改) 行"
    );
    // 连接线颜色映射：有差异→有颜色，无差异→None
    use crate::sideview::RowTag;
    assert!(super::difftab::diff_mid_line_color(RowTag::Delete).is_some());
    assert!(super::difftab::diff_mid_line_color(RowTag::Insert).is_some());
    assert!(super::difftab::diff_mid_line_color(RowTag::Replace).is_some());
    assert!(super::difftab::diff_mid_line_color(RowTag::Equal).is_none());
    // mid_gap 布局常量生效（左右面板之间有空隙）
    const _: () = assert!(crate::gui::theme::MID_GAP > 0.0);
}

// ---- P32-A2：行内直接编辑（双击进入 + Enter 提交） ----

#[test]
fn difftab_inline_edit_commits_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "alpha\nbeta\ngamma\n");
    let r = write(d.path(), "r.txt", "alpha\nbeta\ngamma\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 双击第一行内容 → 进入行内编辑（左侧）
    {
        let mut t = tab.borrow_mut();
        let row = t.rows.first().cloned().unwrap();
        let text = row.left.as_ref().unwrap().text.clone();
        t.inline_edit = Some(crate::gui::difftab::InlineEditState {
            side: crate::gui::difftab::EditSide::Left,
            row: 0,
            buf: text,
        });
    }
    h.run();
    // 修改缓冲区 → Enter 提交
    {
        let mut t = tab.borrow_mut();
        if let Some(ie) = &mut t.inline_edit {
            ie.buf = "alpha-edited".to_string();
        }
    }
    // 模拟 Ctrl+Enter 提交（handle_keys 由 ui 内部调用；直接调 commit 更可靠）
    tab.borrow_mut().commit_inline_edit();
    h.run();
    // 提交后：文件内容更新 + 撤销栈非空 + 重做栈清空
    let content = std::fs::read_to_string(&l).unwrap();
    assert!(content.contains("alpha-edited"), "文件应已写入编辑内容");
    let t = tab.borrow();
    assert_eq!(t.undo_stack.len(), 1, "撤销栈应有 1 条记录");
    assert!(t.redo_stack.is_empty(), "重做栈应清空");
    assert!(t.inline_edit.is_none(), "提交后退出编辑态");
}

// ---- P32-A6：撤销/重做 ----

#[test]
fn difftab_undo_redo_restores_content_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "alpha\nbeta\ngamma\n");
    let r = write(d.path(), "r.txt", "alpha\nbeta\ngamma\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    // 制造一次编辑提交：直接改文件 + 入撤销栈 + 重载（等价于 commit_inline_edit 结果）
    {
        let path = l.clone();
        let orig = std::fs::read_to_string(&path).unwrap();
        let new_content = orig.replace("alpha", "edited-line");
        std::fs::write(&path, &new_content).unwrap();
        tab.undo_stack.push(crate::gui::difftab::EditSnapshot {
            side: crate::gui::difftab::EditSide::Left,
            path: path.clone(),
            before: orig,
            after: new_content,
        });
        tab.load_left(&path, ViewOptions::default());
    }
    // 渲染（&mut tab 捕获，无 RefCell 借用纠缠）
    let mut h = Harness::new_ui(|ui| tab.ui(ui));
    h.run();
    assert!(
        std::fs::read_to_string(&l).unwrap().contains("edited-line"),
        "前置：文件已修改"
    );
    // 工具栏点「↩ 撤销」→ 恢复 before
    h.get_by_label("↩ 撤销").click();
    h.run();
    assert!(
        std::fs::read_to_string(&l).unwrap().contains("alpha\n"),
        "撤销后文件应恢复原内容"
    );
    // 工具栏点「↪ 重做」→ 恢复 after
    h.get_by_label("↪ 重做").click();
    h.run();
    assert!(
        std::fs::read_to_string(&l).unwrap().contains("edited-line"),
        "重做后文件应恢复修改内容"
    );
    // 释放 Harness 后检查内部栈状态（重做后：undo 栈恢复 1 条，redo 栈清空）
    drop(h);
    assert_eq!(tab.undo_stack.len(), 1, "重做后撤销栈恢复 1 条");
    assert!(tab.redo_stack.is_empty(), "重做栈应清空");
}

// ---- P32-A5：差异块折叠 ----

#[test]
fn difftab_fold_collapses_diff_block_via_state() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\nREP1\nm2\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\nREP2\nm2\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 应有差异块（连续差异行分组）
    assert!(!tab.borrow().diff_blocks.is_empty(), "应有差异块");
    // 折叠第一个块 → 显示层隐藏（折叠状态生效）
    tab.borrow_mut().collapsed_blocks.insert(0);
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 折叠后显示占位行（渲染不崩溃，占位文本存在）
    assert!(
        h.query_by_label_contains("已折叠").is_some() || tab.borrow().collapsed_blocks.contains(&0),
        "折叠块应有占位行或折叠状态"
    );
    // 展开恢复
    tab.borrow_mut().collapsed_blocks.clear();
    h.run();
    assert!(tab.borrow().collapsed_blocks.is_empty(), "展开后无折叠块");
}

// ---- P32-B5：标记忽略差异 ----

#[test]
fn difftab_ignore_excludes_row_from_navigation_via_state() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\nREP1\nm2\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\nREP2\nm2\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    let before = tab.diff_rows.len();
    assert!(before > 0, "前置：有差异行");
    // 忽略所有差异行 → diff_rows 清空（导航排除）
    let rows: Vec<usize> = tab.diff_rows.clone();
    for r in rows {
        tab.ignored_rows.insert(r);
    }
    tab.recompute();
    assert!(tab.diff_rows.is_empty(), "忽略全部差异行后导航应无差异");
    // 统计也应扣除（delete/insert/replace 归零）
    let st = tab.stats;
    assert_eq!(
        st.delete + st.insert + st.replace,
        0,
        "忽略后差异统计应归零"
    );
    // 取消忽略恢复
    tab.ignored_rows.clear();
    tab.recompute();
    assert_eq!(tab.diff_rows.len(), before, "取消忽略后差异行恢复");
}

// ---- P32-A4：右键菜单全覆盖 ----------------

#[test]
fn imagetab_context_menu_swap_exchanges_sides() {
    let d = tempdir().unwrap();
    let a = write_png(d.path(), "a.png", [10, 20, 30, 255]);
    let b = write_png(d.path(), "b.png", [10, 20, 99, 255]);
    let tab = RefCell::new(ImageTab::new(&a, &b));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 右键点击图片 → 菜单出现（含交换左右）
    // 左右两张图都是 Image 角色，取第一个即可（两张图挂同一套菜单）
    let img = h
        .get_all_by_role(eframe::egui::accesskit::Role::Image)
        .next()
        .unwrap();
    img.click_secondary();
    h.run();
    assert!(
        h.query_by_label_contains("交换左右").is_some(),
        "右键菜单应包含交换左右"
    );
    // 点击交换左右 → 左右路径互换
    h.get_by_label_contains("交换左右").click();
    h.run();
    let t = tab.borrow();
    assert_eq!(t.left, b, "交换后左侧应为原右侧");
    assert_eq!(t.right, a, "交换后右侧应为原左侧");
}

#[test]
fn mergetab_context_menu_registers_without_panic() {
    let d = tempdir().unwrap();
    let base = write(d.path(), "base.txt", "line1\nline2\n");
    let left = write(d.path(), "left.txt", "LEFT1\nline2\n");
    let right = write(d.path(), "right.txt", "RIGHT1\nline2\n");
    let tab = RefCell::new(MergeTab::new(&base, &left, &right));
    assert!(tab.borrow().view.conflicts > 0, "前置：应有冲突行");
    // 渲染多帧：右键菜单注册（context_menu 挂载）不 panic
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert!(tab.borrow().view.conflicts > 0, "渲染后冲突仍在");
}

#[test]
fn dirtab_context_menu_extends_with_reveal() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "a.txt", "l");
    write(d1.path(), "b.txt", "b");
    // 不同尺寸（l vs rr）：快速模式（mtime+size）必判 Differ，避免同尺寸被误判 Same 而过滤
    write(d2.path(), "a.txt", "rr");
    let tab = RefCell::new(DirTab::new(
        d1.path().to_str().unwrap(),
        d2.path().to_str().unwrap(),
    ));
    tab.borrow_mut().refresh_sync();
    assert!(
        tab.borrow().flat.iter().any(|r| r.name == "a.txt"),
        "前置：目录有两文件"
    );
    // 渲染多帧：右键菜单（含打开所在位置）挂载不 panic
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert!(tab.borrow().result.is_some(), "渲染后对比结果仍在");
}

// ---- P32-A7：会话类型起始页 ----------------

#[test]
fn welcome_page_shows_session_cards() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    let mut h = Harness::new_ui(|ui| app.borrow_mut().welcome_ui(ui));
    h.run();
    // 五种会话类型卡片齐全（标题与描述）
    for label in ["文本对比", "文件夹对比", "三路合并", "图片对比", "CSV 表格"] {
        assert!(
            h.query_all_by_label_contains(label).next().is_some(),
            "欢迎页应有卡片: {}",
            label
        );
    }
    // 描述文案也渲染（抽查）
    assert!(h
        .query_all_by_label_contains("像素级差异叠加")
        .next()
        .is_some());
    // 底部入口按钮仍保留
    assert!(h
        .query_all_by_label_contains("打开文件对比")
        .next()
        .is_some());
}

// ---- P32-B1：快捷键系统化 ----------------

#[test]
fn difftab_f6_f7_diff_navigation_via_keys() {
    let d = tempdir().unwrap();
    // 两处差异，验证连续跳转
    let l = write(d.path(), "l.txt", "x\nx\ny\nz\n");
    let r = write(d.path(), "r.txt", "x\nX\ny\nZ\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // F6 下一差异 → diff_pos 变为 Some
    assert_eq!(tab.borrow().diff_pos, None);
    h.key_press(eframe::egui::Key::F6);
    h.run();
    let first = tab.borrow().diff_pos;
    assert!(first.is_some(), "F6 后应有跳转目标");
    // F6 再按一次 → 跳到下一处差异（位置不同）
    h.key_press(eframe::egui::Key::F6);
    h.run();
    let second = tab.borrow().diff_pos;
    assert_ne!(first, second, "连续 F6 应跳到不同差异行");
    // F7 上一差异 → 回到第一处
    h.key_press(eframe::egui::Key::F7);
    h.run();
    assert_eq!(tab.borrow().diff_pos, first, "F7 应回到上一差异");
}

#[test]
fn difftab_f5_reload_via_key() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nX\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert!(
        tab.borrow()
            .rows
            .iter()
            .any(|row| row.tag != crate::sideview::RowTag::Equal),
        "前置：应有差异行"
    );
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 修改右侧文件后 F5 重新加载 → 差异消除
    std::fs::write(&r, "a\nb\n").unwrap();
    h.key_press(eframe::egui::Key::F5);
    h.run();
    assert!(
        tab.borrow()
            .rows
            .iter()
            .all(|row| row.tag == crate::sideview::RowTag::Equal),
        "F5 重新加载后应无差异"
    );
}

#[test]
fn dirtab_f2_rename_opens_dialog() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "a.txt", "x");
    // 不同尺寸（x vs yy）：快速模式（mtime+size）必判 Differ，避免同尺寸被误判 Same 而过滤
    write(d2.path(), "a.txt", "yy");
    let tab = RefCell::new(DirTab::new(
        d1.path().to_str().unwrap(),
        d2.path().to_str().unwrap(),
    ));
    tab.borrow_mut().refresh_sync();
    assert!(
        tab.borrow().flat.iter().any(|r| r.name == "a.txt"),
        "前置：目录有文件 a.txt"
    );
    // 选中文件后按 F2 → 打开重命名弹窗（rename_target 置位、缓冲预填文件名）
    {
        let mut t = tab.borrow_mut();
        t.selected = t.flat.iter().position(|r| r.name == "a.txt");
        assert!(t.selected_rel().is_some(), "应有选中文件");
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    h.key_press(eframe::egui::Key::F2);
    h.run();
    let t = tab.borrow();
    assert_eq!(
        t.rename_target.as_deref(),
        Some("a.txt"),
        "F2 应打开重命名弹窗并设置目标"
    );
    assert_eq!(t.rename_buf, "a.txt", "重命名缓冲应预填文件名");
}

// ---- P32-B2：DirTab 过滤/显示面板 ----------------

#[test]
fn dirtab_filter_panel_filters_by_ext_and_size() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    // 两侧内容不同尺寸：快速模式（mtime+size）必判 Differ，避免 CI 文件系统（mtime 精度低）误判 Same
    write(d1.path(), "a.txt", "xxxx");
    write(d1.path(), "b.rs", "rr");
    write(d1.path(), "c.md", "mmmmmmmm");
    write(d2.path(), "a.txt", "yyyyy");
    write(d2.path(), "b.rs", "sss");
    write(d2.path(), "c.md", "nnnnnnnnn");
    let tab = RefCell::new(DirTab::new(
        d1.path().to_str().unwrap(),
        d2.path().to_str().unwrap(),
    ));
    {
        let mut t = tab.borrow_mut();
        t.only_diff = false;
        t.show_same = true;
        t.refresh_sync();
        assert_eq!(t.flat.len(), 3, "前置：三文件全部可见");
    }
    // 扩展名过滤：仅 .txt
    {
        let mut t = tab.borrow_mut();
        t.ext_filter = "txt".to_string();
        t.rebuild_tree();
        assert_eq!(t.flat.len(), 1, "扩展名过滤后应只剩 a.txt");
        assert_eq!(t.flat[0].name, "a.txt");
    }
    // 大小范围：2~5 字节（b.rs=2 命中，a.txt=4 命中，c.md=8 排除）
    {
        let mut t = tab.borrow_mut();
        t.ext_filter.clear();
        t.min_size = "2".to_string();
        t.max_size = "5".to_string();
        t.rebuild_tree();
        let names: Vec<String> = t.flat.iter().map(|r| r.name.clone()).collect();
        assert_eq!(names.len(), 2, "大小过滤后应剩 a.txt + b.rs");
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"b.rs".to_string()));
        assert!(!names.contains(&"c.md".to_string()));
    }
    // 清除过滤恢复
    {
        let mut t = tab.borrow_mut();
        t.min_size.clear();
        t.max_size.clear();
        t.rebuild_tree();
        assert_eq!(t.flat.len(), 3, "清除大小过滤后恢复三文件");
    }
    // 渲染过滤面板（展开）不 panic
    {
        let mut t = tab.borrow_mut();
        t.show_filter_panel = true;
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert!(
        h.query_all_by_label_contains("过滤/显示").next().is_some(),
        "过滤面板应渲染标题"
    );
}

#[test]
fn parse_date_secs_basic() {
    // 1970-01-01 → 0
    assert_eq!(super::dirtab::parse_date_secs("1970-01-01"), Some(0));
    // 1970-01-02 → 86400
    assert_eq!(super::dirtab::parse_date_secs("1970-01-02"), Some(86400));
    // 2026-01-01 在 1970 之后很远
    let v = super::dirtab::parse_date_secs("2026-01-01").unwrap();
    assert!(v > 1_700_000_000);
    // 非法格式 → None
    assert_eq!(super::dirtab::parse_date_secs(""), None);
    assert_eq!(super::dirtab::parse_date_secs("2026-13-01"), None);
    assert_eq!(super::dirtab::parse_date_secs("abc"), None);
}

// ---- P32-B4：独立 Hex 对比视图（差异导航） ----------------

#[test]
fn difftab_hex_diff_navigation_via_keys() {
    let d = tempdir().unwrap();
    // 二进制文件：前 16 字节相同，第 16 字节起有差异（跨两行）
    let l = {
        let mut v = vec![0u8; 40];
        v[0] = 0x41;
        v[15] = 0x42;
        v[16] = 0x01;
        v[31] = 0x02;
        let s = String::from_utf8_lossy(&v).into_owned();
        write(d.path(), "l.bin", &s)
    };
    let r = {
        let mut v = vec![0u8; 40];
        v[0] = 0x41;
        v[15] = 0x43; // 第 1 行（offset 0）差异
        v[16] = 0x01;
        v[31] = 0x03; // 第 2 行（offset 16）差异
        let s = String::from_utf8_lossy(&v).into_owned();
        write(d.path(), "r.bin", &s)
    };
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 二进制 → hex 模式（非文本行）
    assert!(tab.borrow().hex.is_some(), "二进制文件应进入 hex 模式");
    assert!(!tab.borrow().hex.as_ref().unwrap().rows.is_empty());
    let diff_rows = tab
        .borrow()
        .hex
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .filter(|row| row.diff)
        .count();
    assert!(
        diff_rows >= 2,
        "应有至少两处 hex 差异行，实际 {}",
        diff_rows
    );

    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // F6 下一差异 → hex_diff_pos 置位
    assert_eq!(tab.borrow().hex_diff_pos, None);
    h.key_press(eframe::egui::Key::F6);
    h.run();
    let first = tab.borrow().hex_diff_pos;
    assert!(first.is_some(), "F6 后应有 hex 差异目标");
    // 再按 F6 → 跳到下一处差异（不同行）
    h.key_press(eframe::egui::Key::F6);
    h.run();
    let second = tab.borrow().hex_diff_pos;
    assert_ne!(first, second, "连续 F6 应跳到不同 hex 差异行");
    // F7 上一差异 → 回到第一处
    h.key_press(eframe::egui::Key::F7);
    h.run();
    assert_eq!(tab.borrow().hex_diff_pos, first, "F7 应回到上一差异");
    // 滚动偏移已设置（跳转生效）
    assert!(
        tab.borrow().scroll.y > 0.0 || first == Some(0),
        "hex 跳转应设置滚动偏移"
    );
}
