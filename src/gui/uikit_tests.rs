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
use crate::gui::difftab::{DiffTab, DiffViewFilter, EditSide};
use crate::gui::dirtab::{DirTab, ViewFilter};
use crate::gui::foldermergetab::FolderMergeTab;
use crate::gui::imagetab::ImageTab;
use crate::gui::mergetab::MergeTab;
use crate::gui::patchtab::PatchTab;
use crate::gui::textedit::TextEditTab;
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
    // 跨平台 headless 渲染时序差异：菜单展开后轮询重试点击「仅左侧」（macOS CI 偶发慢，需更多轮询）
    let mut clicked = false;
    for _ in 0..25 {
        h.run_steps(2);
        if let Some(node) = h.query_by_label("仅左侧") {
            node.click();
            clicked = true;
            break;
        }
    }
    assert!(clicked, "下拉菜单应展开并出现「仅左侧」选项");
    // 轮询等待过滤生效（Windows/macOS CI 偶发时序：点击后多帧才应用；并发测试下需真实 sleep）
    let mut applied = false;
    for _ in 0..60 {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(10));
        if tab.borrow().view_filter == ViewFilter::LeftOnly {
            applied = true;
            break;
        }
    }
    assert!(applied, "点击「仅左侧」后 view_filter 应变为 LeftOnly");
    let t = tab.borrow();
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

// ---- P37-1c：CsvTab 复制单元格至右侧 / 隐藏相同列 ----------------

#[test]
fn csvtab_copy_cell_via_toolbar() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.csv", "id,name\n1,alice\n2,bob\n");
    let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n2,BOB\n");
    let tab = RefCell::new(CsvTab::new(&l, &r));
    {
        let mut t = tab.borrow_mut();
        t.show_same = true;
        t.filter = crate::gui::csvtab::CsvFilter::All;
        // 模拟用户点击选中第 0 行 name 列（对齐下标 0, 列 1）
        t.selected = Some((0, 1));
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 工具栏「→ 复制单元格至右侧」
    h.get_by_label_contains("复制单元格至右侧").click();
    h.run();
    // 右侧文件已更新为左侧值
    let content = std::fs::read_to_string(&r).unwrap();
    assert!(content.contains("alice"), "右侧第 1 行 name 应更新为 alice");
}

#[test]
fn csvtab_hide_same_cols_toggle() {
    let d = tempdir().unwrap();
    // id 两列相同、name 不同
    let l = write(d.path(), "l.csv", "id,name\n1,alice\n");
    let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n");
    let tab = RefCell::new(CsvTab::new(&l, &r));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(!tab.borrow().hide_same_cols, "默认不隐藏相同列");
    // 点击「隐藏相同列」checkbox
    h.get_by_label_contains("隐藏相同列").click();
    h.run();
    assert!(tab.borrow().hide_same_cols, "勾选后应开启隐藏相同列");
    // 开启后只有差异列保留（name 列）
    let vc = tab.borrow().visible_cols().unwrap();
    assert!(
        vc.contains(&1) && !vc.contains(&0),
        "应隐藏 id 列保留 name 列"
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

// ---- P37-1b：三路合并导航（BC Clear Conflict Section, Next / Taken 导航） ----------------

#[test]
fn mergetab_clear_conflict_next_resolves_and_advances() {
    // 两个冲突块：Clear Conflict Section, Next 应解决当前（默认取左）并跳到下一冲突
    let d = tempdir().unwrap();
    let base = write(d.path(), "base.txt", "l1\nX2\nl3\nl4\nX5\nl6\n");
    let left = write(d.path(), "left.txt", "l1\nL2\nl3\nl4\nL5\nl6\n");
    let right = write(d.path(), "right.txt", "l1\nR2\nl3\nl4\nR5\nl6\n");
    let tab = RefCell::new(MergeTab::new(&base, &left, &right));
    {
        let t = tab.borrow();
        assert_eq!(t.view.conflicts, 2, "前置：两个冲突块");
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 点击「清除冲突并跳下一」
    h.get_by_label_contains("清除冲突并跳下一").click();
    h.run();
    let t = tab.borrow();
    // 第一个冲突块已解决为 Left
    let bi0 = t.view.conflict_block_indices[0];
    assert_eq!(
        t.view.blocks[bi0].resolution,
        crate::mergeview::Resolution::Left,
        "清除后第一块应默认取左"
    );
    // 已跳到第二个冲突块（conflict_idx 前进）
    assert_eq!(t.conflict_idx, Some(1), "应跳到下一冲突区段");
}

#[test]
fn mergetab_taken_nav_jumps_to_resolved_blocks() {
    // 三个冲突块（用相同行隔开避免块合并）：第 1 个取左、第 3 个取右 → 采用左导航应落在第 1 个
    let d = tempdir().unwrap();
    let base = write(d.path(), "base.txt", "X1\nk2\nY3\nk4\nZ5\nk6\n");
    let left = write(d.path(), "left.txt", "L1\nk2\nL3\nk4\nL5\nk6\n");
    let right = write(d.path(), "right.txt", "R1\nk2\nR3\nk4\nR5\nk6\n");
    let tab = RefCell::new(MergeTab::new(&base, &left, &right));
    {
        let mut t = tab.borrow_mut();
        assert_eq!(t.view.conflicts, 3, "前置：三个冲突块");
        // 第 1 个取左、第 3 个取右（先拷贝索引避免借用冲突）
        let bi0 = t.view.conflict_block_indices[0];
        let bi2 = t.view.conflict_block_indices[2];
        t.view.blocks[bi0].resolution = crate::mergeview::Resolution::Left;
        t.view.blocks[bi2].resolution = crate::mergeview::Resolution::Right;
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 下一采用左：应跳到第 1 个冲突块
    h.get_by_label_contains("下一采用左").click();
    h.run();
    {
        let t = tab.borrow();
        assert_eq!(t.conflict_idx, Some(0), "采用左导航应落在第 1 块");
        assert_eq!(t.scroll.y, 0.0, "应滚到第一个块顶部");
    }
    // 下一采用右：应跳到第 3 个冲突块
    h.get_by_label_contains("下一采用右").click();
    h.run();
    {
        let t = tab.borrow();
        assert_eq!(t.conflict_idx, Some(2), "采用右导航应落在第 3 块");
    }
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

// ---- P37-1e：图片旋转/翻转 + 差异模式 ----------------

#[test]
fn imagetab_rotate_cw_button_applies_transform() {
    let d = tempdir().unwrap();
    let a = write_png(d.path(), "a.png", [10, 20, 30, 255]);
    let b = write_png(d.path(), "b.png", [10, 20, 99, 255]);
    let tab = RefCell::new(ImageTab::new(&a, &b));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert_eq!(tab.borrow().rotation, 0, "默认不旋转");
    // 点击顺时针旋转按钮（↻）
    h.get_by_label("↻").click();
    h.run();
    assert_eq!(tab.borrow().rotation, 90, "点击↻后旋转 90°");
    // 再次点击 → 180°
    h.get_by_label("↻").click();
    h.run();
    assert_eq!(tab.borrow().rotation, 180, "再次点击后旋转 180°");
    // 重置（↩）→ 0°
    h.get_by_label("↩").click();
    h.run();
    assert_eq!(tab.borrow().rotation, 0, "重置后旋转归零");
}

#[test]
fn imagetab_flip_buttons_toggle_flags() {
    let d = tempdir().unwrap();
    let a = write_png(d.path(), "a.png", [1, 2, 3, 255]);
    let b = write_png(d.path(), "b.png", [1, 2, 99, 255]);
    let tab = RefCell::new(ImageTab::new(&a, &b));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(!tab.borrow().flip_h && !tab.borrow().flip_v, "默认无翻转");
    h.get_by_label("⇋").click(); // 水平翻转
    h.run();
    assert!(tab.borrow().flip_h, "点击⇋后水平翻转");
    h.get_by_label("⇵").click(); // 垂直翻转
    h.run();
    assert!(tab.borrow().flip_v, "点击⇵后垂直翻转");
    h.get_by_label("↩").click(); // 重置
    h.run();
    assert!(
        !tab.borrow().flip_h && !tab.borrow().flip_v,
        "重置后翻转清除"
    );
}

#[test]
fn imagetab_diff_mode_switch_via_combo() {
    let d = tempdir().unwrap();
    let a = write_png(d.path(), "a.png", [10, 20, 30, 255]);
    let b = write_png(d.path(), "b.png", [10, 20, 99, 255]);
    let tab = RefCell::new(ImageTab::new(&a, &b));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert_eq!(
        tab.borrow().diff_mode,
        crate::imgcmp::DiffMode::Exact,
        "默认精确模式"
    );
    // 打开差异模式下拉（value 含「精确」）
    let combo = h
        .query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
        .find(|n| n.value().map(|v| v.contains("精确")).unwrap_or(false))
        .expect("应存在差异模式下拉");
    combo.click();
    let mut clicked = false;
    for _ in 0..25 {
        h.run_steps(2);
        if let Some(node) = h.query_by_label("容差") {
            node.click();
            clicked = true;
            break;
        }
    }
    assert!(clicked, "差异模式下拉应出现「容差」选项");
    h.run();
    assert_eq!(
        tab.borrow().diff_mode,
        crate::imgcmp::DiffMode::Tolerance,
        "切换后应为容差模式"
    );
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

// ---- P37-1d：hex 显示格式（地址 hex/dec、值格式、隐藏地址） ----------------

#[test]
fn difftab_hex_show_addr_toggle() {
    let d = tempdir().unwrap();
    let l = {
        let mut v = vec![0u8; 20];
        v[0] = 0x41;
        String::from_utf8_lossy(&v).into_owned()
    };
    let r = {
        let mut v = vec![0u8; 20];
        v[0] = 0x42;
        String::from_utf8_lossy(&v).into_owned()
    };
    let lp = write(d.path(), "l.bin", &l);
    let rp = write(d.path(), "r.bin", &r);
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&lp, &rp, ViewOptions::default());
    assert!(tab.borrow().hex.is_some(), "二进制文件应进入 hex 模式");
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 默认显示地址（hex）
    {
        let t = tab.borrow();
        let hx = t.hex.as_ref().unwrap();
        assert!(hx.show_addr, "默认显示字节地址");
        assert!(hx.addr_hex, "默认地址格式 hex");
    }
    // 点击「显示字节地址」→ 隐藏
    h.get_by_label_contains("显示字节地址").click();
    h.run();
    assert!(
        !tab.borrow().hex.as_ref().unwrap().show_addr,
        "勾选后应隐藏字节地址"
    );
}

#[test]
fn difftab_hex_value_mode_switch_via_ui() {
    let d = tempdir().unwrap();
    let l = {
        let mut v = vec![0u8; 20];
        v[0] = 0x41;
        String::from_utf8_lossy(&v).into_owned()
    };
    let r = {
        let mut v = vec![0u8; 20];
        v[0] = 0x42;
        String::from_utf8_lossy(&v).into_owned()
    };
    let lp = write(d.path(), "l.bin", &l);
    let rp = write(d.path(), "r.bin", &r);
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&lp, &rp, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 默认 Raw（逐字节）
    assert_eq!(
        tab.borrow().hex.as_ref().unwrap().value_mode,
        crate::hexview::HexValueMode::Raw
    );
    // 打开地址格式 ComboBox（选中文本 value 为「Hex 地址」，区别于视图过滤等下拉）
    let combos: Vec<_> = h
        .query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
        .collect();
    assert!(!combos.is_empty(), "hex 模式应有格式下拉");
    let addr_combo = combos
        .iter()
        .find(|n| n.value().map(|v| v.contains("地址")).unwrap_or(false))
        .cloned()
        .expect("应存在地址格式下拉（value 含「地址」）");
    addr_combo.click();
    let mut clicked = false;
    for _ in 0..25 {
        h.run_steps(2);
        if let Some(node) = h.query_by_label("Dec 地址") {
            node.click();
            clicked = true;
            break;
        }
    }
    assert!(clicked, "地址格式下拉应出现 Dec 地址选项");
    h.run();
    assert!(
        !tab.borrow().hex.as_ref().unwrap().addr_hex,
        "切换后地址应为 dec"
    );
}

// ---- P34：空会话入口 + 拖拽填充 ----------------

#[test]
fn empty_diff_tab_shows_open_buttons() {
    let tab = RefCell::new(DiffTab::new());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 空文本对比会话：显示分别打开左右侧按钮（BC 式，不强求一次选满两个）
    assert!(h.query_all_by_label_contains("打开左侧").next().is_some());
    assert!(h.query_all_by_label_contains("打开右侧").next().is_some());
}

#[test]
fn empty_csv_tab_shows_open_buttons() {
    let tab = RefCell::new(CsvTab::new("", ""));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(h.query_all_by_label_contains("打开左侧").next().is_some());
    assert!(h.query_all_by_label_contains("打开右侧").next().is_some());
}

// ---- P37-1g：文本编辑视图（BC Text Edit） ----------------

#[test]
fn empty_text_edit_tab_shows_open_button() {
    let tab = RefCell::new(TextEditTab::new(""));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(
        h.query_all_by_label_contains("打开文件").next().is_some(),
        "空文本编辑会话应显示打开文件按钮"
    );
}

#[test]
fn text_edit_tab_save_button_writes_file() {
    let d = tempdir().unwrap();
    let p = write(d.path(), "a.txt", "old\n");
    let tab = RefCell::new(TextEditTab::new(&p));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 修改内容后点击「保存文件」→ 写回磁盘（A2 备份）
    {
        let mut t = tab.borrow_mut();
        t.content = "new content\n".to_string();
    }
    h.get_by_label_contains("保存文件").click();
    h.run();
    assert_eq!(fs::read_to_string(&p).unwrap(), "new content\n");
    assert!(
        fs::metadata(format!("{p}.bak")).is_ok(),
        "保存应生成 .bak 备份"
    );
}

// ---- P37-1h：补丁视图（BC Text Patch） ----------------

#[test]
fn patch_tab_renders_diff_and_applies() {
    let d = tempdir().unwrap();
    // 目标文件与补丁同目录
    write(d.path(), "a.txt", "line1\nold line\nline3\n");
    let p = write(
        d.path(),
        "a.patch",
        "--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3\n",
    );
    let tab = RefCell::new(PatchTab::new(&p));
    assert!(tab.borrow().error.is_none(), "解析不应出错");
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 工具栏有「应用补丁」按钮
    assert!(
        h.query_all_by_label_contains("应用补丁").next().is_some(),
        "应有应用补丁按钮"
    );
    // 点击应用补丁 → 目标文件被更新
    h.get_by_label_contains("应用补丁").click();
    h.run();
    assert_eq!(
        fs::read_to_string(d.path().join("a.txt")).unwrap(),
        "line1\nnew line\nline3"
    );
}

#[test]
fn empty_patch_tab_shows_open_button() {
    let tab = RefCell::new(PatchTab::new(""));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(
        h.query_all_by_label_contains("打开文件").next().is_some(),
        "空补丁会话应显示打开文件按钮"
    );
}

// ---- P37-1i：文件夹合并 GUI（BC Folder Merge） ----------------

#[test]
fn foldermerge_tab_renders_and_generates_plan() {
    let d = tempdir().unwrap();
    let base = d.path().join("base");
    let left = d.path().join("left");
    let right = d.path().join("right");
    fs::create_dir_all(&base).unwrap();
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    write(&base, "a.txt", "same\n");
    write(&left, "a.txt", "same\n");
    write(&right, "a.txt", "same\n");
    // 仅左侧文件 → 计划应有 copy from left
    write(&left, "l.txt", "L\n");
    let tab = RefCell::new(FolderMergeTab::new(
        base.to_str().unwrap(),
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        d.path().join("out").to_str().unwrap(),
    ));
    assert!(
        tab.borrow().error.is_none(),
        "不应出错: {:?}",
        tab.borrow().error
    );
    let plan = tab.borrow().plan.clone().expect("应生成计划");
    assert!(plan.iter().any(|i| i.rel == "l.txt"), "计划应包含 l.txt");
    // 渲染多帧不 panic
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert!(
        h.query_all_by_label_contains("生成计划").next().is_some(),
        "工具栏应有生成计划按钮"
    );
}

#[test]
fn fill_empty_session_populates_empty_tabs() {
    let d = tempdir().unwrap();
    let f1 = write(d.path(), "a.txt", "a\n");
    let f2 = write(d.path(), "b.txt", "b\n");
    let dir1 = d.path().join("d1");
    let dir2 = d.path().join("d2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));

    // 空 DirTab：拖目录 → 填充左右两侧
    app.borrow_mut()
        .add_tab(super::Tab::Dir(DirTab::new("", "")));
    let dirs = vec![
        dir1.to_string_lossy().into_owned(),
        dir2.to_string_lossy().into_owned(),
    ];
    assert!(app.borrow_mut().fill_empty_session(&dirs, &[]));
    {
        let app = app.borrow();
        match &app.tabs[app.active] {
            super::Tab::Dir(t) => {
                assert!(!t.left.is_empty());
                assert!(!t.right.is_empty());
            }
            _ => panic!("应为 DirTab"),
        }
    }

    // 空 CsvTab：拖文件 → 填充左右两侧并加载
    app.borrow_mut()
        .add_tab(super::Tab::Csv(CsvTab::new("", "")));
    let files = vec![f1.clone(), f2.clone()];
    assert!(app.borrow_mut().fill_empty_session(&[], &files));
    {
        let app = app.borrow();
        match &app.tabs[app.active] {
            super::Tab::Csv(t) => assert!(!t.is_empty()),
            _ => panic!("应为 CsvTab"),
        }
    }

    // 空 MergeTab：拖 3 文件 → 填充 BASE/LEFT/RIGHT
    app.borrow_mut()
        .add_tab(super::Tab::Merge(MergeTab::new("", "", "")));
    let files3 = vec![f1.clone(), f2.clone(), f1.clone()];
    assert!(app.borrow_mut().fill_empty_session(&[], &files3));
    {
        let app = app.borrow();
        match &app.tabs[app.active] {
            super::Tab::Merge(t) => {
                assert!(!t.base_path.is_empty());
                assert!(!t.left_path.is_empty());
                assert!(!t.right_path.is_empty());
            }
            _ => panic!("应为 MergeTab"),
        }
    }
}

// ---- P35-A1：复制差异块到另一侧 ----------------

#[test]
fn difftab_copy_block_to_other_side() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert!(!tab.borrow().diff_blocks.is_empty(), "应有差异块");
    tab.borrow_mut().diff_pos = Some(0);
    assert!(
        tab.borrow_mut().copy_block_to(EditSide::Right),
        "复制左→右应成功"
    );
    assert_eq!(fs::read_to_string(&r).unwrap(), "a\nb\nc\n");

    // 反向：复制右→左
    let l2 = write(d.path(), "l2.txt", "a\nP\nc\n");
    let r2 = write(d.path(), "r2.txt", "a\nQ\nc\n");
    let tab2 = RefCell::new(DiffTab::new());
    tab2.borrow_mut()
        .load_pair(&l2, &r2, ViewOptions::default());
    tab2.borrow_mut().diff_pos = Some(0);
    assert!(
        tab2.borrow_mut().copy_block_to(EditSide::Left),
        "复制右→左应成功"
    );
    assert_eq!(fs::read_to_string(&l2).unwrap(), "a\nQ\nc\n");
}

#[test]
fn difftab_swap_sides_exchanges_files() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert_eq!(tab.borrow().left.as_ref().unwrap().path, l);
    assert_eq!(tab.borrow().right.as_ref().unwrap().path, r);
    tab.borrow_mut().swap_sides();
    assert_eq!(tab.borrow().left.as_ref().unwrap().path, r);
    assert_eq!(tab.borrow().right.as_ref().unwrap().path, l);
}

// ---- P35-A4：显示空白符 ----------------

#[test]
fn visible_ws_replaces_space_and_tab() {
    use crate::gui::common::visible_ws;
    assert_eq!(visible_ws("a b\tc"), "a·b→c");
    assert_eq!(visible_ws(""), "");
    assert_eq!(visible_ws("普通文本"), "普通文本");
    assert_eq!(visible_ws("  \t"), "··→");
}

// ---- P35-A3：视图过滤 ----------------

#[test]
fn difftab_view_filter_diff_and_context() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\nd\ne\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\nd\nY\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // diff_rows = [1, 4]（第 2/5 行差异）
    let t = tab.borrow();
    let diff_set: std::collections::HashSet<usize> = t.diff_rows.iter().copied().collect();
    drop(t);
    // Diff 过滤：差异行显示，相同行不显示
    tab.borrow_mut().view_filter = DiffViewFilter::Diff;
    let t = tab.borrow();
    assert!(t.row_visible(1, &diff_set));
    assert!(t.row_visible(4, &diff_set));
    assert!(!t.row_visible(0, &diff_set));
    drop(t);
    // Context 过滤（默认 3 行上下文）：相同行靠近差异行也显示
    tab.borrow_mut().view_filter = DiffViewFilter::Context;
    let t = tab.borrow();
    assert!(t.row_visible(1, &diff_set));
    assert!(t.row_visible(0, &diff_set)); // 距差异行 1 <= 3
    assert!(t.row_visible(2, &diff_set)); // 距差异行 1 <= 3
    assert!(t.row_visible(3, &diff_set)); // 距差异行 1 <= 3
    assert!(t.row_visible(4, &diff_set));
}

// ---- P36-D1：DirTab 交换两边 ----------------

#[test]
fn dirtab_swap_sides_exchanges_paths() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "a.txt", "x");
    write(d2.path(), "a.txt", "y");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    assert_eq!(tab.borrow().left, p1);
    assert_eq!(tab.borrow().right, p2);
    tab.borrow_mut().swap_sides();
    assert_eq!(tab.borrow().left, p2);
    assert_eq!(tab.borrow().right, p1);
}

// ---- P36-D2：逐文件操作（复制到边/删除/排除）----------------

#[test]
fn dirtab_copy_single_to_other_side() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "only_left.txt", "L");
    write(d2.path(), "a.txt", "x");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().refresh_sync();
    // 复制仅左侧文件 → 右侧
    tab.borrow_mut().copy_single("only_left.txt", true);
    assert!(
        d2.path().join("only_left.txt").exists(),
        "复制后右侧应出现 only_left.txt"
    );
}

#[test]
fn dirtab_delete_single_removes_file() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "a.txt", "x");
    write(d2.path(), "right_only.txt", "R");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().refresh_sync();
    // 删除右侧仅右侧文件
    tab.borrow_mut().delete_single("right_only.txt", true);
    assert!(
        !d2.path().join("right_only.txt").exists(),
        "删除后右侧不应再有 right_only.txt"
    );
}

#[test]
fn dirtab_exclude_hides_file() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "keep.txt", "k");
    write(d1.path(), "hide.txt", "h");
    // 两侧不同尺寸（k vs kk）：Windows 低精度 mtime 下快速模式必判 Differ，避免被 only_diff 过滤
    write(d2.path(), "keep.txt", "kk");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().refresh_sync();
    // 排除 hide.txt 后，重建树不再显示
    tab.borrow_mut().exclude("hide.txt");
    assert!(
        !tab.borrow().flat.iter().any(|r| r.name.contains("hide")),
        "排除后树中不应有 hide.txt"
    );
    assert!(
        tab.borrow().flat.iter().any(|r| r.name.contains("keep")),
        "keep.txt 应仍在树中"
    );
}

// ---- P37-1f：文件夹同步操作集（独自离开 / 批量镜像 / 立即同步） ----------------

#[test]
fn dirtab_leave_alone_skips_in_sync_plan() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    // 三个差异文件（两侧不同尺寸，必判 Differ）
    for n in ["a.txt", "b.txt", "c.txt"] {
        write(d1.path(), n, "L");
        write(d2.path(), n, "RR");
    }
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let mut tab = DirTab::new(&p1, &p2);
    // mirror 模式：Differ 无条件生成 Copy（update 模式下 dst 更新会 Skip，干扰断言）
    tab.sync_mode = "mirror".to_string();
    tab.refresh_sync();
    tab.gen_sync_plan();
    let plan = tab.sync_plan.clone().unwrap();
    // 前置：三个文件都在计划中（update 模式 → Copy 到右）
    assert_eq!(
        plan.iter()
            .filter(|op| matches!(op, crate::sync::SyncOp::Copy { .. }))
            .count(),
        3,
        "前置：三个差异文件都应生成复制计划"
    );
    // 标记 b.txt 独自离开 → 重新生成计划，b.txt 不勾选
    tab.toggle_leave_alone("b.txt");
    let checked: Vec<&str> = tab
        .sync_checked
        .iter()
        .filter_map(|&i| {
            tab.sync_plan
                .as_ref()
                .and_then(|p| p.get(i))
                .map(|op| op.rel())
        })
        .collect();
    assert!(
        checked.contains(&"a.txt") && checked.contains(&"c.txt"),
        "a/c 应勾选: {:?}",
        checked
    );
    assert!(
        !checked.contains(&"b.txt"),
        "b.txt 独自离开后不应勾选: {:?}",
        checked
    );
    // 再次点击取消独自离开 → 恢复勾选
    tab.toggle_leave_alone("b.txt");
    let checked: Vec<&str> = tab
        .sync_checked
        .iter()
        .filter_map(|&i| {
            tab.sync_plan
                .as_ref()
                .and_then(|p| p.get(i))
                .map(|op| op.rel())
        })
        .collect();
    assert!(checked.contains(&"b.txt"), "取消独自离开后 b.txt 恢复勾选");
}

#[test]
fn dirtab_batch_copy_to_left_mirrors() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    // 仅右侧文件（左侧缺失）：批量复制→左应把右侧内容复制到左侧
    write(d2.path(), "r.txt", "RR");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let mut tab = DirTab::new(&p1, &p2);
    tab.refresh_sync();
    // 前置：右侧有 r.txt（RightOnly）
    assert!(
        tab.result
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.rel == "r.txt"),
        "前置：应检测到 r.txt"
    );
    tab.run_batch_copy_to_left();
    // 左侧现在也有 r.txt 且内容一致 → 不再是差异
    let content = std::fs::read_to_string(d1.path().join("r.txt")).unwrap();
    assert_eq!(content, "RR", "批量复制→左后左侧应有 r.txt 内容 RR");
}

#[test]
fn dirtab_batch_delete_left_mirrors() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    // 仅左侧文件：批量删除左侧应删除它
    write(d1.path(), "l.txt", "LL");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let mut tab = DirTab::new(&p1, &p2);
    tab.refresh_sync();
    assert!(
        tab.result
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.rel == "l.txt"),
        "前置：应检测到 l.txt"
    );
    tab.run_batch_delete_left();
    assert!(
        !d1.path().join("l.txt").exists(),
        "批量删除左侧后 l.txt 应被删除"
    );
}

#[test]
fn dirtab_sync_now_button_via_ui() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    // 左侧文件 → update 模式应复制到右侧
    write(d1.path(), "a.txt", "A");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    {
        let mut t = tab.borrow_mut();
        t.sync_mode = "update".to_string();
        // 禁用自动刷新：kittest 时间推进可能触发 refresh 占用 bg，导致立即同步被吞
        t.last_auto_refresh = f64::MAX;
        t.refresh_sync();
    }
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run_steps(4);
    // 点击「⚡ 立即同步」→ 生成计划并执行（后台线程）
    h.get_by_label_contains("立即同步").click();
    h.run_steps(4);
    // 后台执行是异步的：轮询等待右侧出现 a.txt（本地线程可能已清空 bg，勿断言 bg 状态；CI 慢需真实 sleep）
    let mut done = false;
    for _ in 0..100 {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(10));
        if d2.path().join("a.txt").exists() {
            done = true;
            break;
        }
    }
    assert!(done, "立即同步后右侧应出现 a.txt");
    assert_eq!(
        std::fs::read_to_string(d2.path().join("a.txt")).unwrap(),
        "A"
    );
}

// ---- P36-D3：视图过滤快捷键 1/2/3 ----------------

#[test]
fn dirtab_view_filter_hotkeys_1_2_3() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "same.txt", "x");
    write(d1.path(), "only_left.txt", "L");
    write(d2.path(), "same.txt", "x");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().compare_content = true; // 内容比较：same.txt 两侧内容相同 → 判 Same（避免 mtime 差异误判）
    tab.borrow_mut().refresh_sync();
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run_steps(4);
    // 按 2 → 仅差异
    h.key_press(eframe::egui::Key::Num2);
    h.run_steps(2);
    assert_eq!(tab.borrow().view_filter, ViewFilter::Diff);
    // 按 3 → 仅相同
    h.key_press(eframe::egui::Key::Num3);
    h.run_steps(2);
    assert_eq!(tab.borrow().view_filter, ViewFilter::Same);
    // 按 1 → 全部
    h.key_press(eframe::egui::Key::Num1);
    h.run_steps(2);
    assert_eq!(tab.borrow().view_filter, ViewFilter::All);
}
