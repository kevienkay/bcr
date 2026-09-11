HOME_CSS = """
.content.home{display:grid;grid-template-columns:392px 1fr}
.hside{background:var(--bg-sidebar);border-right:1px solid var(--line);display:flex;flex-direction:column}
.hs-head{padding:9px 14px 7px;font-size:12px;color:var(--fg2);font-weight:600}
.htree{flex:1 1 auto;padding:0 6px}
.tgrp{padding:7px 8px 4px;font-size:11.5px;color:#6E6E73}
.titem{display:flex;align-items:center;gap:8px;height:26px;padding:0 8px;border-radius:5px;font-size:12.5px}
.titem.sub{padding-left:22px;color:var(--fg2)}
.titem .dot{width:11px;height:11px;border-radius:3px;flex:0 0 auto}
.titem .tn{flex:1 1 auto;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.titem .tm{font-size:11px;color:#8E8E93;white-space:nowrap}
.titem.on{background:var(--accent);color:#fff}
.titem.on .tm{color:rgba(255,255,255,.85)}
.hs-foot{height:30px;display:flex;align-items:center;gap:7px;padding:0 12px;border-top:1px solid #CDD3DA;
  font-size:11.5px;color:var(--fg2)}
.hs-foot svg{width:14px;height:14px;stroke:currentColor;fill:none;stroke-width:1.5}
.hmain{background:#F8F8F8;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:26px}
.hcard{width:1120px;background:#fff;border:1px solid var(--line);border-radius:8px;padding:16px 20px;
  display:flex;flex-direction:column;gap:6px}
.hc-top{display:flex;align-items:center;gap:9px;font-size:14px}
.hc-ico svg{width:17px;height:17px;stroke:var(--accent);fill:none;stroke-width:1.5}
.hc-t{font-weight:600}
.hc-p{font-family:var(--mono);font-size:12px;color:var(--fg2);padding-left:26px}
.hc-act{display:flex;gap:9px;padding:8px 0 2px 26px}
.bpri{background:var(--accent);color:#fff;border-radius:5px;padding:5px 18px;font-size:12.5px}
.bgho{border:1px solid var(--line);border-radius:5px;padding:4px 18px;font-size:12.5px}
.hint{text-align:center;font-size:12.5px;color:var(--fg2);line-height:1.7}
.kgrid{display:flex;flex-wrap:wrap;justify-content:center;gap:10px 26px;max-width:1240px}
.kbtn{display:flex;flex-direction:column;align-items:center;gap:6px;width:108px;font-size:12px;
  color:var(--fg2);padding:8px 0;border-radius:6px}
.kbtn .ki svg{width:30px;height:30px;stroke:currentColor;fill:none;stroke-width:1.4}
.wave2{height:132px;margin-top:10px;border:1px solid var(--line);border-radius:4px;background:#FAFBFB;overflow:hidden}
.wave2 svg{width:100%;height:100%;display:block}
"""

# ══════════════════════════════════════════════ 真实样例数据
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _bcdata as D

TEXT_ROWS, TEXT_BLOCKS, TEXT_NL, TEXT_NR, TEXT_SL, TEXT_SR = D.text_data(26)
FOLDER_ROWS, FOLDER_CNT, FOLDER_L, FOLDER_R = D.folder_data()
ARCH_ROWS, ARCH_CNT, ARCH_L, ARCH_R, ARCH_SL, ARCH_SR = D.archive_data()
HEX_ROWS, HEX_LEN, HEX_DIFF, HEX_RANGES = D.hex_data(26)
CSV_HDR, CSV_ROWS, CSV_MOD, CSV_DCOLS = D.csv_data()
IMG = D.image_data()
MED = D.media_data()

def kb(n): return "%.2f KB" % (n / 1024)

# ─────────────────────────────────────────────── 通用工具条组合
TB_COMMON_L = [("home", "主页"), ("session", "会话")]
TB_COMMON_R = [("refresh", "刷新"), ("swap", "交换")]
SEG_LAYOUT = (["边并排", "上-下", "缩略图"], 0)

# ══════════════════════════════════════════════ ① 文本比较
def screen_text(rev=False):
    P = D.text_pair(rev)
    ROWS, blocks = P["rows"], P["blocks"]
    body = []
    for st, ln_l, code_l, ln_r, code_r in ROWS:
        if st == "mod":
            lh, rh = D.inline_hl(code_l, code_r)
        else:
            lh = H.escape(code_l) if code_l is not None else ""
            rh = H.escape(code_r) if code_r is not None else ""
        gl = '<span class="g l">%s</span>' % (ln_l or "")
        gr = '<span class="g r">%s</span>' % (ln_r or "")
        cl = ('<span class="c l">%s</span>' % lh) if code_l is not None else '<span class="c l"></span>'
        cr = ('<span class="c r">%s</span>' % rh) if code_r is not None else '<span class="c r"></span>'
        body.append('<div class="trow %s%s">%s%s<span class="sp"></span>%s%s</div>'
                    % (st, " gap" if st in ("del", "ins") else "", gl, cl, gr, cr))
    n = len(ROWS)
    mm, i = [], 0
    while i < n:
        if ROWS[i][0] != "same":
            j = i
            while j < n and ROWS[j][0] != "same": j += 1
            mm.append((ROWS[i][0], i, j - i)); i = j
        else: i += 1
    blocks_html = "".join('<span class="blk" style="top:%.2f%%;height:%.2f%%;background:%s"></span>'
                          % (k / n * 100, h / n * 100,
                             {"mod": "var(--c-amber)", "del": "var(--c-red)", "ins": "var(--c-green)"}[st])
                          for st, k, h in mm)
    content = ('<div class="content">'
               + two_pane_head("/样例文件/text/" + P["lname"],
                               "今天, 00:16:21 · %s 字节 · C,C++,C# 源代码 · Unicode (UTF-8) · UNIX · %d 行"
                               % (D.fmt(P["lsz"]), P["nl"]),
                               "/样例文件/text/" + P["rname"],
                               "今天, 00:16:21 · %s 字节 · C,C++,C# 源代码 · Unicode (UTF-8) · UNIX · %d 行"
                               % (D.fmt(P["rsz"]), P["nr"]))
               + '<div class="tcode"><div class="trows">' + "".join(body) + '<div class="eof"></div></div>'
               + '<div class="minimap">' + blocks_html
               + '<span class="vp" style="top:4%%;height:34%%"></span><span class="cap">差异图</span></div></div></div>')
    tb = [TB_COMMON_L, [("copyL", "复制到左"), ("copyR", "复制到右")],
          [("all", "显示全部", "on"), ("diff", "显示差异"), ("same", "显示相同")],
          [("ws", "可见空白"), ("syntax", "语法加亮"), ("wrap", "自动换行")],
          [("prev", "上一差异"), ("next", "下一差异"), ("find", "查找")],
          [("layout", "边并排", "on"), ("swap", "交换两边"), ("undo", "撤销", "off"), ("redo", "重做", "off")],
          TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>%d 个差异部分' % blocks),
                 ("", '<span class="chk"></span>忽略的不重要差异'),
                 ("", '插入 <span style="color:#8B8B8B">▾</span>'),
                 ("grow", "新增 <b class='num'>%d</b> · 删除 <b class='num'>%d</b> · 修改 <b class='num'>%d</b>"
                          % (P["nins"], P["ndel"], P["nmod"])),
                 ("", '加载时间: <span class="num">0.03</span> 秒')],
                '<div class="pane-ft">'
                '<span class="scell"><b class="num">2:1</b> <span style="color:#6E6E73">注释</span> '
                '<span style="font-family:var(--mono)">★ main.c · 订单处理入口 · %s ¶</span></span>'
                '<span class="scell"><b class="num">2:1</b> <span style="color:#6E6E73">注释</span> '
                '<span style="font-family:var(--mono)">★ main.c · 订单处理入口 · %s ¶</span></span></div>'
                % ("(v2)" if rev else "(v1)", "(v1)" if rev else "(v2)"))
    return art(menubar("文本比较") + tabbar(TABS_ALL, 2) + toolbar(tb, SEG_LAYOUT) + content + st)

# ══════════════════════════════════════════════ ② 文件夹比较（含压缩包）
def dir_cells(rows, sel=None):
    out = []
    for k, (st, ln, lsz, ldt, lat, rn, rsz, rdt, rat) in enumerate(rows):
        def cell(name, sz, dt, at, side):
            if name is None:
                return ('<div class="cell"><span class="dcell-ico"></span>'
                        '<span class="nm" style="color:#C9CDD2">—</span><span class="sz"></span>'
                        '<span class="dt"></span><span class="at"></span></div>')
            isdir = sz == "" or (name.endswith("/") )
            tag = '<span class="tag">二进制</span>' if st == "b" else ""
            return ('<div class="cell"><span class="dcell-ico %s"></span><span class="nm">%s%s</span>'
                    '<span class="sz">%s</span><span class="dt">%s</span><span class="at">%s</span></div>'
                    % ("dir" if isdir else "file", H.escape(name), tag, sz, dt, at))
        conn = {"l": "var(--c-red)", "r": "var(--c-amber)", "m": "var(--c-blue)",
                "b": "var(--c-violet)"}.get(st, "transparent")
        out.append('<div class="drow %s">%s<span class="mid"><b style="background:%s"></b></span>%s</div>'
                   % ("sel" if k == sel else st, cell(ln, lsz, ldt, lat, "l"), conn,
                      cell(rn, rsz, rdt, rat, "r")))
    return "".join(out)

def dir_screen(rows, cnt, lt, rt, lp, rp, ld, rd, active_tab, tb_extra):
    content = ('<div class="content">' + two_pane_head(lp, ld, rp, rd)
               + '<div class="dhead"><div class="grp"><span></span><span>名称</span>'
                 '<span class="num">大小</span><span class="num">已修改</span><span class="at">属性</span></div>'
                 '<span class="mid"></span>'
                 '<div class="grp"><span></span><span>名称</span><span class="num">大小</span>'
                 '<span class="num">已修改</span><span class="at">属性</span></div></div>'
               + '<div class="drows">' + dir_cells(rows, sel=3) + '</div></div>')
    tb = [TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差异"), ("same", "显示相同")],
          [("copyL", "复制到左"), ("copyR", "复制到右"), ("select", "选择文件")],
          [("expand", "展开"), ("collapse", "折叠")],
          [("prev", "上一差异"), ("next", "下一差异"), ("find", "查找")] + tb_extra, TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>%d 项差异' % (cnt["l"] + cnt["r"] + cnt["m"] + cnt["b"])),
                 ("", '<span class="chk"></span>忽略的不重要差异'),
                 ("", '复制到右边 <span style="color:#8B8B8B">▾</span>'),
                 ("grow", "仅左 <b class='num'>%d</b> · 仅右 <b class='num'>%d</b> · 已修改 <b class='num'>%d</b> · "
                          "二进制不同 <b class='num'>%d</b> · 相同 <b class='num'>%d</b>"
                          % (cnt["l"], cnt["r"], cnt["m"], cnt["b"], cnt["s"])),
                 ("", '加载时间: <span class="num">0.02</span> 秒')],
                '<span class="scell">%d 个文件, %s</span><span class="scell">130 GB 可用</span>'
                '<span class="scell grow" style="font-family:var(--mono)">%s</span>'
                '<span class="scell">%d 个文件, %s</span><span class="scell">130 GB 可用</span>'
                % (lt[0], kb(lt[1]), lp.split("/")[-1] + " ⇄ " + rp.split("/")[-1], rt[0], kb(rt[1])))
    return art(menubar("文件夹比较") + tabbar(TABS_ALL, active_tab) + toolbar(tb) + content + st)

def screen_folder():
    a = dir_screen(FOLDER_ROWS, FOLDER_CNT, FOLDER_L, FOLDER_R,
                   "/样例文件/folders/webapp-v1", "/样例文件/folders/webapp-v2",
                   "12 项 · 磁盘 130 GB 可用", "12 项 · 磁盘 130 GB 可用", 1,
                   [("layout", "边并排", "on"), ("file", "文件夹合并")])
    b = dir_screen(ARCH_ROWS, ARCH_CNT, ARCH_L, ARCH_R,
                   "/样例文件/archive/pkg_v1.zip", "/样例文件/archive/pkg_v2.zip",
                   "%s · %s" % (D.fmt(ARCH_SL), "压缩包"), "%s · %s" % (D.fmt(ARCH_SR), "压缩包"), 1,
                   [("layout", "边并排", "on"), ("meta", "合并基文件夹")])
    return a, b

# ══════════════════════════════════════════════ ③ 图片比较
def screen_image():
    def shapes(right=False):
        if not right:
            items = [("c", "left:6%;top:12%;width:23%;padding-top:23%", "#E03830"),
                     ("r", "left:44%;top:14%;width:30%;height:23%", "#3878E0"),
                     ("r", "left:6%;top:60%;width:28%;height:20%", "#F8C838"),
                     ("c", "left:54%;top:56%;width:27%;padding-top:27%", "#38A858"),
                     ("r", "left:38%;top:42%;width:6%;height:8%", "#EE79A8"),
                     ("r", "left:44%;top:56%;width:12%;height:14%", "#7858A0")]
        else:
            items = [("c", "left:8%;top:7%;width:29%;padding-top:29%", "#E03830"),
                     ("r", "left:46%;top:14%;width:35%;height:27%", "#3878E0"),
                     ("r", "left:6%;top:62%;width:28%;height:20%", "#F8C838"),
                     ("c", "left:55%;top:50%;width:29%;padding-top:29%", "#1890C8"),
                     ("r", "left:78%;top:64%;width:8%;height:18%", "#F08018")]
        return "".join('<i style="%s;background:%s;%s"></i>'
                       % (st, col, "border-radius:50%" if kk == "c" else "") for kk, st, col in items)
    mask = ('<span class="mask">'
            '<u style="left:5%;top:11%;width:25%;height:25%;background:rgba(225,60,50,.55)"></u>'
            '<u style="left:43%;top:13%;width:37%;height:29%;background:rgba(225,60,50,.45)"></u>'
            '<u style="left:53%;top:49%;width:33%;height:31%;background:rgba(255,200,60,.55)"></u>'
            '<u style="left:77%;top:63%;width:11%;height:21%;background:rgba(255,200,60,.6)"></u>'
            '<u style="left:37%;top:41%;width:9%;height:11%;background:rgba(225,60,50,.5)"></u></span>')
    wh = "%d × %d" % IMG["l"]["wh"]
    content = ('<div class="content">'
               + two_pane_head("/样例文件/image/logo_v1.png",
                               "今天, 00:16:21 · %s 字节 · 图片文件 · %s · 24 位 PNG" % (IMG["l"]["size"], wh),
                               "/样例文件/image/logo_v2.png",
                               "今天, 00:16:21 · %s 字节 · 图片文件 · %s · 24 位 PNG" % (IMG["r"]["size"], wh))
               + '<div class="imgwrap"><div class="imgpane"><div class="canvas">' + shapes(False) + '</div></div>'
               + '<div class="mid"></div>'
               + '<div class="imgpane"><div class="canvas">' + shapes(True) + mask + '</div></div></div></div>')
    tb = [TB_COMMON_L, [("tolerance", "容差"), ("range", "不匹配范围", "on"), ("blend", "混合")],
          [("rotate", "旋转"), ("flip", "水平翻转"), ("zoomin", "放大"), ("zoomout", "缩小"), ("fit", "适应窗口")],
          [("prev", "上一差异"), ("next", "下一差异"), ("layout", "边并排", "on")], TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>不匹配范围模式'),
                 ("", '<span class="chk"></span>忽略不重要差异'),
                 ("grow", "容差 <b class='num'>0</b> · 忽略 <b class='num'>0</b> 像素"),
                 ("", '加载时间: <span class="num">0.01</span> 秒')],
                '<span class="scell"><span class="sw" style="background:var(--mask-l)"></span>仅左图有 &nbsp;'
                '<span class="sw" style="background:var(--mask-r)"></span>仅右图有</span>'
                '<span class="scell grow">差异像素 <b class="num">31,842</b> · 占 4.98%</span>'
                '<span class="scell">偏移: <b class="num">0.0</b></span>')
    return art(menubar("图片比较") + tabbar(TABS_ALL, 3) + toolbar(tb, (["容差", "不匹配范围", "混合"], 1)) + content + st)

# ══════════════════════════════════════════════ ④ 十六进制比较
def screen_hex():
    head = ('<div class="hexrow head"><span class="a">偏移(h)</span>'
            '<span class="h">00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F</span>'
            '<span class="asc">0123456789ABCDEF</span></div>')
    def pane(side):
        out = [head]
        for addr, lb, rb, d in HEX_ROWS:
            raw = lb if side == "l" else rb
            hx, asc = [], []
            for n, byte in enumerate(raw.split()):
                v = int(byte, 16)
                vis = chr(v) if 32 <= v < 127 else "·"
                if n in d:
                    hx.append('<span class="b">%s</span>' % byte)
                    asc.append('<span class="b">%s</span>' % vis)
                else:
                    hx.append(byte); asc.append(vis)
            out.append('<div class="hexrow%s"><span class="a">%s</span><span class="h">%s</span>'
                       '<span class="asc">%s</span></div>'
                       % (" diff" if d else "", addr, " ".join(hx), "".join(asc)))
        return '<div class="hexcol">' + "".join(out) + '</div>'
    content = ('<div class="content">'
               + two_pane_head("/样例文件/hex/firmware_v1.bin",
                               "今天, 00:16:21 · %s 字节 · 其它一切 · 编码: --" % D.fmt(HEX_LEN),
                               "/样例文件/hex/firmware_v2.bin",
                               "今天, 00:16:21 · %s 字节 · 其它一切 · 编码: --" % D.fmt(HEX_LEN))
               + '<div class="hexwrap">' + pane("l") + '<div class="mid"></div>' + pane("r") + '</div></div>')
    tb = [TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差异"), ("same", "显示相同")],
          [("copyL", "复制到左"), ("copyR", "复制到右")],
          [("prev", "上一差异字节"), ("next", "下一差异字节"), ("find", "转到偏移")],
          [("layout", "边并排", "on"), ("meta", "字节地址 ▾")], TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>%d 个差异范围' % len(HEX_RANGES)),
                 ("", '<span class="chk"></span>忽略不重要差异'),
                 ("", '字节 <span style="color:#8B8B8B">▾</span>'),
                 ("grow", "当前差异 <b class='num'>1/%d</b> · F6 / F7 跳转" % len(HEX_RANGES)),
                 ("", '加载时间: <span class="num">0.04</span> 秒')],
                '<span class="scell">%d 个差异字节</span><span class="scell">相同 <b class="num">%d</b></span>'
                '<span class="scell grow" style="font-family:var(--mono)">偏移 0x%08X – 0x%08X</span>'
                '<span class="scell">小尾值 <b class="num">0x%08X</b></span>'
                % (HEX_DIFF, HEX_LEN - HEX_DIFF, HEX_RANGES[0][0], HEX_RANGES[0][1],
                   int.from_bytes(bytes([0x0D, 0, 0, 0]), "little")))
    return art(menubar("十六进制比较") + tabbar(TABS_ALL, 4) + toolbar(tb) + content + st)

# ══════════════════════════════════════════════ ⑤ 表格比较
def screen_table():
    def pane(side, sel=False):
        letters = ('<tr class="letters"><th class="mk"></th><th class="rn"></th>'
                   + "".join('<th class="%s">%s</th>'
                             % (("dc " if ci in CSV_DCOLS else "") + ("cur" if (ci == 0 and sel) else ""),
                                chr(65 + ci)) for ci in range(len(CSV_HDR))) + '</tr>')
        head = ('<tr><th class="mk"></th><th class="rn">#</th>'
                + "".join('<th>%s</th>' % c for c in CSV_HDR) + '</tr>')
        body = []
        for ri, (st, l, r) in enumerate(CSV_ROWS):
            cells = l if side == 0 else r
            if cells is None:
                body.append('<tr class="%s"><td class="mk"></td><td class="rn"></td>%s</tr>'
                            % ("d" if side == 0 else "s", '<td style="color:#C9CDD2">—</td>' * len(CSV_HDR)))
                continue
            other = r if side == 0 else l
            mk = "■" if st in ("m", "i", "d") else ""
            tds = []
            for ci, c in enumerate(cells):
                cls = []
                if other and other[ci] != c: cls.append("cd")
                if side == 0 and ri == 2 and ci == 3: cls.append("cur")
                tds.append('<td class="%s">%s</td>' % (" ".join(cls), c) if cls else '<td>%s</td>' % c)
            body.append('<tr class="%s%s"><td class="mk">%s</td><td class="rn">%d</td>%s</tr>'
                        % (st, " selcell" if (side == 0 and ri == 2) else "", mk, ri + 1, "".join(tds)))
        return ('<table class="tbl"><thead>%s%s</thead><tbody>%s</tbody></table>'
                % (letters, head, "".join(body)))
    content = ('<div class="content">'
               + two_pane_head("/样例文件/csv/staff_v1.csv",
                               "今天, 00:16:21 · 413 字节 · 逗号间隔的值 · Unicode (UTF-8) · UNIX · 逗号 · 列名称",
                               "/样例文件/csv/staff_v2.csv",
                               "今天, 00:16:21 · 413 字节 · 逗号间隔的值 · Unicode (UTF-8) · UNIX · 逗号 · 列名称")
               + '<div class="tablewrap"><div>' + pane(0, True) + '</div><div class="mid"></div>'
               + '<div>' + pane(1) + '</div></div></div>')
    tb = [TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差异"), ("same", "显示相同")],
          [("copyR", "复制单元格"), ("select", "选择行"), ("refresh", "删除行"), ("undo", "插入行")],
          [("prev", "上一差异单元格"), ("next", "下一差异单元格")],
          [("layout", "边并排", "on"), ("meta", "隐藏相同列")], TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>%d 个差异行' % CSV_MOD),
                 ("", '<span class="chk"></span>忽略不重要差异'),
                 ("", '编辑 <span style="color:#8B8B8B">▾</span>'),
                 ("grow", "A3 = <b class='num'>3</b> · Wang Wu"),
                 ("", '加载时间: <span class="num">0.04</span> 秒')],
                '<span class="scell">%d 个差异行</span><span class="scell">相同 <b class="num">%d</b></span>'
                '<span class="scell grow">%d 个差异列（%s）</span>'
                '<span class="scell">选择: 行 3, 列 D（salary）</span>'
                % (CSV_MOD, len(CSV_ROWS) - CSV_MOD, len(CSV_DCOLS),
                   ", ".join(CSV_HDR[c] for c in CSV_DCOLS)))
    return art(menubar("表格比较") + tabbar(TABS_ALL, 5) + toolbar(tb, SEG_LAYOUT) + content + st)

# ══════════════════════════════════════════════ ⑥ 媒体比较
def screen_media():
    def pane(k, diff_rate):
        m = MED[k]
        rows = [("数值", "%s 字节" % m["size"], True),
                ("音频", "PCM %d-bit %s" % (m["bits"], "单声道" if m["ch"] == 1 else "立体声"), False),
                ("采样率", "%d Hz" % m["rate"], diff_rate),
                ("时长", m["dur"], False),
                ("比特率", "%d kbps" % round(m["rate"] * m["bits"] * m["ch"] / 1000), False),
                ("声道", "1（单声道）" if m["ch"] == 1 else "2（立体声）", False)]
        tr = "".join('<tr class="%s"><td class="k">%s</td><td>%s</td></tr>' % ("d" if d else "", kk, v)
                     for kk, v, d in rows)
        wave = D.wave_svg(os.path.join(D.S, "media/tone_%s.wav" % ("v1" if k == "l" else "v2")), 300)
        return ('<div class="metacol"><table class="mtable"><thead><tr><th>名称</th><th>值</th></tr></thead>'
                '<tbody>%s</tbody></table><div class="wavedraw">%s</div></div>' % (tr, wave))
    content = ('<div class="content">'
               + two_pane_head("/样例文件/media/tone_v1.wav",
                               "今天, 00:24:13 · %s 字节" % MED["l"]["size"],
                               "/样例文件/media/tone_v2.wav",
                               "今天, 00:24:13 · %s 字节" % MED["r"]["size"])
               + '<div class="metawrap">' + pane("l", False) + '<div class="mid"></div>'
               + pane("r", False) + '</div></div>')
    tb = [TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差别"), ("same", "显示相同")],
          [("wave", "波形"), ("meta", "元数据")],
          [("play", "播放"), ("stop", "停止"), ("prev", "上一对比"), ("next", "下一对比")],
          [("zoomin", "放大"), ("zoomout", "缩小"), ("fit", "适应窗口")], TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>1 处差异（音频内容）'),
                 ("", '<span class="chk"></span>忽略不重要差异'),
                 ("grow", "元数据完全一致 · 差异在波形内容（440 Hz vs 880 Hz）"),
                 ("", '加载时间: <span class="num">0.05</span> 秒')],
                '<span class="scell">波形差异区段 <b class="num">1</b></span>'
                '<span class="scell grow">播放 00:00:00.860 / 00:00:01.500</span>'
                '<span class="scell">并列播放</span>')
    return art(menubar("媒体比较") + tabbar(TABS_ALL, 6) + toolbar(tb) + content + st)

# ══════════════════════════════════════════════ ⑦ 文本合并
def screen_merge():
    left = [("same", "1", "# app_v1.conf"), ("same", "2", "# 由 v1 升级而来"), ("same", "3", "[server]"),
            ("same", "4", "port = 8080"), ("cf", "5", "debug = true"), ("same", "6", "log_level = info"),
            ("cf", "7", "cache_ttl = 120"), ("same", "8", "log_file = /var/log/app.log")]
    right = [("same", "1", "# app_v2.conf"), ("same", "2", "# 由 v2 生成"), ("same", "3", "[server]"),
             ("same", "4", "port = 8080"), ("cf", "5", "debug = false"), ("same", "6", "log_level = info"),
             ("cf", "7", "cache_ttl = 300"), ("same", "8", "log_file = /var/log/app_v2.log")]
    out = [("same", "1", "# app_v1.conf"), ("same", "2", "# 由 v1 升级而来"), ("same", "3", "[server]"),
           ("same", "4", "port = 8080"), ("sel", "5", "debug = false"), ("same", "6", "log_level = info"),
           ("cf", "7", "cache_ttl = 300"), ("same", "8", "log_file = /var/log/app.log")]
    def col(title, tag, color, rows, cls=""):
        r = "".join('<div class="mrow %s"><span class="g">%s</span><span class="c">%s</span></div>'
                    % (st, ln, H.escape(code)) for st, ln, code in rows)
        bar = ('<div class="mbar2">%s<span>1:1</span><span>%s</span><span>%s</span></div>'
               % ('<span class="mbang">✔ 采用左边</span>' if cls != "out"
                  else '<span class="mbang g">✔ 输出已更新</span>',
                  "可编辑" if cls != "out" else "只读",
                  "冲突 2 处" if cls != "out" else "已解决 2 / 2"))
        return ('<div class="mcol %s"><div class="mhead">%s<span class="st" style="background:%s">%s</span>'
                '<span style="font-weight:400;color:#8B8B8B">app_%s.conf</span></div>'
                '<div class="mbody">%s</div>%s</div>'
                % (cls, title, color, tag, "v1" if title != "右" else "v2", r, bar))
    content = ('<div class="content"><div class="mwrap">'
               + col("左", "可编辑", "#C87C1E", left) + col("右", "可编辑", "#1F7A3D", right)
               + col("输出", "合并结果", "#0A63C9", out, "out") + '</div></div>')
    tb = [TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差别"), ("same", "显示相同")],
          [("copyL", "采用左边"), ("same", "采用中心"), ("copyR", "采用右边")],
          [("prev", "上一个冲突"), ("next", "下一个冲突"), ("find", "替换内容")],
          [("undo", "撤销"), ("redo", "重做"), ("save", "保存输出")], TB_COMMON_R]
    st = status([("", '<span class="x">✗</span>2 个冲突区域'),
                 ("", '<span class="chk"></span>忽略不重要差异'),
                 ("", '编辑 <span style="color:#8B8B8B">▾</span>'),
                 ("grow", "左右栏可编辑 · 输出栏只读"),
                 ("", '加载时间: <span class="num">0.02</span> 秒')],
                '<span class="scell">已解决 2 / 2</span>'
                '<span class="scell">2 个冲突区域</span>'
                '<span class="scell grow" style="font-family:var(--mono)">输出: /样例文件/out/app.conf</span>'
                '<span class="scell">保存输出 <span style="color:#8B8B8B">⌘S</span></span>')
    return art(menubar("文本合并") + tabbar(TABS_ALL, 7) + toolbar(tb) + content + st)

# ══════════════════════════════════════════════ ⑧ 主页
def screen_home():
    recent = [("pkg_v1.zip ⇄ pkg_v2.zip", "压缩包 · 5 个文件", "#2C7BE5", True),
              ("order_main_v1.c ⇄ order_main_v2.c", "文本比较 · %d 个差异部分" % TEXT_BLOCKS, "#2C7BE5", False),
              ("firmware_v1.bin ⇄ firmware_v2.bin", "十六进制 · %d 个差异字节" % HEX_DIFF, "#B3271E", False),
              ("logo_v1.png ⇄ logo_v2.png", "图片比较 · 1000 × 640", "#1F7A3D", False),
              ("app_v1.conf ⇄ app_v2.conf", "文本比较 · 2 个差异部分", "#2C7BE5", False),
              ("webapp-v1 ⇄ webapp-v2", "文件夹比较 · %d 项差异" % (FOLDER_CNT["m"] + FOLDER_CNT["r"] + FOLDER_CNT["b"]), "#2C7BE5", False)]
    tree = ['<div class="tgrp">▾ 最近保存</div>']
    for name, meta, col, sel in recent:
        tree.append('<div class="titem%s"><span class="dot" style="background:%s"></span>'
                    '<span class="tn">%s</span><span class="tm">%s</span></div>'
                    % (" on" if sel else "", col, H.escape(name), meta))
    tree += ['<div class="tgrp">▾ 最近</div>', '<div class="titem sub">浏览文件夹…</div>',
             '<div class="titem sub">新建会话…</div>', '<div class="tgrp">▾ 今天</div>',
             '<div class="tgrp">▾ 超过 6 天前</div>']
    kinds = [("folder", "文件夹比较", "#2C7BE5"), ("folder", "文件夹合并", "#C87C1E"), ("sync", "文件夹同步", "#6A3FA0"),
             ("file", "文本比较", "#2C7BE5"), ("file", "文本合并", "#1F7A3D"), ("textedit", "文本编辑", "#8B8B8B"),
             ("meta", "16 进制比较", "#B3271E"), ("wave", "媒体比较", "#1F7A3D"),
             ("tolerance", "图片比较", "#C87C1E"), ("select", "表格比较", "#2C7BE5")]
    grid = "".join('<div class="kbtn"><span class="ki" style="color:%s">%s</span><span>%s</span></div>'
                   % (col, I[ico], label) for ico, label, col in kinds)
    content = ('<div class="content home">'
               '<aside class="hside"><div class="hs-head">会话</div>'
               '<div class="htree">' + "".join(tree) + '</div>'
               '<div class="hs-foot">' + I["find"] + '<span>就绪</span></div></aside>'
               '<main class="hmain">'
               '<div class="hcard"><div class="hc-top"><span class="hc-ico">' + I["file"] + '</span>'
               '<span class="hc-t">pkg_v1.zip ⇄ pkg_v2.zip</span></div>'
               '<div class="hc-p">/样例文件/archive/pkg_v1.zip</div>'
               '<div class="hc-p">/样例文件/archive/pkg_v2.zip</div>'
               '<div class="hc-act"><span class="bpri">打开</span><span class="bgho">编辑</span></div></div>'
               '<p class="hint">将文件或文件夹拖放到会话图标上<br>或点击一个会话类型开始</p>'
               '<div class="kgrid">' + grid + '</div></main></div>')
    tb = [TB_COMMON_L]
    st = status([("", '<span class="chk on"></span>显示会话管理'), ("", "显示网络资源"),
                 ("grow", "6 个最近会话 · 磁盘 130 GB 可用"),
                 ("", '加载时间: <span class="num">0.01</span> 秒')])
    return art(menubar("主页") + tabbar(TABS_ALL, 0) + toolbar(tb) + content + st), HOME_CSS
