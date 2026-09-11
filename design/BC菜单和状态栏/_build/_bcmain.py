# ══════════════════════════════════════════════ 输出
def main():
    print("输出到 %s/" % OUT)
    folder_a, folder_b = screen_folder()
    home_art, home_css = screen_home()
    page_css = HOME_CSS
    write(OUT + "/index.html", page_index())
    write(OUT + "/screens/text-compare.html", page(
        "文本比较",
        '<div class="sheet">'
        '<div class="artlabel">① 正向 —— order_main_v1.c ⇄ order_main_v2.c：绿 = 新增，琥珀 = 修改（真实文件，无删除行）</div>'
        + screen_text(False)
        + '<div class="artlabel">② 交换两边后 —— order_main_v2.c ⇄ order_main_v1.c：同一份真实数据，红 = 删除</div>'
        + screen_text(True) + '</div>'))
    write(OUT + "/screens/folder-compare.html", page(
        "文件夹比较",
        '<div class="sheet">'
        '<div class="artlabel">① 文件夹比较 —— webapp-v1 ⇄ webapp-v2（真实目录：仅右 / 已修改 / 二进制不同）</div>'
        + folder_a
        + '<div class="artlabel">② 压缩包以文件夹会话打开 —— pkg_v1.zip ⇄ pkg_v2.zip（仅左 / 仅右 / 已修改 / 二进制不同 四态齐全）</div>'
        + folder_b + '</div>'))
    write(OUT + "/screens/image-compare.html", page("图片比较", art(screen_image())))
    write(OUT + "/screens/hex-compare.html", page("十六进制比较", art(screen_hex())))
    write(OUT + "/screens/table-compare.html", page("表格比较", art(screen_table())))
    write(OUT + "/screens/media-compare.html", page("媒体比较", art(screen_media()), page_css))
    write(OUT + "/screens/text-merge.html", page("文本合并", art(screen_merge())))
    write(OUT + "/screens/home.html", page("主页", art(home_art), page_css))
    a1, a2 = page_menus()
    write(OUT + "/screens/menus.html", page(
        "菜单展开态",
        '<div class="sheet">'
        '<div class="artlabel">① 会话菜单 / 窗口菜单 —— 单标签时的置灰项</div>' + a1
        + '<div class="artlabel">② 编辑菜单 —— 比较会话（只读，置灰） vs 文本合并会话（可用）</div>' + a2
        + '<div class="artlabel">③ 视图菜单 —— 随会话类型整组变化（图片 / 文本 / 主页）</div>' + page_view_menus()
        + '</div>', MI_CSS))
    write(OUT + "/screens/status-bar.html", page_statusbar())
    print("完成")

main()
