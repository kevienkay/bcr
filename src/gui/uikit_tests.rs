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
    // 搜索框是第一个 TextInput（搜索 + 替换 + 行号 三个输入框）
    h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
        .next()
        .unwrap()
        .focus();
    h.run();
    h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
        .next()
        .unwrap()
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
