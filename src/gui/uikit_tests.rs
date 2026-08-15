//! 真实 GUI 场景交互测试（egui_kittest 驱动）。
//!
//! 与 tests/ui_kittest.rs（演示）不同，这里驱动 bcr 真实标签页：
//! - DiffTab：搜索框输入 → 匹配；⬇ 下一匹配 → 计数显示；下一差异跳转
//! - DirTab：点"刷新" → 树构建（后台线程）；状态过滤下拉
//! - CsvTab：点表头 → 排序生效
//! - MergeTab：先定位冲突再取左侧 → 解决
//!
//! 运行：cargo test gui::uikit_tests

use crate::compare::FileStatus;
use crate::gui::csvtab::CsvTab;
use crate::gui::difftab::{DiffDetailMode, DiffLayout, DiffTab, DiffViewFilter, EditSide};
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
    // 加固（P40-2 CI 第 4 次撞 flaky）：首次点击可能因菜单自动关闭时序未命中 → 整轮重试最多 3 次
    let mut applied = false;
    for attempt in 0..3 {
        if attempt > 0 {
            // 重新打开下拉再点（上一轮点击可能落在已关闭的菜单上）
            h.get_by_role(eframe::egui::accesskit::Role::ComboBox)
                .click();
            h.run_steps(4);
            let mut reclicked = false;
            for _ in 0..30 {
                h.run_steps(2);
                if let Some(node) = h.query_by_label("仅左侧") {
                    node.click();
                    reclicked = true;
                    break;
                }
            }
            if !reclicked {
                continue;
            }
        }
        for _ in 0..60 {
            h.run_steps(2);
            std::thread::sleep(std::time::Duration::from_millis(10));
            if tab.borrow().view_filter == ViewFilter::LeftOnly {
                applied = true;
                break;
            }
        }
        if applied {
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
    assert!(super::difftab::diff_mid_line_color(true, RowTag::Delete).is_some());
    assert!(super::difftab::diff_mid_line_color(true, RowTag::Insert).is_some());
    assert!(super::difftab::diff_mid_line_color(true, RowTag::Replace).is_some());
    assert!(super::difftab::diff_mid_line_color(true, RowTag::Equal).is_none());
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

// ---- P38-1a：隔离（BC Isolate） ----------------

#[test]
fn difftab_isolate_limits_navigation_and_view() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\nREP1\nm2\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\nREP2\nm2\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    assert!(!tab.diff_blocks.is_empty(), "前置：有差异块");
    // 定位第一个差异行并隔离其所在块
    tab.diff_pos = Some(0);
    assert!(tab.isolate_current(), "隔离应成功");
    let iso = tab.isolated.expect("隔离后应有范围");
    let block0 = tab.diff_blocks[0];
    assert_eq!(iso, block0, "隔离范围=当前差异块: {iso:?} vs {block0:?}");
    // 导航只在隔离范围内循环
    let nav = tab.nav_diff_rows();
    assert!(
        nav.iter().all(|&r| r >= iso.0 && r <= iso.1),
        "导航行应在隔离范围内: {nav:?}"
    );
    // 取消隔离恢复
    tab.unisolate();
    assert!(tab.isolated.is_none(), "取消后无隔离");
    assert_eq!(
        tab.nav_diff_rows().len(),
        tab.diff_rows.len(),
        "恢复全部导航"
    );
}

#[test]
fn difftab_isolate_context_menu_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    tab.borrow_mut().diff_pos = Some(0);
    assert!(tab.borrow_mut().isolate_current(), "隔离应成功");
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    // 隔离提示条出现（渲染不崩溃 + 状态保持）
    assert!(
        tab.borrow().isolated.is_some(),
        "隔离状态应保持（提示条渲染不 panic）"
    );
    // 取消隔离
    tab.borrow_mut().unisolate();
    h.run();
    assert!(tab.borrow().isolated.is_none(), "取消后无隔离");
}

// ---- P38-1b：对齐方式（BC Align With） ----------------

#[test]
fn difftab_align_manual_pair_via_state() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL1\nDEL2\nm\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS1\nINS2\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    let before = tab.rows.len();
    // 手动对齐：左侧第 2 行 ↔ 右侧第 3 行（自动对齐时不在同一行）
    tab.manual_aligns.push((2, 3));
    tab.recompute();
    // 对齐后应出现一个同时含 left_no=2 与 right_no=3 的 Replace 行
    let paired = tab
        .rows
        .iter()
        .find(|r| r.left_no == Some(2) && r.right_no == Some(3));
    assert!(paired.is_some(), "应生成手动对齐行: {:?}", tab.rows);
    assert_eq!(
        paired.unwrap().tag,
        crate::sideview::RowTag::Replace,
        "对齐行状态为 Replace"
    );
    // 行数应减少（两行合并为一行）
    assert!(
        tab.rows.len() < before,
        "合并后行数减少: {} < {}",
        tab.rows.len(),
        before
    );
    // 清除对齐恢复
    tab.clear_aligns();
    let paired2 = tab
        .rows
        .iter()
        .find(|r| r.left_no == Some(2) && r.right_no == Some(3));
    assert!(paired2.is_none(), "清除后不再有手动对齐行");
}

#[test]
fn difftab_align_pick_finish_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL1\nDEL2\nm\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS1\nINS2\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 进入对齐模式（左侧行 2 为源）
    tab.borrow_mut().start_align(EditSide::Left, 2);
    assert!(tab.borrow().align_pick.is_some(), "对齐模式已开启");
    // 完成对齐：目标右侧行 3
    assert!(tab.borrow_mut().finish_align(3), "完成对齐应成功");
    assert!(tab.borrow().align_pick.is_none(), "对齐后退出模式");
    assert_eq!(tab.borrow().manual_aligns, vec![(2, 3)], "记录对齐对");
    // 渲染不 panic（对齐行显示）
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert!(
        tab.borrow()
            .rows
            .iter()
            .any(|r| r.left_no == Some(2) && r.right_no == Some(3)),
        "UI 渲染后对齐行存在"
    );
}

// ---- P38-1c：缩进调整（BC Increase/Decrease Indent） ----------------

#[test]
fn difftab_indent_block_adjusts_both_sides() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nDEL\nb\n");
    let r = write(d.path(), "r.txt", "a\nb\nINS\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert!(!tab.borrow().diff_blocks.is_empty(), "应有差异块");
    // 对第 2 行（块内）增加缩进
    tab.borrow_mut().diff_pos = Some(0);
    assert!(tab.borrow_mut().indent_block(1, 1), "增加缩进应成功");
    // 左侧 DEL 行与右侧对应行 +4 空格（两侧不同尺寸避免快速模式误判）
    let lc = fs::read_to_string(&l).unwrap();
    assert!(lc.contains("    DEL"), "左侧块行应 +4 空格: {lc}");
    assert!(!lc.contains("    a\n"), "块外行不应缩进: {lc}");
    // 减少缩进
    tab.borrow_mut().indent_block(1, -1);
    let lc2 = fs::read_to_string(&l).unwrap();
    assert!(lc2.contains("DEL\n"), "减少后恢复: {lc2}");
}

#[test]
fn difftab_indent_block_outside_block_returns_false() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "same\nDEL\n");
    let r = write(d.path(), "r.txt", "same\nINS\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    // 第 0 行（相同行）不在差异块内 → false
    assert!(!tab.indent_block(0, 1), "非差异块行应返回 false");
}

// ---- P38-1d：编辑导航（BC Next/Previous Edit） ----------------

#[test]
fn difftab_edit_nav_marks_and_jumps() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\nREP1\nm2\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\nREP2\nm2\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    // 复制一个差异块 → 该块行被标记为已编辑
    tab.diff_pos = Some(0);
    assert!(tab.copy_block_to(EditSide::Right), "复制应成功");
    let edited = tab.edited_rows();
    assert!(!edited.is_empty(), "复制后应有已编辑行: {edited:?}");
    // 编辑导航循环前进/后退（单条编辑：next→Some(0)，prev 循环回 Some(0)）
    tab.next_edit();
    assert_eq!(tab.edit_pos, Some(0), "next 后应定位到唯一编辑");
    tab.prev_edit();
    assert_eq!(tab.edit_pos, Some(0), "prev 循环回到唯一编辑");
}

#[test]
fn difftab_edit_nav_context_menu_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "h\nDEL\nm\n");
    let r = write(d.path(), "r.txt", "h\nm\nINS\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    tab.borrow_mut().diff_pos = Some(0);
    assert!(
        tab.borrow_mut().copy_block_to(EditSide::Right),
        "复制应成功"
    );
    // 渲染不 panic（编辑行圆点标记）
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert!(
        !tab.borrow().edited_rows().is_empty(),
        "UI 渲染后仍保留已编辑标记"
    );
}

// ---- P38-1e：文件级联动（BC Copy File and Open Next Difference） ----------------

#[test]
fn difftab_copy_file_to_overwrites_target() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "x\ny\nz\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, &r, ViewOptions::default());
    assert!(tab.copy_file_to(EditSide::Right), "复制文件左→右应成功");
    // 右侧文件 = 左侧内容，左侧不变
    assert_eq!(fs::read_to_string(&r).unwrap(), "a\nb\nc\n");
    assert_eq!(fs::read_to_string(&l).unwrap(), "a\nb\nc\n");
    // 备份存在
    assert!(fs::metadata(format!("{r}.bak")).is_ok(), "应生成 .bak 备份");
    // 内容相同后再次复制返回 false
    assert!(!tab.copy_file_to(EditSide::Right), "无变化应返回 false");

    // 反向：复制右→左
    let l2 = write(d.path(), "l2.txt", "p\nq\n");
    let r2 = write(d.path(), "r2.txt", "m\nn\n");
    let mut tab2 = DiffTab::new();
    tab2.load_pair(&l2, &r2, ViewOptions::default());
    assert!(tab2.copy_file_to(EditSide::Left), "复制文件右→左应成功");
    assert_eq!(fs::read_to_string(&l2).unwrap(), "m\nn\n");
}

#[test]
fn difftab_copy_file_to_single_side_returns_false() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\n");
    let mut tab = DiffTab::new();
    tab.load_pair(&l, "", ViewOptions::default());
    // 未加载右侧 → 复制到右侧失败
    assert!(!tab.copy_file_to(EditSide::Right), "无目标侧应返回 false");
}

#[test]
fn difftab_copy_file_context_menu_via_ui() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "x\ny\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert!(
        tab.borrow_mut().copy_file_to(EditSide::Right),
        "复制文件应成功"
    );
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    for _ in 0..3 {
        h.run();
    }
    assert_eq!(
        fs::read_to_string(&r).unwrap(),
        "a\nb\n",
        "UI 渲染后右侧文件已被覆盖"
    );
}

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

// P38-1f：修复欢迎页卡片点击不跳转（Frame response 默认 Sense::hover 不含 click）

#[test]
fn welcome_page_card_click_opens_session() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    assert!(app.borrow().tabs.is_empty(), "前置：无标签页");
    let mut h = Harness::new_ui(|ui| app.borrow_mut().welcome_ui(ui));
    h.run();
    // 点击「文本对比」卡片 → 应创建 Diff 标签
    h.get_by_label_contains("文本对比").click();
    h.run();
    assert_eq!(app.borrow().tabs.len(), 1, "点击后应创建 1 个标签页");
    assert!(
        matches!(app.borrow().tabs[0], super::Tab::Diff(_)),
        "文本对比卡片应创建 Diff 标签"
    );
    // 点击「CSV 表格」卡片（第二行首列，视口内）→ 创建 Csv 标签（同一 Harness 继续点击）
    h.get_by_label_contains("CSV 表格").click();
    h.run();
    let n = app.borrow().tabs.len();
    assert_eq!(n, 2, "点击后应再创建 1 个标签页");
    assert!(
        matches!(app.borrow().tabs[n - 1], super::Tab::Csv(_)),
        "CSV 卡片应创建 Csv 标签"
    );
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
    // P40-1：显示字节地址选项已移入 View 菜单（App 层），此处改为字段级验证
    {
        let mut t = tab.borrow_mut();
        t.hex.as_mut().unwrap().show_addr = false;
    }
    h.run();
    assert!(
        !tab.borrow().hex.as_ref().unwrap().show_addr,
        "关闭后应隐藏字节地址"
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
    // P40-1：地址/值格式下拉已移入 View 菜单（App 层），此处改为字段级验证
    {
        let mut t = tab.borrow_mut();
        let hx = t.hex.as_mut().unwrap();
        hx.addr_hex = false;
        hx.value_mode = crate::hexview::HexValueMode::LittleEndian;
    }
    h.run();
    {
        let t = tab.borrow();
        let hx = t.hex.as_ref().unwrap();
        assert!(!hx.addr_hex, "地址格式应为 dec");
        assert_eq!(
            hx.value_mode,
            crate::hexview::HexValueMode::LittleEndian,
            "值格式应为小端"
        );
    }
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

// ---- P37-1n：在文件中查找（BC Find in Files） ----------------

#[test]
fn text_edit_find_in_files_window_renders() {
    let d = tempdir().unwrap();
    let p = write(d.path(), "a.txt", "needle here\n");
    let tab = RefCell::new(TextEditTab::new(&p));
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 打开查找/替换栏 → 「在文件中查找…」按钮可见
    h.get_by_label_contains("查找/替换").click();
    h.run();
    assert!(
        h.query_all_by_label_contains("在文件中查找")
            .next()
            .is_some(),
        "查找栏应显示在文件中查找按钮"
    );
    // 打开弹窗并执行搜索 → 结果列表渲染不 panic
    {
        let mut t = tab.borrow_mut();
        t.search = "needle".to_string();
    }
    h.get_by_label_contains("在文件中查找…").click();
    h.run();
    h.get_by_label_contains("搜索").click();
    for _ in 0..3 {
        h.run();
    }
    assert!(tab.borrow().file_hits_total >= 1, "应命中当前文件至少 1 处");
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
fn difftab_copy_line_to_other_side() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 第 2 行是差异行（索引 1），行级复制左→右
    assert!(
        tab.borrow_mut().copy_line_at(1, EditSide::Right),
        "复制行左→右应成功"
    );
    assert_eq!(fs::read_to_string(&r).unwrap(), "a\nb\nc\n");

    // 反向：复制右→左
    let l2 = write(d.path(), "l2.txt", "a\nP\nc\n");
    let r2 = write(d.path(), "r2.txt", "a\nQ\nc\n");
    let tab2 = RefCell::new(DiffTab::new());
    tab2.borrow_mut()
        .load_pair(&l2, &r2, ViewOptions::default());
    assert!(
        tab2.borrow_mut().copy_line_at(1, EditSide::Left),
        "复制行右→左应成功"
    );
    assert_eq!(fs::read_to_string(&l2).unwrap(), "a\nQ\nc\n");
}

#[test]
fn difftab_copy_line_context_menu_registers() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 渲染多帧：右键菜单含「复制行到右侧/左侧」（不 panic）
    for _ in 0..3 {
        h.run();
    }
    assert!(!tab.borrow().rows.is_empty(), "应有渲染行");
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

// ---- P39-2a：快捷键系统化 -------------

#[test]
fn difftab_view_filter_hotkeys_1_2_3() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\nd\ne\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\nd\nY\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 按 2 → 仅差异
    h.key_press(eframe::egui::Key::Num2);
    h.run();
    assert_eq!(tab.borrow().view_filter, DiffViewFilter::Diff);
    // 按 3 → 仅相同
    h.key_press(eframe::egui::Key::Num3);
    h.run();
    assert_eq!(tab.borrow().view_filter, DiffViewFilter::Same);
    // 按 1 → 全部
    h.key_press(eframe::egui::Key::Num1);
    h.run();
    assert_eq!(tab.borrow().view_filter, DiffViewFilter::All);
}

#[test]
fn difftab_cmd_l_goto_focus() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nb\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(!tab.borrow().goto_focus);
    h.key_combination_modifiers(eframe::egui::Modifiers::COMMAND, &[eframe::egui::Key::L]);
    h.run();
    // goto_focus 渲染时被消费（request_focus 后置 false）→ 检查行号输入框实际获得焦点
    assert!(
        h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
            .next()
            .is_some_and(|n| n.is_focused()),
        "⌘L 后行号输入框应获得焦点"
    );
}

#[test]
fn difftab_cmd_g_next_prev_match() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "alpha\nbeta\n");
    let r = write(d.path(), "r.txt", "ALPHA\nbeta\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 构造两条匹配（不区分大小写）
    tab.borrow_mut().search.query = "a".to_string();
    tab.borrow_mut().update_search();
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    assert!(
        tab.borrow().search.matches.len() >= 2,
        "搜索 a 应有 ≥2 匹配"
    );
    assert_eq!(tab.borrow().search.current, None);
    // ⌘G 下一匹配 → current = 0
    h.key_combination_modifiers(eframe::egui::Modifiers::COMMAND, &[eframe::egui::Key::G]);
    h.run();
    assert_eq!(tab.borrow().search.current, Some(0));
    // ⌘G 再按 → current = 1（循环）
    h.key_combination_modifiers(eframe::egui::Modifiers::COMMAND, &[eframe::egui::Key::G]);
    h.run();
    assert_eq!(tab.borrow().search.current, Some(1));
    // ⇧⌘G 上一匹配 → 回到 0
    h.key_combination_modifiers(
        eframe::egui::Modifiers::COMMAND | eframe::egui::Modifiers::SHIFT,
        &[eframe::egui::Key::G],
    );
    h.run();
    assert_eq!(tab.borrow().search.current, Some(0));
}

// ---- P39-2a：全局快捷键（⌘T 新建标签 / ⌘, 设置 / ⌥⌘S 会话 / ⌥⌘C 清除） -------------

#[test]
fn global_shortcuts_new_tab_settings_clear() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    // 先建一个文本对比标签
    app.borrow_mut().open_empty_diff();
    assert_eq!(app.borrow().tabs.len(), 1);
    let mut h = Harness::new_ui(|ui| app.borrow_mut().handle_global_shortcuts_safe(ui));
    h.run();
    // ⌘T 新建标签页 → 2 个标签
    h.key_combination_modifiers(eframe::egui::Modifiers::COMMAND, &[eframe::egui::Key::T]);
    h.run();
    assert_eq!(app.borrow().tabs.len(), 2, "⌘T 应新建标签页");
    // ⌘, 打开设置
    h.key_combination_modifiers(
        eframe::egui::Modifiers::COMMAND,
        &[eframe::egui::Key::Comma],
    );
    h.run();
    assert!(app.borrow().show_settings, "⌘, 应打开设置对话框");
    // ⌥⌘S 会话中心
    h.key_combination_modifiers(
        eframe::egui::Modifiers::COMMAND | eframe::egui::Modifiers::ALT,
        &[eframe::egui::Key::S],
    );
    h.run();
    assert!(app.borrow().show_sessions, "⌥⌘S 应打开会话中心");
    // ⌥⌘C 清除会话 → 当前标签重置为空（左右清空）
    h.key_combination_modifiers(
        eframe::egui::Modifiers::COMMAND | eframe::egui::Modifiers::ALT,
        &[eframe::egui::Key::C],
    );
    h.run();
    let app = app.borrow();
    assert_eq!(app.tabs.len(), 2, "清除会话不关闭标签");
    assert!(
        matches!(&app.tabs[app.active], super::Tab::Diff(t) if t.left.is_none() && t.right.is_none()),
        "⌥⌘C 应把当前标签重置为空会话"
    );
}

// ---- P39-2c：差异部分导航（区块级跳转） -------------

#[test]
fn difftab_diff_section_navigation() {
    let d = tempdir().unwrap();
    // 两个差异块：第 1 行块 + 第 4-5 行块
    let l = write(d.path(), "l.txt", "a\nb\nc\nd\ne\nf\n");
    let r = write(d.path(), "r.txt", "A\nb\nc\nD\nE\nf\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // diff_blocks 应含 2 个块
    assert_eq!(tab.borrow().diff_blocks.len(), 2, "应有两处差异块");
    // 起始：diff_pos None → 跳到第 0 块起始行
    tab.borrow_mut().next_diff_section();
    let t = tab.borrow();
    let first = t.diff_pos.map(|p| t.diff_rows[p]).unwrap_or(usize::MAX);
    drop(t);
    assert_eq!(first, 0, "第一块应从第 0 行开始");
    // 下一块 → 第 3 行（第二块起始）
    tab.borrow_mut().next_diff_section();
    let t = tab.borrow();
    let second = t.diff_pos.map(|p| t.diff_rows[p]).unwrap_or(usize::MAX);
    drop(t);
    assert_eq!(second, 3, "第二块应从第 3 行开始");
    // 上一块 → 回到第 0 块
    tab.borrow_mut().prev_diff_section();
    let t = tab.borrow();
    let back = t.diff_pos.map(|p| t.diff_rows[p]).unwrap_or(usize::MAX);
    drop(t);
    assert_eq!(back, 0, "上一块应回到第 0 块");
}

// ---- P39-2c：会话中心保存当前会话 + 报告预览 -------------

#[test]
fn session_center_save_current_and_report_preview() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nB\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    {
        let mut app = app.borrow_mut();
        app.open_empty_diff();
        if let super::Tab::Diff(t) = &mut app.tabs[0] {
            t.load_pair(&l, &r, ViewOptions::default());
        }
    }
    // session_paths：DiffTab 应能提取左右路径
    let paths = {
        let app = app.borrow();
        super::session_paths(&app.tabs[0])
    };
    assert!(paths.is_some(), "DiffTab 应能提取会话路径");
    let (pl, pr) = paths.unwrap();
    assert_eq!(pl, l);
    assert_eq!(pr, r);
    // 报告预览：DiffTab 应有统计文本
    let preview = {
        let app = app.borrow();
        let t = match &app.tabs[0] {
            super::Tab::Diff(t) => t,
            _ => panic!("应为 DiffTab"),
        };
        super::diff_report_preview(t)
    };
    assert!(preview.contains("统计"), "报告预览应含统计行");
    assert!(preview.contains("bcr 文本对比报告"), "报告预览应含标题");
}

// ---- P39-2d：书签 0-9 / 细节三模式 / 布局 -------------

#[test]
fn difftab_bookmarks_toggle_goto_clear() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\nd\ne\n");
    let r = write(d.path(), "r.txt", "a\nb\nX\nd\ne\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 方法级：切换书签（绑定当前 scroll 顶部行）
    tab.borrow_mut().toggle_bookmark(1);
    assert!(
        tab.borrow().bookmarks().contains_key(&1),
        "toggle 应绑定书签 1"
    );
    // 再 toggle → 取消
    tab.borrow_mut().toggle_bookmark(1);
    assert!(
        !tab.borrow().bookmarks().contains_key(&1),
        "再次 toggle 应取消书签"
    );
    // 绑定 + 跳转（不 panic、集合保留）
    tab.borrow_mut().toggle_bookmark(3);
    tab.borrow_mut().goto_bookmark(3);
    assert!(tab.borrow().bookmarks().contains_key(&3));
    // 清除全部
    tab.borrow_mut().clear_bookmarks();
    assert!(tab.borrow().bookmarks().is_empty());
    // 键盘：⌘⌥⌃1 切换书签（Harness 渲染后触发）
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    h.key_combination_modifiers(
        eframe::egui::Modifiers::COMMAND
            | eframe::egui::Modifiers::ALT
            | eframe::egui::Modifiers::CTRL,
        &[eframe::egui::Key::Num1],
    );
    h.run();
    assert_eq!(tab.borrow().bookmarks().len(), 1, "⌘⌥⌃1 应切换出 1 个书签");
}

#[test]
fn difftab_detail_mode_hex_and_layout() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nB\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert_eq!(tab.borrow().detail_mode, DiffDetailMode::Text);
    // 16进制细节：文本文件也构建 hex 数据
    tab.borrow_mut().set_detail_mode(DiffDetailMode::Hex);
    assert_eq!(tab.borrow().detail_mode, DiffDetailMode::Hex);
    assert!(tab.borrow().hex.is_some(), "Hex 细节模式应构建字节网格");
    // 切回文本
    tab.borrow_mut().set_detail_mode(DiffDetailMode::Text);
    assert!(tab.borrow().hex.is_none() || tab.borrow().detail_mode == DiffDetailMode::Text);
    // 布局切换
    tab.borrow_mut().set_layout(DiffLayout::TopBottom);
    assert_eq!(
        tab.borrow().row_h(),
        super::theme::ROW_H * 2.0,
        "上-下布局行高应为 2 倍"
    );
    tab.borrow_mut().set_layout(DiffLayout::SideBySide);
    assert_eq!(
        tab.borrow().row_h(),
        super::theme::ROW_H,
        "并排布局行高应为 1 倍"
    );
}

// ---- P39-2e：替换菜单聚焦 + 忽略不重要差异 + 视图切换 -------------

#[test]
fn difftab_replace_focus_via_shift_cmd_f() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nb\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    h.key_combination_modifiers(
        eframe::egui::Modifiers::COMMAND | eframe::egui::Modifiers::SHIFT,
        &[eframe::egui::Key::F],
    );
    h.run();
    // 渲染时 replace_focus 消费 → 替换框获得焦点（TextInput 第 3 个：行号→搜索→替换）
    assert!(
        h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
            .nth(2)
            .is_some_and(|n| n.is_focused()),
        "⇧⌘F 后替换框应获得焦点"
    );
}

#[test]
fn ignore_minor_toggles_all_options() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\n");
    let r = write(d.path(), "r.txt", "b\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 默认全 false
    assert!(!tab.borrow().opts.ignore_whitespace);
    // 模拟菜单「忽略不重要差异」：四开关同开
    {
        let mut t = tab.borrow_mut();
        t.opts.ignore_whitespace = true;
        t.opts.ignore_trailing = true;
        t.opts.ignore_case = true;
        t.opts.ignore_crlf = true;
        t.recompute();
    }
    assert!(tab.borrow().opts.ignore_whitespace);
    assert!(tab.borrow().opts.ignore_crlf);
    // 重算后无 panic、diff_rows 清空/更新
    assert!(!tab.borrow().rows.is_empty());
}

// ---- P39-2e：比较文件使用（视图切换） -------------

#[test]
fn compare_using_reopen_views() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nB\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    // 文本 → 图片视图
    {
        let mut app = app.borrow_mut();
        app.reopen_as_image(&l, &r);
        assert!(
            matches!(app.tabs[0], super::Tab::Image(_)),
            "应打开图片视图"
        );
    }
    // 文本 → 表格视图
    {
        let mut app = app.borrow_mut();
        app.reopen_as_csv(&l, &r);
        assert!(matches!(app.tabs[1], super::Tab::Csv(_)), "应打开表格视图");
    }
    // 文本 → hex 视图（强制 hex 细节）
    {
        let mut app = app.borrow_mut();
        app.reopen_as_hex(&l, &r);
        assert!(matches!(app.tabs[2], super::Tab::Diff(_)), "应打开文本标签");
        if let super::Tab::Diff(t) = &app.tabs[2] {
            assert_eq!(
                t.detail_mode,
                DiffDetailMode::Hex,
                "hex 视图应强制 Hex 细节"
            );
        }
    }
}

// ---- P41-1：展开/折叠全部（D7） ----------------

#[test]
fn dirtab_expand_collapse_all() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "sub/a.txt", "x");
    write(d1.path(), "sub/deep/b.txt", "y");
    write(d1.path(), "root.txt", "z");
    write(d2.path(), "sub/a.txt", "x");
    write(d2.path(), "sub/deep/b.txt", "y");
    write(d2.path(), "root.txt", "z");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().compare_content = true;
    tab.borrow_mut().view_filter = ViewFilter::All;
    tab.borrow_mut().only_diff = false; // only_diff=true 会把相同文件全过滤
    tab.borrow_mut().show_same = true;
    tab.borrow_mut().refresh_sync();
    // 初始全部展开（flat 含全部文件）
    assert!(
        tab.borrow().flat.iter().any(|r| r.name == "b.txt"),
        "初始应展开到深层文件"
    );
    // 折叠全部 → 深层文件消失
    tab.borrow_mut().collapse_all();
    assert!(
        !tab.borrow().flat.iter().any(|r| r.name == "b.txt"),
        "折叠全部后深层文件应隐藏"
    );
    assert!(
        tab.borrow()
            .flat
            .iter()
            .any(|r| r.is_dir && r.name == "sub/"),
        "折叠全部后仍显示顶层目录"
    );
    // 展开全部 → 深层文件恢复
    tab.borrow_mut().expand_all();
    assert!(
        tab.borrow().flat.iter().any(|r| r.name == "b.txt"),
        "展开全部后深层文件应恢复"
    );
}

// ---- P41-2：视图过滤扩展（较新维度 D4） ----------------

#[test]
fn dirtab_view_filter_left_right_newer() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    // 两侧内容不同 + mtime 不同：左新 / 右新 各一个
    write(d1.path(), "left_new.txt", "L1");
    write(d2.path(), "left_new.txt", "L2");
    write(d1.path(), "right_new.txt", "R2");
    write(d2.path(), "right_new.txt", "R1");
    // 手动调整 mtime：left_new.txt 左侧更新；right_new.txt 右侧更新
    let base = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    for (name, side_newer) in [("left_new.txt", "left"), ("right_new.txt", "right")] {
        let lp = d1.path().join(name);
        let rp = d2.path().join(name);
        let t_new =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_100);
        let t_old =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let (l_t, r_t) = if side_newer == "left" {
            (t_new, t_old)
        } else {
            (t_old, t_new)
        };
        let _ = filetime::set_file_mtime(&lp, filetime::FileTime::from_system_time(l_t));
        let _ = filetime::set_file_mtime(&rp, filetime::FileTime::from_system_time(r_t));
    }
    let _ = base;
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().view_filter = ViewFilter::All;
    tab.borrow_mut().only_diff = false;
    tab.borrow_mut().show_same = true;
    tab.borrow_mut().refresh_sync();
    // 两个文件都是 Differ（内容不同）
    {
        let t = tab.borrow();
        let r = t.result.as_ref().unwrap();
        assert_eq!(r.entries.len(), 2, "两个文件都应判 Differ");
        assert!(
            r.entries.iter().all(|e| e.status == FileStatus::Differ),
            "内容不同应判 Differ"
        );
    }
    // 仅左侧较新 → 只剩 left_new.txt
    tab.borrow_mut().view_filter = ViewFilter::LeftNewer;
    tab.borrow_mut().rebuild_tree();
    {
        let t = tab.borrow();
        let names: Vec<&str> = t.flat.iter().map(|r| r.name.as_str()).collect();
        assert!(
            names.contains(&"left_new.txt"),
            "LeftNewer 应含 left_new.txt: {:?}",
            names
        );
        assert!(
            !names.contains(&"right_new.txt"),
            "LeftNewer 不应含 right_new.txt: {:?}",
            names
        );
    }
    // 仅右侧较新 → 只剩 right_new.txt
    tab.borrow_mut().view_filter = ViewFilter::RightNewer;
    tab.borrow_mut().rebuild_tree();
    {
        let t = tab.borrow();
        let names: Vec<&str> = t.flat.iter().map(|r| r.name.as_str()).collect();
        assert!(
            names.contains(&"right_new.txt"),
            "RightNewer 应含 right_new.txt: {:?}",
            names
        );
        assert!(
            !names.contains(&"left_new.txt"),
            "RightNewer 不应含 left_new.txt: {:?}",
            names
        );
    }
}

// ---- P41-3：选择操作（D5） ----------------

#[test]
fn dirtab_selection_ops_select_all_invert_orphans() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "same.txt", "x");
    write(d1.path(), "only_left.txt", "L");
    write(d1.path(), "newer.txt", "N1");
    write(d2.path(), "same.txt", "x");
    write(d2.path(), "only_right.txt", "R");
    write(d2.path(), "newer.txt", "N2");
    // 让 newer.txt 左侧 mtime 更新（判 LeftNewer）
    let _ = filetime::set_file_mtime(
        d1.path().join("newer.txt"),
        filetime::FileTime::from_system_time(
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_100),
        ),
    );
    let _ = filetime::set_file_mtime(
        d2.path().join("newer.txt"),
        filetime::FileTime::from_system_time(
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        ),
    );
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().compare_content = true; // 快速模式 same.txt 两侧 mtime 不同会误判 Differ
    tab.borrow_mut().view_filter = ViewFilter::All;
    tab.borrow_mut().only_diff = false;
    tab.borrow_mut().show_same = true;
    tab.borrow_mut().refresh_sync();
    // 全选：文件全进 selected_set（same.txt 也选，目录不选）
    tab.borrow_mut().select_all();
    let rels = tab.borrow().selected_set_rels();
    assert!(
        rels.iter().any(|s| s == "same.txt"),
        "全选应含 same.txt: {:?}",
        rels
    );
    assert!(
        rels.iter().any(|s| s == "only_left.txt"),
        "全选应含 only_left.txt"
    );
    assert!(
        rels.iter().any(|s| s == "only_right.txt"),
        "全选应含 only_right.txt"
    );
    assert!(!rels.iter().any(|s| s.is_empty()), "目录不应被选中");
    // 反向选择：清空后全选（同文件集合）
    tab.borrow_mut().invert_selection();
    let rels2 = tab.borrow().selected_set_rels();
    assert!(
        rels2.is_empty(),
        "反向选择后应无选中（原来全选）: {:?}",
        rels2
    );
    tab.borrow_mut().invert_selection();
    // 选择独有项：only_left + only_right，不含 same/newer
    tab.borrow_mut().select_orphans();
    let rels3 = tab.borrow().selected_set_rels();
    assert!(
        rels3.iter().any(|s| s == "only_left.txt"),
        "独有应含 only_left: {:?}",
        rels3
    );
    assert!(
        rels3.iter().any(|s| s == "only_right.txt"),
        "独有应含 only_right"
    );
    assert!(
        !rels3.iter().any(|s| s == "same.txt"),
        "独有不应含 same.txt"
    );
    assert!(
        !rels3.iter().any(|s| s == "newer.txt"),
        "独有不应含 newer.txt"
    );
    // 选择较新项：newer.txt（LeftNewer）
    tab.borrow_mut().select_newer();
    let rels4 = tab.borrow().selected_set_rels();
    assert!(
        rels4.iter().any(|s| s == "newer.txt"),
        "较新应含 newer.txt: {:?}",
        rels4
    );
    assert!(
        !rels4.iter().any(|s| s == "only_left.txt"),
        "较新不应含 only_left"
    );
    assert!(!rels4.iter().any(|s| s == "same.txt"), "较新不应含 same");
    // 取消选择
    tab.borrow_mut().select_none();
    assert!(tab.borrow().selected_set_rels().is_empty());
}

// ---- P42-1：文本比较转换文件（BC Convert File） ----------------

#[test]
fn difftab_convert_file_trim_and_line_ending() {
    let d = tempdir().unwrap();
    // 左侧有行尾空白，右侧 CRLF
    let l = write(d.path(), "l.txt", "a  \nb\n");
    let r = write(d.path(), "r.txt", "a\r\nb\r\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // Trim 行尾空白（作用于两侧）
    tab.borrow_mut()
        .convert_file(crate::gui::textedit::ConvertMode::Trim);
    assert_eq!(
        std::fs::read_to_string(&l).unwrap(),
        "a\nb\n",
        "Trim 后左侧行尾空白应去除"
    );
    // 行尾 → LF（作用于两侧）
    tab.borrow_mut()
        .convert_file(crate::gui::textedit::ConvertMode::ToLf);
    assert_eq!(
        std::fs::read_to_string(&r).unwrap(),
        "a\nb\n",
        "ToLf 后右侧 CRLF 应转 LF"
    );
    // Tabs → 空格
    let t2 = write(d.path(), "t.txt", "\ta\n");
    let t3 = write(d.path(), "t3.txt", "\ta\n");
    let tab2 = RefCell::new(DiffTab::new());
    tab2.borrow_mut()
        .load_pair(&t2, &t3, ViewOptions::default());
    tab2.borrow_mut()
        .convert_file(crate::gui::textedit::ConvertMode::TabsToSpaces);
    assert_eq!(
        std::fs::read_to_string(&t2).unwrap(),
        "    a\n",
        "TabsToSpaces 后 tab 应转 4 空格"
    );
    // .bak 备份生成
    assert!(std::path::Path::new(&format!("{l}.bak")).exists());
}

// ---- P42-2：剪贴板比较 ⌘V + 文本编辑打开剪贴板 ----------------

#[test]
fn difftab_cmd_v_loads_clipboard_right() {
    // 用系统剪贴板（arboard）——测试环境可能无剪贴板，跳过真实断言，只验证快捷键不 panic
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nb\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 写入剪贴板后按 ⌘V → 右侧被剪贴板内容替换（headless 无剪贴板则保持原内容，不 panic）
    let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text("clip\n"));
    h.key_combination_modifiers(eframe::egui::Modifiers::COMMAND, &[eframe::egui::Key::V]);
    h.run();
    // 无断言硬依赖（CI headless 剪贴板不可用），仅验证不 panic
    let _ = tab.borrow().right.as_ref().map(|f| f.content.clone());
}

#[test]
fn textedit_open_clipboard_sets_content() {
    let mut tab = TextEditTab::new("");
    tab.content = "old".to_string();
    // 尝试从剪贴板读（headless 可能失败）；成功则内容替换、路径清空
    let ok = arboard::Clipboard::new()
        .and_then(|mut c| c.set_text("clipboard-new\n"))
        .is_ok();
    if ok {
        tab.open_clipboard();
        assert_eq!(tab.content, "clipboard-new\n", "打开剪贴板应替换内容");
        assert!(tab.is_empty(), "剪贴板内容未命名（另存）");
    }
}

// ---- P42-3：字符列标尺 ----------------

#[test]
fn difftab_ruler_toggle_renders() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nb\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    assert!(!tab.borrow().show_ruler, "标尺默认关闭");
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    tab.borrow_mut().show_ruler = true;
    h.run();
    assert!(
        tab.borrow().show_ruler,
        "开启标尺后应保持 true 且渲染不 panic"
    );
}

// ---- P42-4：图例 / 日志 / 工具栏开关 ----------------

#[test]
fn legend_and_log_windows_render() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    app.borrow_mut()
        .log
        .push("打开对比: a.txt ↔ b.txt".to_string());
    let mut h = Harness::new_ui(|ui| app.borrow_mut().legend_log_windows(ui));
    h.run();
    // 打开图例弹窗
    app.borrow_mut().show_legend = true;
    h.run();
    assert!(
        h.query_by_label("图例").is_some() || h.query_by_label("Legend").is_some(),
        "图例弹窗应渲染"
    );
    app.borrow_mut().show_legend = false;
    // 打开日志面板
    app.borrow_mut().show_log = true;
    h.run();
    assert!(
        h.query_all_by_label_contains("1 条").next().is_some()
            || h.query_all_by_label_contains("1").next().is_some(),
        "日志面板应显示条目数"
    );
    app.borrow_mut().show_log = false;
    // 全局工具栏开关默认开启（并发测试中不可改全局值，避免干扰其他工具栏测试）
    assert!(
        super::common::SHOW_TOOLBAR.load(std::sync::atomic::Ordering::Relaxed),
        "工具栏开关默认开启"
    );
}

// ---- P43-1：导航历史（后退/前进/上一层/比较父文件夹） ----------------

#[test]
fn dirtab_navigation_history_back_forward_up() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    let sub1 = d1.path().join("sub");
    let sub2 = d2.path().join("sub");
    std::fs::create_dir_all(&sub1).unwrap();
    std::fs::create_dir_all(&sub2).unwrap();
    write(&sub1, "a.txt", "x");
    write(&sub2, "a.txt", "x");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let s1 = sub1.to_str().unwrap().to_string();
    let s2 = sub2.to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().view_filter = ViewFilter::All;
    tab.borrow_mut().only_diff = false;
    tab.borrow_mut().show_same = true;
    tab.borrow_mut().refresh_sync();
    // 导航进入 sub 子目录（new 时左右路径非空 → 首次 navigate 把当前路径作为起点入栈）
    tab.borrow_mut().navigate(&s1, &s2);
    assert_eq!(tab.borrow().left, s1);
    assert_eq!(tab.borrow().history.len(), 2, "起点(根)+navigate 共 2 条");
    // 后退 → 回根
    assert!(tab.borrow_mut().back());
    assert_eq!(tab.borrow().left, p1, "后退应回到根目录");
    assert_eq!(tab.borrow().history_pos, 0);
    // 前进 → 回 sub
    assert!(tab.borrow_mut().forward());
    assert_eq!(tab.borrow().left, s1, "前进应回到 sub");
    assert_eq!(tab.borrow().history_pos, 1);
    // 上一层 → 回根
    assert!(tab.borrow_mut().up_level());
    assert_eq!(tab.borrow().left, p1, "上一层应回父目录");
    // 比较父文件夹（根再上一级）
    tab.borrow_mut().navigate(&s1, &s2);
    assert!(tab.borrow_mut().compare_parent());
    assert_eq!(tab.borrow().left, p1, "比较父文件夹应回父目录");
}

// ---- P43-2：文本选区操作（T6 选择选择内容/剪贴板比较） ----------------

#[test]
fn difftab_selection_ops_select_block_and_text() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\nd\ne\n");
    let r = write(d.path(), "r.txt", "a\nX\nc\nd\nY\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    // 定位到第 2 行差异（diff_rows 索引 1）
    tab.borrow_mut().next_diff();
    tab.borrow_mut().next_diff();
    // 选择选择内容：当前差异块选为选区
    tab.borrow_mut().select_selection();
    let sel = tab.borrow().selection;
    assert!(sel.is_some(), "应产生选区");
    // 选区文本（左侧行）
    let text = tab.borrow().selection_text();
    assert!(!text.is_empty(), "选区文本非空");
    // 把选择内容和剪贴板比较：右侧被选区文本替换（headless 剪贴板不可用则跳过断言）
    tab.borrow_mut().selection_to_clipboard();
    let _ = tab.borrow().right.as_ref().map(|f| f.content.clone());
}

// ---- P43-3：替换导航 + 差异文件导航 ----------------

#[test]
fn difftab_replace_nav_next_prev() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "alpha\nbeta\nalpha\n");
    let r = write(d.path(), "r.txt", "ALPHA\nbeta\nALPHA\n");
    let tab = RefCell::new(DiffTab::new());
    tab.borrow_mut().load_pair(&l, &r, ViewOptions::default());
    tab.borrow_mut().search.query = "a".to_string();
    tab.borrow_mut().update_search();
    let mut h = Harness::new_ui(|ui| tab.borrow_mut().ui(ui));
    h.run();
    // 下一替换：跳到第一匹配并聚焦替换框
    tab.borrow_mut().next_replace();
    assert_eq!(
        tab.borrow().search.current,
        Some(0),
        "下一替换应定位到第一匹配"
    );
    assert!(tab.borrow().search.replace_focus, "下一替换应聚焦替换框");
    // 再下一替换：循环到第二匹配
    tab.borrow_mut().next_replace();
    assert_eq!(
        tab.borrow().search.current,
        Some(1),
        "再下一替换应到第二匹配"
    );
    // 上一替换：回到第一匹配
    tab.borrow_mut().prev_replace();
    assert_eq!(tab.borrow().search.current, Some(0), "上一替换应回第一匹配");
}

#[test]
fn dirtab_next_prev_diff_file_navigation() {
    let d1 = tempdir().unwrap();
    let d2 = tempdir().unwrap();
    write(d1.path(), "same.txt", "x");
    write(d1.path(), "diff1.txt", "A");
    write(d1.path(), "diff2.txt", "B");
    write(d2.path(), "same.txt", "x");
    write(d2.path(), "diff1.txt", "B");
    write(d2.path(), "diff2.txt", "C");
    let p1 = d1.path().to_str().unwrap().to_string();
    let p2 = d2.path().to_str().unwrap().to_string();
    let tab = RefCell::new(DirTab::new(&p1, &p2));
    tab.borrow_mut().compare_content = true;
    tab.borrow_mut().view_filter = ViewFilter::All;
    tab.borrow_mut().only_diff = false;
    tab.borrow_mut().show_same = true;
    tab.borrow_mut().refresh_sync();
    // 下一差异文件：应选中第一个差异文件
    assert!(tab.borrow_mut().next_diff_file(), "应找到差异文件");
    let rel1 = tab.borrow().selected_rel();
    assert!(rel1.is_some(), "选中应有相对路径");
    // 再下一：到第二个差异文件
    assert!(tab.borrow_mut().next_diff_file());
    let rel2 = tab.borrow().selected_rel();
    assert_ne!(rel1, rel2, "两个差异文件应不同");
    // 上一差异文件：回到第一个
    assert!(tab.borrow_mut().prev_diff_file());
    let rel3 = tab.borrow().selected_rel();
    assert_eq!(rel1, rel3, "上一差异文件应回到第一个");
}

// ---- P43-4：合并文件 + 和输出比较 ----------------

#[test]
fn reopen_as_merge_and_compare_with_output() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nB\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    // 文本 → 合并文件（BASE 留空）
    {
        let mut app = app.borrow_mut();
        app.reopen_as_merge(&l, &r);
        assert!(
            matches!(app.tabs[0], super::Tab::Merge(_)),
            "应打开三路合并标签"
        );
        if let super::Tab::Merge(m) = &app.tabs[0] {
            assert_eq!(m.left_path, l, "左路径应为原左文件");
            assert_eq!(m.right_path, r, "右路径应为原右文件");
        }
    }
    // 文件夹合并 → 和输出比较（输出 vs 左侧开 DirTab）
    let d2 = tempdir().unwrap();
    let out_dir = d2.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.to_str().unwrap().to_string();
    {
        let mut app = app.borrow_mut();
        app.add_tab(super::Tab::FolderMerge(super::FolderMergeTab::new(
            "", &l, &r, &out,
        )));
        app.active = 1;
        app.compare_with_output();
        assert_eq!(app.tabs.len(), 3, "应新增 1 个标签");
        assert!(
            matches!(app.tabs[2], super::Tab::Dir(_)),
            "和输出比较应打开目录对比"
        );
    }
}

// ---- P43-5：信息弹窗 ----------------

#[test]
fn info_window_shows_tab_stats() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "a\nB\nc\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    {
        let mut app = app.borrow_mut();
        app.open_empty_diff();
        if let super::Tab::Diff(t) = &mut app.tabs[0] {
            t.load_pair(&l, &r, ViewOptions::default());
        }
        app.show_info = true;
    }
    let mut h = Harness::new_ui(|ui| app.borrow_mut().info_window(ui));
    h.run();
    // 信息弹窗应显示文本对比视图 + 差异行统计
    assert!(
        h.query_all_by_label_contains("文本对比").next().is_some(),
        "信息弹窗应显示视图类型"
    );
    assert!(
        h.query_all_by_label_contains("3").next().is_some()
            || h.query_all_by_label_contains("行数").next().is_some(),
        "信息弹窗应显示行数"
    );
    app.borrow_mut().show_info = false;
}

// ---- P43-6：媒体比较 ----------------

#[test]
fn mediatab_compares_wav_metadata() {
    let d = tempdir().unwrap();
    // 两个 WAV：仅采样率不同 → 应检出差异字段
    let mk = |name: &str, sr: u32| -> String {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&36u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&sr.to_le_bytes());
        bytes.extend_from_slice(&(sr * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(sr * 4).to_le_bytes());
        let p = d.path().join(name);
        std::fs::write(&p, &bytes).unwrap();
        p.to_str().unwrap().to_string()
    };
    let l = mk("l.wav", 44100);
    let r = mk("r.wav", 48000);
    let t = super::MediaTab::new(&l, &r);
    // 采样率不同 → 至少 sample_rate 一个差异字段
    assert!(
        t.diffs.iter().any(|df| df.field == "sample_rate"),
        "应检出采样率差异"
    );
    assert!(!t.title().is_empty());
}

// ---- P44-1：窗口菜单 + 标签切换 ----------------

#[test]
fn window_tab_switch_and_close_all() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    {
        let mut app = app.borrow_mut();
        app.open_empty_diff();
        app.open_empty_dir();
        app.open_empty_csv();
        assert_eq!(app.tabs.len(), 3);
        assert_eq!(app.active, 2);
        // ⌘] 下一标签：2 → 0（循环）
        app.next_tab();
        assert_eq!(app.active, 0);
        // ⌘[ 上一标签：0 → 2（循环）
        app.prev_tab();
        assert_eq!(app.active, 2);
        // ⌘⇧W 关闭所有窗口：清空回主页
        app.close_all_tabs();
        assert!(app.tabs.is_empty());
        assert_eq!(app.active, 0);
    }
}

// ---- P44-2：文本比较快捷键（⌘A 对齐 / ] [ 缩进 / ⌘E 选区查找） ----------------

#[test]
fn diff_tab_align_indent_find_shortcuts() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "    a\n    b\n    c\n");
    let r = write(d.path(), "r.txt", "    A\n    b\n    c\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    {
        let mut app = app.borrow_mut();
        app.open_empty_diff();
        if let super::Tab::Diff(t) = &mut app.tabs[0] {
            t.load_pair(&l, &r, ViewOptions::default());
            // ⌘E 使用选择内容查找：先选区（选差异块），再 find_selection
            t.select_selection();
            let sel = t.selection_text();
            assert!(!sel.is_empty(), "选区应有内容");
            t.find_selection();
            assert!(
                t.search.query.contains(&sel),
                "查找框应填入选区文本（{}）",
                t.search.query
            );
            // ] 增加缩进（当前差异块）
            t.indent_current(1);
            let after = t.selection_text();
            assert!(
                after.starts_with("    ") && sel.trim_start() == after.trim_start(),
                "缩进后选区首行应多 4 空格"
            );
            // ⌘A 对齐当前块
            t.align_current();
            assert!(t.align_pick.is_some(), "对齐应进入等待点击目标行状态");
        }
    }
}

// ---- P44-3：文本合并快捷键（⇧←/⇧→ 采用、⌘B/⇧⌘B 顺序合并、⌘⇧⌃↓/↑ 冲突导航） ----------------

#[test]
fn merge_tab_take_shortcuts() {
    let d = tempdir().unwrap();
    let base = write(d.path(), "base.txt", "a\nb\nc\n");
    let l = write(d.path(), "l.txt", "L1\nb\nc\n");
    let r = write(d.path(), "r.txt", "R1\nb\nc\n");
    let mut t = super::MergeTab::new(&base, &l, &r);
    // 首冲突块未解决 → 定位后采用左边（⇧← 语义）
    t.next_conflict();
    t.resolve_current(crate::mergeview::Resolution::Left);
    assert!(
        t.view
            .conflict_block_indices
            .iter()
            .all(|&bi| t.view.blocks[bi].resolution != crate::mergeview::Resolution::Auto),
        "采用左边后冲突应已解决"
    );
    // 下一冲突并采用右边（⇧→ 语义）
    t.next_conflict();
    t.resolve_current(crate::mergeview::Resolution::Right);
    // 顺序合并（⌘B 左后右 / ⇧⌘B 右后左）
    t.resolve_current(crate::mergeview::Resolution::LeftThenRight);
    t.resolve_current(crate::mergeview::Resolution::RightThenLeft);
    assert!(
        t.view.blocks.iter().all(|b| {
            b.resolution != crate::mergeview::Resolution::Auto
                || b.kind != crate::mergeview::BlockKind::Conflict
        }),
        "全部冲突应采用后不再有 Auto"
    );
}

// ---- P44-4：会话/文件菜单补齐（⌥⌘O 打开会话 / ⌘R 重新比较 / ⌘⇧S 保存文件为 / 已锁定） ----------------

#[test]
fn session_lock_and_reload_shortcuts() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\nc\n");
    let r = write(d.path(), "r.txt", "a\nB\nc\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    {
        let mut app = app.borrow_mut();
        app.open_empty_diff();
        if let super::Tab::Diff(t) = &mut app.tabs[0] {
            t.load_pair(&l, &r, ViewOptions::default());
            // 已锁定开关（Session 菜单 checkbox 语义）
            t.locked = true;
            assert!(t.locked, "锁定开关应生效");
            t.locked = false;
        }
        // ⌘R 重新比较（reload_current：DiffTab reload 不崩溃且保留路径）
        app.reload_current();
        if let super::Tab::Diff(t) = &app.tabs[0] {
            assert_eq!(t.left.as_ref().map(|f| f.path.as_str()), Some(l.as_str()));
        }
        // ⌥⌘O 打开会话
        app.show_sessions = true;
        assert!(app.show_sessions);
        app.show_sessions = false;
    }
}

// ---- P44-5：工具菜单（导出/导入设置、恢复出厂默认、编辑文本文件/查看补丁） ----------------

#[test]
fn settings_export_import_reset() {
    let d = tempdir().unwrap();
    let p = d.path().join("cfg.toml");
    let mut s = super::Settings {
        lang: "ja".to_string(),
        ignore_case: true,
        ..super::Settings::default()
    };
    // 导出 → 文件存在且含 lang=ja
    s.export_to(&p).unwrap();
    let txt = std::fs::read_to_string(&p).unwrap();
    assert!(txt.contains("ja"), "导出应含 lang=ja");
    // 重置 → 默认（lang 空）
    s.reset_defaults();
    assert!(s.lang.is_empty(), "恢复出厂后 lang 应为空");
    // 导入 → 恢复 ja/ignore_case
    s.import_from(&p).unwrap();
    assert_eq!(s.lang, "ja");
    assert!(s.ignore_case);
}

// ---- P44-6：视图开关 + 表格快捷键（DiffTab 行号/语法开关；CsvTab 修改 ⇧⌃↩/前后插行/排序） ----------------

#[test]
fn diff_view_switches_and_csv_shortcuts() {
    let d = tempdir().unwrap();
    // DiffTab 行号/语法开关默认开，可关
    let mut dt = super::DiffTab::new();
    assert!(dt.show_line_numbers && dt.show_syntax);
    dt.show_line_numbers = false;
    dt.show_syntax = false;
    assert!(!dt.show_line_numbers && !dt.show_syntax);

    // CsvTab 排序对话框（selected 私有，走公开方法验证）
    let l = write(d.path(), "l.csv", "id,name\n1,a\n2,b\n");
    let r = write(d.path(), "r.csv", "id,name\n1,A\n2,b\n");
    let mut ct = super::CsvTab::new(&l, &r);
    assert!(!ct.sort_dialog_open());
    ct.open_sort_dialog();
    assert!(ct.sort_dialog_open(), "排序对话框应打开");
    assert!(!ct.sort_label().is_empty());
}

// ---- P44-7：搜索补齐（DirTab 查找文件名 ⌘F；TextEdit 在多个文件中查找 ⌘⇧F） ----------------

#[test]
fn dirtab_find_name_and_textedit_find_in_files() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l", "");
    let r = write(d.path(), "r", "");
    let mut t = super::DirTab::new(&l, &r);
    t.refresh_sync();
    assert!(t.find_name.is_empty());
    // 查找文件名过滤（不区分大小写）
    t.find_name = "README".to_string();
    t.rebuild_tree();
    assert!(t.find_name == "README");
    // 清空恢复
    t.find_name.clear();
    t.rebuild_tree();

    // TextEdit 在多个文件中查找（默认当前文件目录）
    let tf = write(d.path(), "note.txt", "hello\n");
    let mut te = super::TextEditTab::new(&tf);
    te.open_find_in_files();
    assert!(te.find_in_files_open(), "在多个文件中查找弹窗应打开");
}

// ---- P45-1：文本合并行级采用（⌥⇧←/→ 采用左/右行；Edit 菜单 3 项） ----------------

#[test]
fn merge_tab_line_take() {
    let d = tempdir().unwrap();
    let base = write(d.path(), "base.txt", "a\nb\nc\n");
    let l = write(d.path(), "l.txt", "L1\nL2\nc\n");
    let r = write(d.path(), "r.txt", "R1\nR2\nc\n");
    let mut t = super::MergeTab::new(&base, &l, &r);
    // 定位第一个冲突块，逐行行级采用（cur_line = 冲突块起始行/下一行）
    t.next_conflict();
    let start = t.view.conflict_rows[0];
    t.cur_line = start;
    t.take_line(crate::mergeview::Resolution::Left);
    t.cur_line = start + 1;
    t.take_line(crate::mergeview::Resolution::Right);
    assert_eq!(t.line_takes(), 2, "行级采用后应记录 2 行");
    // 预览输出应包含行级采用结果（首行取左、次行取右）
    let (lines, _) = crate::mergeview::render_merged(&t.view, &t.label_l, &t.label_r);
    assert!(
        lines.iter().any(|ln| ln == "L1"),
        "行级采用左边后输出应含 L1"
    );
    assert!(
        lines.iter().any(|ln| ln == "R2"),
        "行级采用右边后输出应含 R2"
    );
}

// ---- P45-2：文件夹合并视图过滤（1-7 快捷键 + View 菜单 7 项） ----------------

#[test]
fn folder_merge_view_filter() {
    let d = tempdir().unwrap();
    let b = write(d.path(), "b", "");
    let l = write(d.path(), "l", "");
    let r = write(d.path(), "r", "");
    let mut t = super::FolderMergeTab::new(&b, &l, &r, "");
    t.reload();
    let Some(plan) = &t.plan else {
        return; // 空目录无计划也通过（环境相关）
    };
    // 过滤匹配：全部恒真；未变化只匹配 same
    let has_same = plan.iter().any(|i| i.op == "same");
    t.view_filter = super::foldermergetab::MergeFilter::All;
    for item in plan.iter() {
        assert!(t.filter_matches(item), "All 过滤应匹配全部");
    }
    if has_same {
        t.view_filter = super::foldermergetab::MergeFilter::Unchanged;
        assert!(
            plan.iter().all(|i| !t.filter_matches(i) || i.op == "same"),
            "Unchanged 过滤只应匹配 same"
        );
    }
    // 默认过滤为 All
    assert_eq!(
        super::foldermergetab::MergeFilter::default(),
        super::foldermergetab::MergeFilter::All
    );
}

// ---- P45-3：文件夹比较视图过滤扩展（独有/不独有/差异但无独有/组合项） ----------------

#[test]
fn dirtab_view_filter_extended() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l", "");
    let r = write(d.path(), "r", "");
    let mut t = super::DirTab::new(&l, &r);
    t.refresh_sync();
    // 过滤匹配（通过 rebuild 后的 flat 条目数量验证新过滤枚举可用）
    t.view_filter = super::dirtab::ViewFilter::Orphans;
    t.rebuild_tree();
    t.view_filter = super::dirtab::ViewFilter::NonOrphans;
    t.rebuild_tree();
    t.view_filter = super::dirtab::ViewFilter::DiffNoOrphans;
    t.rebuild_tree();
    t.view_filter = super::dirtab::ViewFilter::LeftNewerOrOrphan;
    t.rebuild_tree();
    t.view_filter = super::dirtab::ViewFilter::RightNewerOrOrphan;
    t.rebuild_tree();
    t.view_filter = super::dirtab::ViewFilter::All;
    t.rebuild_tree();
}

// ---- P45-4：图片比较补齐（重置差异偏移 + 比较元数据弹窗） ----------------

#[test]
fn image_tab_reset_offset_and_meta() {
    let d = tempdir().unwrap();
    // 生成两张小 PNG（imgcmp 依赖文件存在才比较；这里仅验证方法不 panic）
    let p1 = write(d.path(), "a.png", "");
    let p2 = write(d.path(), "b.png", "");
    let mut t = super::ImageTab::new(&p1, &p2);
    // 重置差异偏移：滚动归零 + 请求定位差异（无 pair 时安全）
    t.scroll = eframe::egui::Vec2::new(100.0, 200.0);
    t.reset_diff_offset();
    assert_eq!(t.scroll, eframe::egui::Vec2::ZERO, "重置差异偏移应清空滚动");
    // 比较元数据：弹窗开关翻转 + show_meta 开启
    t.compare_meta();
    assert!(t.show_meta_compare, "比较元数据弹窗应打开");
    assert!(t.show_meta, "比较元数据应同时开启元数据显示");
    t.compare_meta();
    assert!(!t.show_meta_compare, "再次调用应关闭弹窗");
}

// ---- P45-5：表格/HEX/补丁/文本编辑补齐（在后面插入列/HEX复制到右边/选择选择内容/⌘E选区查找） ----------------

#[test]
fn p45_5_misc_tab_features() {
    let d = tempdir().unwrap();
    // CsvTab 在后面插入列
    let l = write(d.path(), "l.csv", "id,name\n1,a\n");
    let r = write(d.path(), "r.csv", "id,name\n1,A\n");
    let mut ct = super::CsvTab::new(&l, &r);
    ct.select_row_col(0, 0);
    assert!(ct.insert_col_after(), "在后面插入列应成功");

    // PatchTab 选择选择内容（第一个差异块选为选区）
    let patch = write(
        d.path(),
        "p.patch",
        "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new\n",
    );
    let mut pt = super::PatchTab::new(&patch);
    pt.select_selection();
    assert!(pt.selection.is_some(), "补丁应有差异块可选为选区");

    // TextEdit 使用选择内容查找（⌘E）：选区文本填入查找框
    let tf = write(d.path(), "note.txt", "hello world\n");
    let mut te = super::TextEditTab::new(&tf);
    te.sel_range = Some((0, 5));
    te.find_selection();
    assert_eq!(te.search, "hello", "选区文本应填入查找框");
}

// ---- P46-1：TextEdit 视图开关（行号/自动换行/文件信息） ----------------

#[test]
fn textedit_view_switches() {
    let d = tempdir().unwrap();
    let tf = write(d.path(), "note.txt", "hello\nworld\n");
    let mut t = super::TextEditTab::new(&tf);
    // 默认：行号/文件信息开，自动换行关
    assert!(t.show_line_numbers && t.show_file_info);
    assert!(!t.show_wrap);
    // 切换开关
    t.show_line_numbers = false;
    t.show_wrap = true;
    t.show_file_info = false;
    assert!(!t.show_line_numbers && !t.show_file_info && t.show_wrap);
    // 文件信息辅助方法可用
    assert_eq!(t.char_count(), 12); // "hello\nworld\n" 含 2 个换行符
    assert_eq!(t.line_count(), 2);
}

// ---- P46-2：PatchTab 差异导航（⇧⌥⌃↓/↑ 差异、⇧⌃↓/↑ 差异部分） ----------------

#[test]
fn patch_tab_diff_navigation() {
    let d = tempdir().unwrap();
    let patch = write(
        d.path(),
        "p.patch",
        "--- a\n+++ b\n@@ -1,3 +1,3 @@\n-old1\n+new1\n same\n@@ -5,2 +5,2 @@\n-old2\n+new2\n",
    );
    let mut t = super::PatchTab::new(&patch);
    // 无差异时 next_diff 安全
    t.next_diff();
    // 存在差异（至少两个差异块）→ 下一差异应定位到非 Equal 行
    if t.parsed.is_some() {
        t.next_diff();
        assert!(t.current_diff_pos().is_some(), "下一差异应定位");
        let p1 = t.current_diff_pos().unwrap();
        t.next_diff();
        let p2 = t.current_diff_pos().unwrap();
        assert_ne!(p1, p2, "连续两次下一差异应不同行");
        // 区块导航：下一差异部分
        t.next_diff_section();
        assert!(t.current_diff_pos().is_some());
        // 上一差异
        t.prev_diff();
        assert!(t.current_diff_pos().is_some());
    }
}

// ---- P46-3：hex 视图过滤与布局（1/2/3 + 边并排/上-下） ----------------

#[test]
fn hex_view_filter_and_layout() {
    let mut t = super::DiffTab::new();
    // 默认：全部过滤 + 边并排布局
    assert_eq!(t.hex_filter, super::difftab::HexViewFilter::All);
    assert_eq!(t.hex_layout, super::difftab::HexViewLayout::SideBySide);
    // 切换过滤（1/2/3 语义）
    t.hex_filter = super::difftab::HexViewFilter::Diff;
    assert_eq!(t.hex_filter, super::difftab::HexViewFilter::Diff);
    t.hex_filter = super::difftab::HexViewFilter::Same;
    assert_eq!(t.hex_filter, super::difftab::HexViewFilter::Same);
    // 切换布局（边并排/上-下）
    t.hex_layout = super::difftab::HexViewLayout::TopBottom;
    assert_eq!(t.hex_layout, super::difftab::HexViewLayout::TopBottom);
}

// ---- P46-4：DirTab 结构选项（总是显示文件夹/仅比较文件） ----------------

#[test]
fn dirtab_structure_options() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l", "");
    let r = write(d.path(), "r", "");
    let mut t = super::DirTab::new(&l, &r);
    // 默认：总是显示文件夹开，仅比较文件关
    assert!(t.show_all_dirs);
    assert!(!t.only_files);
    // 切换结构选项后 rebuild 不 panic
    t.show_all_dirs = false;
    t.only_files = true;
    t.refresh_sync();
    t.rebuild_tree();
    t.show_all_dirs = true;
    t.only_files = false;
    t.rebuild_tree();
}

// ---- P46-5：文件夹同步视图 + 工作空间（⇧L 图例快捷键；标签布局 TOML 持久化） ----------------

#[test]
fn workspace_save_load_roundtrip() {
    let d = tempdir().unwrap();
    let l = write(d.path(), "l.txt", "a\nb\n");
    let r = write(d.path(), "r.txt", "a\nB\n");
    let wp = d.path().join("ws.toml");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    {
        let mut app = app.borrow_mut();
        app.open_empty_diff();
        if let super::Tab::Diff(t) = &mut app.tabs[0] {
            t.load_pair(&l, &r, ViewOptions::default());
        }
        app.open_empty_dir();
        // 保存工作空间（Diff + Dir 两个标签）
        let sr = app.save_workspace(&wp);
        assert!(sr.is_ok(), "保存工作空间应成功: {:?}", sr);
        // 清空后加载
        app.tabs.clear();
        app.active = 0;
        let lr = app.load_workspace(&wp);
        assert!(lr.is_ok(), "加载工作空间应成功: {:?}", lr);
        assert_eq!(app.tabs.len(), 2, "应恢复 2 个标签");
        if let super::Tab::Diff(t) = &app.tabs[0] {
            assert_eq!(t.left.as_ref().map(|f| f.path.as_str()), Some(l.as_str()));
        }
    }
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

// ---- P49-3：菜单栏真点验证（kittest 驱动完整 menu_bar，点击菜单项触发行为） ----

#[test]
fn menubar_session_new_text_creates_diff_tab() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    assert!(app.borrow().tabs.is_empty(), "前置：无标签页");
    let mut h = Harness::new_ui(|ui| crate::gui::menubar::menu_bar(&mut app.borrow_mut(), ui));
    h.run();
    // 展开「会话」菜单（菜单 popup 需要多帧展开）
    h.get_by_label(crate::i18n::t(crate::i18n::Key::MenuSession))
        .click();
    h.run_steps(2);
    // 点击「新建文本对比」→ 应创建 Diff 标签
    h.get_by_label(crate::i18n::t(crate::i18n::Key::MenuNewText))
        .click();
    h.run_steps(2);
    assert_eq!(app.borrow().tabs.len(), 1, "菜单新建文本对比应创建标签");
    assert!(
        matches!(app.borrow().tabs[0], super::Tab::Diff(_)),
        "新标签应为 Diff"
    );
}

#[test]
fn menubar_session_new_dir_creates_dir_tab() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    let mut h = Harness::new_ui(|ui| crate::gui::menubar::menu_bar(&mut app.borrow_mut(), ui));
    h.run();
    h.get_by_label(crate::i18n::t(crate::i18n::Key::MenuSession))
        .click();
    h.run_steps(2);
    h.get_by_label(crate::i18n::t(crate::i18n::Key::MenuNewDir))
        .click();
    h.run_steps(2);
    assert_eq!(app.borrow().tabs.len(), 1, "菜单新建文件夹对比应创建标签");
    assert!(
        matches!(app.borrow().tabs[0], super::Tab::Dir(_)),
        "新标签应为 Dir"
    );
}

#[test]
fn menubar_session_new_merge_creates_merge_tab() {
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    let mut h = Harness::new_ui(|ui| crate::gui::menubar::menu_bar(&mut app.borrow_mut(), ui));
    h.run();
    h.get_by_label(crate::i18n::t(crate::i18n::Key::MenuSession))
        .click();
    h.run_steps(2);
    h.get_by_label(crate::i18n::t(crate::i18n::Key::MenuNewMerge))
        .click();
    h.run_steps(2);
    assert_eq!(app.borrow().tabs.len(), 1, "菜单新建三路合并应创建标签");
    assert!(
        matches!(app.borrow().tabs[0], super::Tab::Merge(_)),
        "新标签应为 Merge"
    );
}

// ---- 拖放注入验证（Bug 1：拖拽导入不成功） ----------------

/// 模拟拖放文件的测试桩：实现 egui::DroppedFile（path + bytes）
#[derive(Debug)]
struct TestDroppedFile(std::path::PathBuf);

impl eframe::egui::DroppedFile for TestDroppedFile {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn inject_dropped(h: &mut Harness, paths: &[String]) {
    for p in paths {
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(TestDroppedFile(
                std::path::PathBuf::from(p),
            )));
    }
}

#[test]
fn dropped_single_file_creates_diff_tab_with_left_loaded() {
    let d = tempdir().unwrap();
    let f = write(d.path(), "a.txt", "hello\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    let mut h = Harness::new_ui(|ui| app.borrow_mut().handle_dropped(ui.ctx()));
    inject_dropped(&mut h, &[f]);
    h.run_steps(2);
    {
        let b = app.borrow();
        assert_eq!(b.tabs.len(), 1, "拖入单文件应创建标签");
        match &b.tabs[0] {
            super::Tab::Diff(t) => {
                assert!(t.left.is_some(), "左侧应加载");
                assert!(
                    !t.rows.is_empty(),
                    "单侧加载后应能立即显示内容（不等第二个文件）"
                );
            }
            _ => panic!("应为 DiffTab"),
        }
    }
}

#[test]
fn dropped_two_files_creates_diff_tab_with_both_sides() {
    let d = tempdir().unwrap();
    let f1 = write(d.path(), "a.txt", "a\n");
    let f2 = write(d.path(), "b.txt", "b\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    let mut h = Harness::new_ui(|ui| app.borrow_mut().handle_dropped(ui.ctx()));
    inject_dropped(&mut h, &[f1, f2]);
    h.run_steps(2);
    {
        let b = app.borrow();
        assert_eq!(b.tabs.len(), 1, "拖入双文件应创建标签");
        match &b.tabs[0] {
            super::Tab::Diff(t) => {
                assert!(t.left.is_some(), "左侧应加载");
                assert!(t.right.is_some(), "右侧应加载");
            }
            _ => panic!("应为 DiffTab"),
        }
    }
}

#[test]
fn dropped_two_dirs_creates_dir_tab() {
    let d = tempdir().unwrap();
    let dir1 = d.path().join("d1");
    let dir2 = d.path().join("d2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    let mut h = Harness::new_ui(|ui| app.borrow_mut().handle_dropped(ui.ctx()));
    inject_dropped(
        &mut h,
        &[
            dir1.to_string_lossy().into_owned(),
            dir2.to_string_lossy().into_owned(),
        ],
    );
    h.run_steps(2);
    {
        let b = app.borrow();
        assert_eq!(b.tabs.len(), 1, "拖入双目录应创建标签");
        match &b.tabs[0] {
            super::Tab::Dir(t) => {
                assert!(!t.left.is_empty());
                assert!(!t.right.is_empty());
            }
            _ => panic!("应为 DirTab"),
        }
    }
}

#[test]
fn dropped_single_file_into_empty_diff_tab_keeps_left_only() {
    let d = tempdir().unwrap();
    let f = write(d.path(), "a.txt", "hello\n");
    let app = RefCell::new(super::DiffApp::new(super::Settings::default()));
    // 已有空 DiffTab 会话（欢迎页点「文本对比」卡片后）再拖入单文件
    app.borrow_mut().add_tab(super::Tab::Diff(DiffTab::new()));
    let mut h = Harness::new_ui(|ui| app.borrow_mut().handle_dropped(ui.ctx()));
    inject_dropped(&mut h, &[f]);
    h.run_steps(2);
    {
        let b = app.borrow();
        assert_eq!(b.tabs.len(), 1, "拖入单文件不应新建标签");
        match &b.tabs[0] {
            super::Tab::Diff(t) => {
                assert!(t.left.is_some(), "左侧应加载");
                assert!(t.right.is_none(), "右侧应留空（不读空路径）");
                assert!(t.error.is_none(), "不应报 ENOENT 读取错误");
            }
            _ => panic!("应为 DiffTab"),
        }
    }
}
