# -*- coding: utf-8 -*-
"""BC 5.2.5 (macOS) 高保真 UI 设计稿生成器 → BC菜单和状态栏/"""
import os, html as H

OUT = "BC菜单和状态栏"
os.makedirs(OUT + "/screens", exist_ok=True)

# ─────────────────────────────────────────────────────────── tokens
CSS = """
:root{
  /* 取样自 14 张 BC 截图的真实界面色 */
  --bg-window:#F0F4F5; --bg-panel:#F7F9F8; --bg-content:#FFFFFF; --bg-gutter:#ECEFF0;
  --bg-status:#DFE4EA; --bg-sidebar:#DEE0E2; --bg-selsoft:#9CC3EA;
  --line:#D4D7D8; --line-soft:#E0E4E5; --fg:#1C1C1E; --fg2:#4A4A4F; --dim:#8B8B8B;
  --accent:#0078F0; --menu-hl:#228EF4;
  /* 差异语义色 */
  --c-red:#E01E10;   --c-red-bg:#FDE0DF;   /* 删除 · 仅左侧 */
  --c-amber:#A9761A; --c-amber-bg:#FBF0C8; /* 修改行 · 仅右侧 */
  --c-green:#1F7A3D; --c-green-bg:#CCE1D8; /* 新增 */
  --c-blue:#0A63C9;  --c-blue-bg:#DCE9FA;  /* 已修改(文件夹) · 选中 */
  --c-violet:#6A3FA0;--c-violet-bg:#EFE6F8;/* 二进制不同 */
  --img-canvas:#292821; --mask-l:#E13C32; --mask-r:#FFC83C;
  --font:-apple-system,"SF Pro Text","Helvetica Neue","PingFang SC",sans-serif;
  --mono:"SF Mono",ui-monospace,Menlo,Consolas,monospace;
}
*{box-sizing:border-box;margin:0;padding:0}
body{background:#7E7E82;overflow-x:hidden;font-family:var(--font);color:var(--fg);
  -webkit-font-smoothing:antialiased;text-rendering:optimizeLegibility}
html{--fit:1}
.sheet{width:2560px;margin:0 auto;padding:40px 0 60px;zoom:var(--fit)}
.art{width:2560px;height:1320px;background:#fff;position:relative;overflow:hidden;
  box-shadow:0 18px 60px -20px rgba(0,0,0,.45)}
.art+.art{margin-top:56px}
.screen{position:absolute;inset:0;display:flex;flex-direction:column}

/* ① 顶部菜单栏 30px */
.mbar{height:30px;flex:0 0 auto;display:flex;align-items:center;gap:1px;padding:0 8px;
  background:rgba(246,246,247,.94);border-bottom:1px solid #CBCBCB;font-size:13px}
.mbar .mi{padding:2px 10px;border-radius:5px;color:#1C1C1E;white-space:nowrap}
.mbar .app{font-weight:700}
.mbar .mi.on{background:var(--menu-hl);color:#fff}
.mbar .mi.dim{color:#B4B4B6}
.mbar .mright{margin-left:auto;display:flex;align-items:center;gap:14px;color:#3A3A3C;font-size:12.5px}
.mbar .mright svg{width:15px;height:15px}

/* ② 会话标签页栏 40px */
.tabbar{height:40px;flex:0 0 auto;display:flex;align-items:flex-end;gap:3px;padding:0 12px;
  background:var(--bg-window);border-bottom:1px solid var(--line)}
.lights{display:flex;gap:8px;align-self:center;margin-right:10px}
.lights i{width:12px;height:12px;border-radius:50%;display:block}
.tab{height:29px;display:flex;align-items:center;gap:7px;padding:0 12px;font-size:12.5px;color:#55555A;
  border:1px solid transparent;border-bottom:0;border-radius:5px 5px 0 0;white-space:nowrap}
.tab .x{color:#9A9A9E;font-size:12px}
.tab.on{background:#fff;border-color:var(--line);color:var(--fg);font-weight:600;
  margin-bottom:-1px;padding-bottom:1px}
.tab .k{width:13px;height:13px;border-radius:3px;flex:0 0 auto}
.tab .m{font-family:var(--mono);font-size:11px;color:var(--dim);font-weight:400}
.tab.on .m{color:#6E6E73}

/* ③ 工具栏 56px */
.toolbar{height:56px;flex:0 0 auto;display:flex;align-items:center;padding:0 10px;
  background:var(--bg-window);border-bottom:1px solid var(--line)}
.tgroup{display:flex;align-items:center;gap:1px;height:100%;padding:0 7px}
.tgroup+.tgroup{border-left:1px solid var(--line-soft)}
.tbtn{width:62px;height:46px;display:flex;flex-direction:column;align-items:center;justify-content:center;
  gap:3px;border-radius:5px;font-size:10.5px;color:var(--fg2);white-space:nowrap}
.tbtn svg{width:19px;height:19px;stroke:currentColor;fill:none;stroke-width:1.5;
  stroke-linecap:round;stroke-linejoin:round}
.tbtn.on{background:#D3E3F8;color:#0A4FA0}
.tbtn.off{color:#B7B7BB}
.seg{display:flex;height:26px;border:1px solid var(--line);border-radius:5px;overflow:hidden;background:#fff}
.seg b{display:flex;align-items:center;padding:0 11px;font-size:11.5px;font-weight:400;color:var(--fg2);
  border-right:1px solid var(--line-soft)}
.seg b:last-child{border-right:0}
.seg b.on{background:var(--accent);color:#fff;font-weight:600}

/* ④ 内容区 */
.content{flex:1 1 auto;min-height:0;display:flex;flex-direction:column;background:var(--bg-content)}

/* 两栏文件头（BC 式：路径行 + 明细行） */
.chead{display:grid;grid-template-columns:1fr 22px 1fr;flex:0 0 auto;
  background:var(--bg-panel);border-bottom:1px solid var(--line);padding:7px 0}
.chead .side{display:flex;flex-direction:column;gap:2px;padding:0 12px;min-width:0}
.chead .p{display:flex;align-items:center;gap:7px;font-family:var(--mono);font-size:12px;
  white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.chead .p svg{width:14px;height:14px;flex:0 0 auto;stroke:var(--accent);fill:none;stroke-width:1.5}
.chead .d{font-family:var(--mono);font-size:11px;color:var(--dim);padding-left:21px}
.chead .mid{display:grid;place-items:center;color:var(--dim)}

/* 文本比较 */
.tcode{flex:1 1 auto;min-height:0;display:grid;grid-template-columns:1fr 76px}
.trows{display:flex;flex-direction:column;overflow:hidden}
.trow{flex:0 0 auto;display:grid;grid-template-columns:56px 1fr 1px 56px 1fr;min-height:20px;font-family:var(--mono);
  font-size:12px;line-height:20px}
.trow .g{text-align:right;padding:0 8px;color:var(--dim);background:var(--bg-gutter);font-size:11px}
.trow .g.cur{color:var(--accent);font-weight:600}
.trow .c{padding:0 10px;white-space:pre;overflow:hidden}
.trow .sp{background:var(--line-soft)}
.trow.mod .c.l,.trow.mod .c.r{background:var(--c-amber-bg)}
.trow.mod .c.l{color:var(--c-amber)}.trow.mod .c.r{color:var(--c-amber)}
.trow.del .c.l,.trow.del .g.l{background:var(--c-red-bg)}
.trow.del .c.l{color:var(--c-red)}
.trow.ins .c.r,.trow.ins .g.r{background:var(--c-green-bg)}
.trow.ins .c.r{color:var(--c-green)}
.trow.mod .g.l,.trow.mod .g.r{background:#F1E5C8}
.trow.gap .c{background:var(--bg-gutter);
  background-image:repeating-linear-gradient(45deg,transparent 0 3px,rgba(0,0,0,.055) 3px 4px)}
.trow.cur .c.l{box-shadow:inset 3px 0 0 var(--accent)}
.trow .hl{background:rgba(169,118,26,.22);border-radius:2px}
.eof{flex:1 1 auto;min-height:36px;background:#fff;background-image:repeating-linear-gradient(45deg,transparent 0 3px,rgba(0,0,0,.05) 3px 4px),repeating-linear-gradient(-45deg,transparent 0 3px,rgba(0,0,0,.05) 3px 4px)}
/* 差异导航小地图 */
.minimap{background:var(--bg-panel);border-left:1px solid var(--line);position:relative;padding:8px 0}
.minimap .blk{position:absolute;left:16px;right:16px;border-radius:2px}
.minimap .vp{position:absolute;left:4px;right:4px;border:1px solid var(--accent);border-radius:3px;
  background:rgba(0,122,240,.07)}
.minimap .cap{position:absolute;bottom:6px;left:0;right:0;text-align:center;font-size:9.5px;color:var(--dim)}

/* 文件夹比较 */
.dhead{display:grid;grid-template-columns:1fr 22px 1fr;flex:0 0 auto;background:var(--bg-panel);
  border-bottom:1px solid var(--line);height:24px;font-size:11.5px;color:var(--fg2)}
.dhead .grp,.drow .cell{display:grid;grid-template-columns:18px 1fr 76px 132px 96px;align-items:center;
  gap:8px;padding:0 12px 0 6px}
.dhead .grp{padding-left:6px}
.dhead .num,.drow .sz,.drow .dt,.drow .at{text-align:right;font-family:var(--mono);font-size:11px;color:var(--dim)}
.dhead .at{text-align:left}
.dhead .mid{background:var(--bg-gutter);border-left:1px solid var(--line);border-right:1px solid var(--line)}
.drows{overflow:hidden}
.drow{display:grid;grid-template-columns:1fr 22px 1fr;height:26px;border-bottom:1px solid #F0F2F3}
.drow .mid{background:var(--bg-gutter)}
.drow .mid b{display:block;width:3px;height:100%;margin:0 auto}
.drow .cell{position:relative}
.dcell-ico{width:13px;height:12px;flex:0 0 auto;justify-self:center}
.dcell-ico.dir{clip-path:polygon(0 26%,42% 26%,52% 0,100% 0,100% 100%,0 100%);background:#9FC7EC}
.dcell-ico.file{clip-path:polygon(0 0,68% 0,100% 26%,100% 100%,0 100%);background:#C9CDD2}
.drow .nm{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12.5px}
.drow.l .nm{color:var(--c-red)}.drow.l{background:var(--c-red-bg)}
.drow.r .nm{color:var(--c-amber)}.drow.r{background:var(--c-amber-bg)}
.drow.m .nm{color:var(--c-blue)}.drow.m{background:var(--c-blue-bg)}
.drow.b .nm{color:var(--c-violet)}.drow.b{background:var(--c-violet-bg)}
.drow.sel{background:var(--accent)}
.drow.sel .nm,.drow.sel .sz,.drow.sel .dt,.drow.sel .at{color:#fff}
.drow.sel .dcell-ico.file{background:rgba(255,255,255,.85)}
.drow .tag{font-family:var(--mono);font-size:9.5px;padding:0 4px;border-radius:3px;
  border:1px solid currentColor;margin-left:6px}
.drow.sel .tag{color:#fff}

/* 十六进制 */
.hexwrap{flex:1 1 auto;min-height:0;display:grid;grid-template-columns:1fr 22px 1fr}
.hexcol{overflow:hidden}
.hexrow{display:grid;grid-template-columns:88px 1fr 178px;gap:14px;padding:0 12px;
  font-family:var(--mono);font-size:12px;line-height:20px;height:20px;white-space:pre}
.hexrow.head{background:var(--bg-panel);border-bottom:1px solid var(--line);color:var(--dim);font-size:10.5px}
.hexrow .a{color:#8A8A8E}
.hexrow.head .a,.hexrow.head .h,.hexrow.head .asc{background:none;color:var(--dim)}
.hexrow .h{color:#2C6FB5}
.hexrow .asc{color:#5A5A5E}
.hexrow.diff{background:var(--c-red-bg)}
.hexrow.diff .h,.hexrow.diff .asc{color:var(--c-red)}
.hexrow.diff .b{background:#F6BDB8;border-radius:2px;font-weight:600}
.hexrow .b.ins{background:var(--c-green-bg);color:var(--c-green);border-radius:2px;font-weight:600}
.hexwrap>.mid{background:var(--bg-gutter);border-left:1px solid var(--line);border-right:1px solid var(--line)}

/* 表格比较 */
.tablewrap{flex:1 1 auto;min-height:0;display:grid;grid-template-columns:1fr 22px 1fr}
.tbl{border-collapse:collapse;font-family:var(--mono);font-size:12px;width:100%}
.tbl th,.tbl td{border-right:1px solid var(--line-soft);border-bottom:1px solid var(--line-soft);
  padding:2px 9px;text-align:left;white-space:nowrap}
.tbl thead th{background:var(--bg-panel);font-weight:600;font-size:11.5px}
.tbl tr.letters th{background:var(--bg-panel);color:var(--dim);text-align:center;height:19px;font-size:10.5px}
.tbl tr.letters th.dc{color:var(--c-red)}
.tbl tr.letters th.cur{background:#D3E3F8;color:var(--accent);font-weight:700}
.tbl td.rn,.tbl th.rn{background:var(--bg-gutter);color:var(--dim);text-align:right;width:34px}
.tbl td.mk,.tbl th.mk{width:16px;text-align:center;color:var(--dim);font-size:10px}
.tbl tr.m td.mk{color:var(--c-violet)}
.tbl tr.i td.mk{color:var(--c-green)}
.tbl tr.d td.mk{color:var(--c-red)}
.tbl tr.mod td{background:var(--c-amber-bg)}
.tbl tr.mod td.cd{background:#F3D79B;color:var(--c-red);font-weight:600}
.tbl tr.d td{background:var(--c-red-bg)}
.tbl tr.i td{background:var(--c-green-bg)}
.tbl tr.selcell td.cur{box-shadow:inset 0 0 0 2px var(--accent)}

/* 媒体 / 元数据 */
.metawrap{flex:1 1 auto;min-height:0;display:grid;grid-template-columns:1fr 22px 1fr}
.metacol{padding:10px 12px;overflow:hidden}
.mtable{width:100%;border-collapse:collapse;font-size:12.5px}
.mtable th{background:#D3E3F8;border-bottom:1px solid var(--line);text-align:left;padding:4px 9px;
  font-size:11.5px;color:#123B6B}
.mtable td{padding:4px 9px;border-bottom:1px solid var(--line-soft);font-family:var(--mono);font-size:12px}
.mtable td.k{width:38%;font-family:var(--font);font-size:12.5px;color:var(--fg2)}
.mtable tr.d td{background:var(--c-red-bg);color:var(--c-red)}
.wave{height:132px;margin-top:10px;border:1px solid var(--line);border-radius:4px;background:#FAFBFB;position:relative;overflow:hidden}
.wave .wv{position:absolute;inset:0;background:
  repeating-linear-gradient(90deg,rgba(31,122,61,.75) 0 1px,transparent 1px 4px)}
.wave .wv.m{background:repeating-linear-gradient(90deg,rgba(224,30,16,.7) 0 1px,transparent 1px 4px);opacity:.85}
.wave .ax{position:absolute;left:0;right:0;top:50%;height:1px;background:var(--line)}

/* 文本合并（三栏 左/右/输出） */
.mwrap{flex:1 1 auto;min-height:0;display:grid;grid-template-columns:1fr 1fr 1fr}
.mcol{display:flex;flex-direction:column;min-width:0;border-right:1px solid var(--line)}
.mcol.out{border-right:0;background:#FCFCFD}
.mhead{height:24px;flex:0 0 auto;display:flex;align-items:center;gap:8px;padding:0 10px;
  background:var(--bg-panel);border-bottom:1px solid var(--line);font-size:11.5px;font-weight:600}
.mhead .st{font-weight:400;font-size:10.5px;padding:0 5px;border-radius:3px;color:#fff}
.mbody{flex:1 1 auto;min-height:0;overflow:hidden}
.mrow{display:grid;grid-template-columns:46px 1fr;min-height:20px;font-family:var(--mono);
  font-size:12px;line-height:20px}
.mrow .g{text-align:right;padding:0 8px;color:var(--dim);background:var(--bg-gutter);font-size:11px}
.mrow .c{padding:0 9px;white-space:pre;overflow:hidden}
.mrow.cf .c{background:var(--c-amber-bg)}
.mrow.cf .g{background:#F1E5C8}
.mrow.sel .c{box-shadow:inset 0 0 0 2px var(--accent)}
.mbar2{height:26px;flex:0 0 auto;display:flex;align-items:center;gap:6px;padding:0 8px;
  background:var(--bg-panel);border-top:1px solid var(--line);font-size:11px;color:var(--fg2)}
.mbang{display:flex;align-items:center;gap:6px;height:22px;padding:0 8px;border-radius:4px;
  background:var(--accent);color:#fff;font-size:11px}
.mbang.g{background:#5A9E6F}

/* 图片比较 */
.imgwrap{flex:1 1 auto;min-height:0;display:grid;grid-template-columns:1fr 22px 1fr;background:var(--img-canvas)}
.imgpane{display:grid;place-items:center;padding:24px}
.canvas{position:relative;width:520px;height:333px;background:#EDEDED;border:1px solid #101010;
  box-shadow:0 8px 26px rgba(0,0,0,.5)}
.canvas i{position:absolute;display:block}
.canvas .shot{display:block;width:100%;height:100%;object-fit:contain}
.canvas .mask{position:absolute;inset:0;pointer-events:none}
.canvas .mask u{position:absolute;display:block}

/* 状态栏 48px */
.status{flex:0 0 auto;background:var(--bg-status);border-top:1px solid var(--line);font-size:11.5px}
.srow{height:24px;display:flex;align-items:center}
.srow+.srow{border-top:1px solid #CDD3DA}
.scell{display:flex;align-items:center;gap:7px;padding:0 13px;height:100%;white-space:nowrap}
.scell+.scell{border-left:1px solid #C7CDD4}
.scell.grow{flex:1 1 auto;justify-content:center;color:var(--fg2)}
.scell .x{color:var(--c-red);font-weight:700}
.scell .num{font-family:var(--mono)}
.sw{width:10px;height:10px;border-radius:3px;display:inline-block}
.chk{width:12px;height:12px;border:1px solid #9AA0A8;border-radius:3px;display:inline-block;background:#fff}
.chk.on{background:var(--accent);border-color:var(--accent);position:relative}
.chk.on::after{content:"";position:absolute;left:3px;top:0.5px;width:4px;height:7px;
  border-right:1.6px solid #fff;border-bottom:1.6px solid #fff;transform:rotate(40deg)}
.split2{display:grid;grid-template-columns:1fr 1fr;flex:1 1 auto;height:100%}
.split2>.scell+.scell{border-left:1px solid #C7CDD4}
.pane-ft{display:grid;grid-template-columns:1fr 1fr;height:100%}
.pane-ft .scell{border-left:0}
.pane-ft .scell+.scell{border-left:1px solid #C7CDD4}

/* 菜单展开态 */
.menu{position:absolute;min-width:262px;background:rgba(246,246,247,.97);border:1px solid rgba(0,0,0,.14);
  border-radius:8px;padding:4px;box-shadow:0 16px 40px -8px rgba(0,0,0,.34),0 2px 8px -2px rgba(0,0,0,.2)}
.menu .it{display:flex;align-items:center;height:22px;padding:0 12px 0 8px;border-radius:5px;
  font-size:13px;color:#1C1C1E}
.menu .it .tk{width:15px;flex:0 0 auto;color:var(--menu-hl);font-size:12px}
.menu .it .lb{flex:1 1 auto;white-space:nowrap}
.menu .it .kb{margin-left:auto;padding-left:26px;color:#6E6E73;font-size:12.5px;letter-spacing:.06em}
.menu .it .ar{margin-left:auto;padding-left:22px;color:#3A3A3C;font-size:13px}
.menu .it.hl{background:var(--menu-hl);color:#fff}
.menu .it.hl .kb,.menu .it.hl .ar{color:rgba(255,255,255,.82)}
.menu .it.dis{color:#B0B0B2}
.menu .sep{height:1px;background:rgba(0,0,0,.13);margin:5px 10px}
.fly{position:absolute;left:100%;top:0;margin-left:-4px;min-width:190px;background:rgba(240,240,241,.98);
  border:1px solid rgba(0,0,0,.14);border-radius:8px;padding:4px;
  box-shadow:0 16px 40px -8px rgba(0,0,0,.34),0 2px 8px -2px rgba(0,0,0,.2)}

/* 注释标签（规格稿用） */
.note{position:absolute;background:#FFF7D6;border:1px solid #E0C46A;border-radius:6px;
  padding:9px 12px;font-size:12.5px;line-height:1.55;color:#4A3B08;max-width:330px;
  box-shadow:0 6px 18px -6px rgba(0,0,0,.28)}
.note b{color:#8A6B00}
.note .bar{position:absolute;height:2px;background:#E0C46A}
.tag-num{display:inline-grid;place-items:center;width:17px;height:17px;border-radius:50%;
  background:#C8452F;color:#fff;font-size:11px;font-weight:700;margin-right:6px;vertical-align:-3px}

/* 规格页（index / tokens / components / status bar） */
.doc{width:1800px;margin:0 auto;padding:56px 0 90px;color:#1C1C1E;zoom:var(--fit)}
.doc h1{font-size:34px;letter-spacing:-.02em;margin-bottom:6px}
.doc h2{font-size:20px;margin:44px 0 14px;padding-bottom:8px;border-bottom:1px solid var(--line)}
.doc p.lead{color:var(--fg2);font-size:14px;line-height:1.7;max-width:1040px}
.doc .grid{display:grid;gap:16px}
.doc .cards{grid-template-columns:repeat(4,1fr)}
.doc a.card{display:block;background:#fff;border:1px solid var(--line);border-radius:8px;padding:16px 18px;
  text-decoration:none;color:inherit}
.doc a.card:hover{border-color:var(--accent)}
.doc a.card .n{font-family:var(--mono);font-size:11px;color:var(--dim)}
.doc a.card .t{font-size:15px;font-weight:600;margin:5px 0 4px}
.doc a.card .d{font-size:12.5px;color:var(--fg2);line-height:1.55}
table.spec{width:100%;border-collapse:collapse;background:#fff;font-size:13px}
table.spec th,table.spec td{border:1px solid var(--line);padding:7px 11px;text-align:left;vertical-align:top}
table.spec th{background:var(--bg-panel);font-weight:600;font-size:12.5px}
table.spec td.mono{font-family:var(--mono);font-size:12px}
.swatch{display:inline-block;width:38px;height:20px;border-radius:4px;border:1px solid rgba(0,0,0,.16);
  vertical-align:-5px;margin-right:8px}
.rowset{display:flex;gap:26px;flex-wrap:wrap;background:#fff;border:1px solid var(--line);
  border-radius:8px;padding:18px 20px}
.rowset div{font-family:var(--mono);font-size:11.5px;color:var(--fg2)}
.strip{background:#fff;border:1px solid var(--line);border-radius:8px;overflow:hidden}
.strip .cap{display:flex;align-items:center;gap:10px;padding:9px 14px;background:var(--bg-panel);
  border-bottom:1px solid var(--line);font-size:13px}
.strip .cap b{font-size:13.5px}
.strip .cap .n{font-family:var(--mono);font-size:11px;color:var(--dim)}
.strip .cap .d{margin-left:auto;font-size:12px;color:var(--fg2)}
.strip .body{padding:0}
.artlabel{font-size:13px;color:#F2F2F2;padding:0 0 10px 2px;font-family:var(--mono)}
"""

# ─────────────────────────────────────────────────────────── icons
def ic(d): return ('<svg viewBox="0 0 24 24">' + "".join('<path d="%s"/>' % p for p in d) + "</svg>")
I = {
 "home": ic(["M3 11l9-7 9 7v9H3z"]),
 "session": ic(["M3 7h18v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", "M9 7V4h6v3"]),
 "open": ic(["M3 6h6l2 2h10v10H3z"]),
 "save": ic(["M5 4h11l3 3v13H5z", "M9 4v5h6"]),
 "copyL": ic(["M20 12H6", "M11 6l-6 6 6 6"]),
 "copyR": ic(["M4 12h14", "M13 6l6 6-6 6"]),
 "all": ic(["M12 3v18", "M3 12h18"]),
 "diff": ic(["M5 8l6 4-6 4", "M19 8l-6 4 6 4"]),
 "same": ic(["M5 12l4 4 10-10"]),
 "expand": ic(["M4 9l8-5 8 5", "M4 15l8 5 8-5"]),
 "collapse": ic(["M4 19l8 5 8-5", "M4 9l8-5 8 5"]),
 "select": ic(["M4 6h10", "M4 12h10", "M4 18h10", "M17 8l3 4-3 4"]),
 "file": ic(["M6 3h8l4 4v14H6z", "M14 3v4h4", "M9 14l2 2 4-4"]),
 "refresh": ic(["M20 11A8 8 0 1 0 18 17", "M20 5v6h-6"]),
 "swap": ic(["M7 4l-3 4 3 4", "M4 8h13", "M17 12l3 4-3 4", "M20 16H7"]),
 "stop": ic(["M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M9.5 9.5l5 5", "M14.5 9.5l-5 5"]),
 "prev": ic(["M12 19V6", "M6 12l6-6 6 6"]),
 "next": ic(["M12 5v13", "M6 12l6 6 6-6"]),
 "find": ic(["M11 5a6 6 0 1 0 0 12 6 6 0 0 0 0-12z", "M20 20l-4-4"]),
 "layout": ic(["M4 4h16v16H4z", "M12 4v16"]),
 "wrap": ic(["M4 7h16", "M4 12h11a3 3 0 0 1 0 6h-2", "M15 16l-2 2 2 2"]),
 "ws": ic(["M5 6h14", "M5 12h14", "M5 18h9"]),
 "syntax": ic(["M9 6L4 12l5 6", "M15 6l5 6-5 6"]),
 "zoomin": ic(["M11 5a6 6 0 1 0 0 12 6 6 0 0 0 0-12z", "M11 8v6", "M8 11h6"]),
 "zoomout": ic(["M11 5a6 6 0 1 0 0 12 6 6 0 0 0 0-12z", "M8 11h6"]),
 "fit": ic(["M4 9V4h5", "M20 9V4h-5", "M4 15v5h5", "M20 15v5h-5"]),
 "rotate": ic(["M5 12a7 7 0 1 1 2 5", "M5 6v6h6"]),
 "flip": ic(["M12 4v16", "M8 8l-4 4 4 4", "M16 8l4 4-4 4"]),
 "tolerance": ic(["M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M12 4v16"]),
 "range": ic(["M4 4h16v16H4z", "M4 12h16", "M12 4v16"]),
 "blend": ic(["M4 4h16v16H4z", "M12 4a8 8 0 0 1 0 16"]),
 "play": ic(["M8 5v14l11-7z"]),
 "wave": ic(["M3 12h3l2-6 3 12 3-9 2 3h5"]),
 "meta": ic(["M4 5h16v14H4z", "M4 10h16", "M9 10v9"]),
 "undo": ic(["M9 8H5V4", "M5 8a8 8 0 1 1 2 9"]),
 "redo": ic(["M15 8h4V4", "M19 8a8 8 0 1 0-2 9"]),
 "cut": ic(["M6 5l12 12", "M18 5L6 17", "M6 18a2 2 0 1 0 0 1z"]),
 "close": ic(["M6 6l12 12", "M18 6L6 18"]),
 "folder": ic(["M3 6h6l2 2h10v11H3z"]),
 "sync": ic(["M4 9a8 8 0 0 1 14-3", "M20 15a8 8 0 0 1-14 3", "M18 3v6h-6", "M6 21v-6h6"]),
 "textedit": ic(["M6 3h8l4 4v14H6z", "M14 3v4h4", "M9 12h6", "M9 16h6"]),
}

# ─────────────────────────────────────────────────────────── chrome
def menubar(title, open_idx=None, dim_idx=()):
    items = ["Beyond Compare", "会话", "文件", "编辑", "视图", "工具", "窗口", "帮助"]
    out = []
    for i, t in enumerate(items):
        cls = "mi app" if i == 0 else "mi"
        if open_idx == i: cls += " on"
        if i in dim_idx: cls += " dim"
        out.append('<span class="%s">%s</span>' % (cls, t))
    right = ('<span class="mright">'
             + ic(["M12 3a5 5 0 0 1 5 5v3H7V8a5 5 0 0 1 5-5z", "M4 11h16v9H4z"]) 
             + ic(["M4 12a4 4 0 0 1 4-4h8a4 4 0 0 1 4 4v6H4z"])
             + '<span>周二 09:41</span></span>')
    return '<div class="mbar">' + "".join(out) + right + '</div>'

def tabbar(tabs, active=0):
    out = ['<div class="tabbar"><span class="lights"><i style="background:#FF5F57"></i>'
           '<i style="background:#FEBC2E"></i><i style="background:#28C840"></i></span>']
    for i, (label, meta, color) in enumerate(tabs):
        on = " on" if i == active else ""
        out.append('<span class="tab%s"><span class="k" style="background:%s"></span>%s'
                   '<span class="m">%s</span><span class="x">✕</span></span>' % (on, color, label, meta))
    return "".join(out) + '</div>'

def toolbar(groups, seg=None):
    out = ['<div class="toolbar">']
    for g in groups:
        out.append('<div class="tgroup">')
        for b in g:
            name, label, state = b[0], b[1], (b[2] if len(b) > 2 else "")
            out.append('<span class="tbtn %s">%s<span>%s</span></span>' % (state, I[name], label))
        out.append('</div>')
    if seg:
        out.append('<div class="tgroup"><span class="seg">' + "".join(
            '<b class="%s">%s</b>' % ("on" if i == seg[1] else "", s) for i, s in enumerate(seg[0])) + '</span></div>')
    return "".join(out) + '</div>'

def status(common, extra=None):
    c = ['<div class="status">']
    c.append('<div class="srow">')
    for cell in common:
        c.append('<span class="scell %s">%s</span>' % (cell[0], cell[1]))
    c.append('</div>')
    if extra is not None:
        c.append('<div class="srow">' + extra + '</div>')
    c.append('</div>')
    return "".join(c)

def page(title, body, extra_css=""):
    return ('<!doctype html>\n<html lang="zh-CN">\n<head>\n<meta charset="utf-8">\n'
            '<title>%s</title>\n<style>%s%s</style>\n</head>\n<body>\n%s\n'
            '<script>\n(function(){var base=null;function fit(){var el=document.querySelector(".sheet")'
            '||document.querySelector(".doc");if(!el)return;if(base===null)base=el.offsetWidth||2560;'
            'var k=Math.min(1,(window.innerWidth-32)/base);document.documentElement.style.setProperty('
            '"--fit",k);}fit();window.addEventListener("resize",fit);})();\n</script>\n</body>\n</html>\n'
            % (H.escape(title), CSS, extra_css, body))

def art(inner, cls=""): return '<div class="art %s"><div class="screen">%s</div></div>' % (cls, inner)

TABS_ALL = [
    ("主页", "会话中心", "#2C7BE5"),
    ("文件夹比较", "webapp-v1 ⇄ webapp-v2", "#2C7BE5"),
    ("文本比较", "order_main_v1.c", "#2C7BE5"),
    ("图片比较", "logo_v1.png", "#2C7BE5"),
    ("十六进制比较", "firmware_v1.bin", "#B3271E"),
    ("表格比较", "staff_v1.csv", "#2C7BE5"),
    ("媒体比较", "tone_v1.wav", "#1F7A3D"),
    ("文本合并", "app_v1.conf", "#C87C1E"),
]

STD_COMMON = [
    ("", '<span class="x">✗</span>15 个差异部分'),
    ("", '<span class="chk"></span>忽略的不重要差异'),
    ("", '插入 <span style="color:#8B8B8B">▾</span>'),
    ("grow", "忽略行尾空白 · 忽略大小写"),
    ("", '加载时间: <span class="num">0.05</span> 秒'),
]

def two_pane_head(lp, ld, rp, rd):
    return ('<div class="chead">'
            '<div class="side"><span class="p">%s%s</span><span class="d">%s</span></div>'
            '<span class="mid">%s</span>'
            '<div class="side"><span class="p">%s%s</span><span class="d">%s</span></div>'
            '</div>' % (I["file"], lp, ld, I["swap"], I["file"], rp, rd))

def write(path, content):
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)
    print("  %-42s %5d 行" % (path, content.count("\n") + 1))

# ─────────────────────────────────────────────── 通用工具条组合
TB_COMMON_L = [("home","主页"),("session","会话")]
TB_COMMON_R = [("refresh","刷新"),("swap","交换")]
SEG_LAYOUT = (["边并排","上-下","缩略图"], 0)

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
    IMG_TAG = ('<img class="shot" src="../assets/%s" width="1000" height="640" '
               'alt="%s 预览">')
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
               + '<div class="imgwrap">'
               + '<div class="imgpane"><div class="canvas">' + (IMG_TAG % ("logo_v1.png", "logo_v1.png")) + '</div></div>'
               + '<div class="mid"></div>'
               + '<div class="imgpane"><div class="canvas">' + (IMG_TAG % ("logo_v2.png", "logo_v2.png")) + mask + '</div></div>'
               + '</div></div>')
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

# ══════════════════════════════════════════════ 菜单展开态
MI_CSS = """
.mbar .wrap{position:relative;display:flex;align-items:stretch}
.menu.anchored{position:absolute;top:26px;left:0;z-index:60}
.note.nt{background:#FFF7D6}
.note .k{display:block;margin-top:5px;font-family:var(--mono);font-size:11.5px;color:#7A5E00}
"""
def mbar_menus(open_idx, menus):
    items = ["Beyond Compare", "会话", "文件", "编辑", "视图", "工具", "窗口", "帮助"]
    out = []
    for i, t in enumerate(items):
        cls = "mi app" if i == 0 else "mi"
        if i == open_idx: cls += " on"
        inner = ('<span class="wrap"><span class="%s">%s</span>%s</span>'
                 % (cls, t, menus.get(i, "")))
        out.append(inner)
    return ('<div class="mbar">' + "".join(out)
            + '<span class="mright">' + I["home"] + I["find"] + '<span>周二 09:41</span></span></div>')

def menu(items, cls="menu anchored"):
    out = ['<div class="%s">' % cls]
    for it in items:
        if it[0] == "s":
            out.append('<div class="sep"></div>')
        elif it[0] == "f":                       # 子菜单父项
            out.append('<div class="it %s"><span class="tk"></span><span class="lb">%s</span>'
                       '<span class="ar">›</span><span class="fly">%s</span></div>'
                       % (it[3] if len(it) > 3 else "", it[1],
                          "".join('<div class="it"><span class="tk"></span>'
                                  '<span class="lb">%s</span></div>' % x for x in it[2])))
        else:
            lbl, kb, state = it[1], (it[2] if len(it) > 2 else ""), (it[3] if len(it) > 3 else "")
            tick = '<span class="tk">%s</span>' % ("✓" if state == "on" else "")
            kbs = '<span class="kb">%s</span>' % kb if kb else ""
            out.append('<div class="it %s">%s<span class="lb">%s</span>%s</div>'
                       % ("dis" if state == "dis" else ("hl" if state == "hl" else ""), tick, lbl, kbs))
    return "".join(out) + '</div>'

def note(html, left, top, width=330):
    return '<div class="note" style="left:%dpx;top:%dpx;width:%dpx">%s</div>' % (left, top, width, html)

def page_menus():
    # ① 会话 / 窗口（单标签置灰）
    m_sess = menu([
        ("f", "新建会话", ["文件夹比较", "文件夹合并", "文件夹同步", "文本比较", "文本合并",
                        "16 进制比较", "媒体比较", "图片比较", "表格比较"], "hl"),
        ("i", "新建标签页", "⌘T"), ("i", "新建窗口", "⌘N"), ("i", "打开会话…", "⇧⌘O"), ("s",),
        ("i", "保存会话", "⌘S"), ("i", "保存会话为…"), ("i", "会话设置…", "⌘,"), ("s",),
        ("i", "关闭标签页", "", "dis"), ("i", "关闭其它标签页", "", "dis"), ("s",),
        ("i", "重新比较文件", "⌘R"), ("i", "交换两边"), ("i", "浏览文件夹…"), ("s",),
        ("i", "文件夹比较报告…", "⌘P"), ("i", "文件夹比较信息"), ("s",),
        ("i", "比较父文件夹", "", "dis")])
    m_win = menu([
        ("i", "最小化", "⌘M"), ("i", "最小化全部", "⌥⌘M"), ("i", "缩放"), ("i", "缩放全部"), ("s",),
        ("i", "选择上一个标签页", "⇧⌘[", "dis"), ("i", "选择下一个标签页", "⇧⌘]", "dis"),
        ("i", "移动标签页到新窗口", "", "dis"), ("i", "合并所有窗口", "", "dis"), ("s",),
        ("i", "全部前移"), ("i", "在前面排列")])
    a1 = (menubar("文件夹比较", 1)
          .replace('<span class="mi on">会话</span>', '<span class="wrap"><span class="mi on">会话</span>' + m_sess + '</span>')
          .replace('<span class="mi">窗口</span>', '<span class="wrap"><span class="mi">窗口</span>' + m_win + '</span>'))
    a1 = a1.replace('<div class="mbar">', '<div class="mbar" style="position:relative">')
    content = ('<div class="content" style="background:#EDEFF0">'
               '<div style="padding:24px 32px;font-size:13px;color:#6E6E73">'
               '会话标签页：仅 1 个标签 → 关闭标签页 / 关闭其它标签页 置灰；窗口菜单的标签页操作同样置灰。'
               '</div></div>')
    art1 = (art(a1 + tabbar([TABS_ALL[1]], 0)
                + toolbar([TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差异"), ("same", "显示相同")],
                           [("copyL", "复制到左"), ("copyR", "复制到右")]] )
                + content + status(STD_COMMON, '<span class="scell">3 个文件, 4.1 KB</span>'
                                               '<span class="scell">130 GB 可用</span>'
                                               '<span class="scell grow"></span>'
                                               '<span class="scell">4 个文件, 3.19 KB</span>'
                                               '<span class="scell">130 GB 可用</span>'))
           + note('<b>置灰规则 ①</b>单标签时的会话菜单：<span class="k">关闭标签页 · 关闭其它标签页 · 比较父文件夹</span>', 940, 250, 360)
           + note('<b>置灰规则 ②</b>单标签时的窗口菜单：<span class="k">选择上一个/下一个标签页 · 移动标签页到新窗口 · 合并所有窗口</span>', 940, 620, 360))

    # ② 编辑菜单：只读比较 vs 文本合并
    ed_ro = menu([
        ("i", "撤销", "⌘Z", "dis"), ("i", "重做", "⇧⌘Z", "dis"), ("s",),
        ("i", "对齐方式…", ""), ("i", "隔离"), ("i", "替换内容…"), ("s",),
        ("i", "复制到右边", "⌥→"), ("i", "复制行到右边"), ("s",),
        ("i", "剪切", "⌘X", "dis"), ("i", "复制", "⌘C", "dis"), ("i", "粘贴", "⌘V", "dis"),
        ("i", "删除", "", "dis"), ("s",),
        ("i", "全选", "⌘A", "dis"), ("i", "选择选择内容", "⌘D", "dis"), ("s",),
        ("i", "转换文件", "", "")])
    ED_MERGE = [
        ("i", "撤销", "⌘Z"), ("i", "重做", "⇧⌘Z"), ("s",),
        ("i", "采用左边"), ("i", "采用中心"), ("i", "采用右边"), ("s",),
        ("i", "采用左边然后右边", "⌥B"), ("i", "采用右边然后左边", "⌥B"), ("s",),
        ("i", "剪切", "⌘X"), ("i", "复制", "⌘C"), ("i", "粘贴", "⌘V"), ("i", "删除"), ("s",),
        ("i", "全选", "⌘A"), ("i", "选择选择内容", "⌘D")]
    ed_merge = menu(ED_MERGE)
    a2bar = mbar_menus(3, {3: ed_ro})
    a2bar2 = mbar_menus(3, {3: ed_merge})
    merge_col = ('<div style="width:520px;border-left:1px solid var(--line);background:#F7F8F9;padding:18px 20px">'
                 '<div style="font-size:12.5px;font-weight:600;margin-bottom:10px">文本合并会话 · 编辑菜单（同一位置全部可用）</div>'
                 + menu(ED_MERGE, "menu").replace('class="menu"', 'class="menu" style="position:static;min-width:238px"')
                 + '<div style="font-size:12px;color:#6E6E73;line-height:1.7;margin-top:12px">'
                   '左右栏可编辑 → 撤销 / 重做 / 剪切 / 复制 / 粘贴 / 全选 恢复为黑色可用状态，'
                   '并多出「采用左边 / 中心 / 右边」与顺序采用。</div></div>')
    code = "".join('<div class="trow mod"><span class="g l">%d</span><span class="c l">%s</span>'
                   '<span class="sp"></span><span class="g r">%d</span><span class="c r">%s</span></div>'
                   % (i + 1, H.escape(x), i + 1, H.escape(x)) for i, x in enumerate(
                       ["#define MAX_ORDERS 256", "typedef struct { int id; double total; } Order;",
                        "double calc_total(double s) {", "    return s * (1.0 + TAX_RATE);",
                        "}", "", "int main(void) {", "    printf(\"order v1\");"]))
    a2 = art(a2bar + tabbar(TABS_ALL, 2)
             + toolbar([TB_COMMON_L, [("all", "显示全部", "on"), ("diff", "显示差异"), ("same", "显示相同")],
                        [("copyL", "复制到左"), ("copyR", "复制到右"), ("prev", "上一差异"), ("next", "下一差异")],
                        [("undo", "撤销", "off"), ("redo", "重做", "off"), ("find", "查找")]])
             + '<div class="content" style="flex-direction:row">'
             + '<div class="trows" style="flex:1 1 auto;min-width:0">' + code + '</div>'
             + merge_col + '</div>'
             + status(STD_COMMON, '<div class="pane-ft"><span class="scell"><b class="num">2:1</b> 注释</span>'
                                  '<span class="scell"><b class="num">2:1</b> 注释</span></div>')
             + note('<b>比较会话（只读）</b>编辑菜单：撤销 / 重做 / 剪切 / 复制 / 粘贴 / 删除 / 全选 / 选择选择内容 '
                    '全部置灰；只有「复制到右边 / 对齐方式 / 隔离 / 替换内容 / 转换文件」可用。', 900, 330, 380))
    return art1, a2

def page_view_menus():
    V_IMG = [
        ("i", "显示全部", "1"), ("i", "显示差异", "2"), ("i", "显示相同", "3"), ("s",),
        ("i", "容差模式", "1"), ("i", "不匹配范围模式", "2", "on"), ("i", "混合模式", "3"), ("s",),
        ("i", "忽略不重要差异"), ("i", "自动缩放"), ("s",),
        ("i", "顺时针旋转"), ("i", "逆时针旋转"), ("i", "水平翻转"), ("i", "垂直翻转"),
        ("i", "重置差异偏移", "", "dis"), ("s",),
        ("i", "比较元数据"), ("i", "混合切换"), ("i", "文件信息"), ("i", "全屏"), ("i", "缩放")]
    v_img = menu(V_IMG)
    V_TEXT = [
        ("i", "显示全部", "1", "on"), ("i", "显示差异", "2"), ("i", "显示相同", "3"), ("s",),
        ("i", "可见空白"), ("i", "语法加亮"), ("i", "自动换行"), ("i", "行号"), ("s",),
        ("i", "边并排布局", "on"), ("i", "上-下布局"), ("s",),
        ("i", "缩略图"), ("i", "图例", "L"), ("i", "日志"), ("i", "工具栏")]
    v_text = menu(V_TEXT)
    V_HOME = [
        ("i", "显示会话管理", "", "on"), ("i", "显示网络资源"), ("s",),
        ("i", "工具栏"), ("i", "状态栏"), ("i", "日志")]
    v_home = menu(V_HOME)
    bar = ('<div class="mbar" style="position:relative">'
           '<span class="wrap"><span class="mi app">Beyond Compare</span></span>'
           '<span class="wrap"><span class="mi">会话</span></span>'
           '<span class="wrap"><span class="mi">文件</span></span>'
           '<span class="wrap"><span class="mi">编辑</span></span>'
           '<span class="wrap"><span class="mi on">视图</span></span>'
           '<span class="wrap"><span class="mi">工具</span></span>'
           '<span class="wrap"><span class="mi">窗口</span></span>'
           '<span class="wrap"><span class="mi">帮助</span></span>'
           '<span class="mright">' + I["home"] + I["find"] + '<span>周二 09:41</span></span></div>')
    holder = ('<div class="content" style="background:#EDEFF0;display:block;padding:0">'
              '<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:0;height:100%">'
              '<div style="padding:20px 24px;border-right:1px solid #D8DBDD">'
              '<div style="font-size:13px;font-weight:600;margin-bottom:10px">图片比较 · 视图菜单</div>'
              + menu(V_IMG, "menu").replace('class="menu"', 'class="menu" style="position:static"')
              + '</div>'
              '<div style="padding:20px 24px;border-right:1px solid #D8DBDD">'
              '<div style="font-size:13px;font-weight:600;margin-bottom:10px">文本比较 · 视图菜单</div>'
              + menu(V_TEXT, "menu").replace('class="menu"', 'class="menu" style="position:static"')
              + '</div>'
              '<div style="padding:20px 24px">'
              '<div style="font-size:13px;font-weight:600;margin-bottom:10px">主页 · 视图菜单</div>'
              + menu(V_HOME, "menu").replace('class="menu"', 'class="menu" style="position:static"')
              + '</div></div></div>')
    a3 = art(bar + tabbar(TABS_ALL, 3)
             + toolbar([TB_COMMON_L,
                        [("tolerance", "容差"), ("range", "不匹配范围", "on"), ("blend", "混合")],
                        [("rotate", "旋转"), ("flip", "水平翻转"), ("zoomout", "缩小"), ("zoomin", "放大")],
                        [("prev", "上一差异"), ("next", "下一差异")]])
             + holder
             + status(STD_COMMON, '<span class="scell"><span class="sw" style="background:var(--mask-l)"></span>'
                                  '仅左图有 &nbsp;<span class="sw" style="background:var(--mask-r)"></span>仅右图有</span>'
                                  '<span class="scell grow">不匹配范围模式 · 差异像素 <b class="num">31,842</b></span>')
             + note('<b>整组切换</b>视图菜单随会话类型整体变化：图片比较是「容差 / 不匹配范围 / 混合 + 旋转翻转」，'
                    '文本比较是「可见空白 / 语法加亮 / 自动换行」，主页是「显示会话管理 / 显示网络资源」。', 940, 1060, 620)
             + note('<b>置灰项</b>图片比较中「重置差异偏移」置灰（当前偏移为 0，无可重置）。', 100, 1050, 420))
    return a3

# ══════════════════════════════════════════════ 状态栏组件稿
def page_statusbar():
    strips = []
    def strip(no, name, desc, common, extra, w=2560):
        return ('<div class="strip" style="width:%dpx;margin-bottom:26px">'
                '<div class="cap"><span class="n">%s</span><b>%s</b>'
                '<span class="d">%s</span></div>'
                '<div class="body">%s</div></div>'
                % (w, no, name, desc, status(common, extra)))
    P = lambda *cells: list(cells)
    strips.append(strip("① 通用行", "所有会话类型共用", "差异计数 · 忽略开关 · 编辑模式 · 加载时间",
                        [("", '<span class="x">✗</span>15 个差异部分'),
                         ("", '<span class="chk"></span>忽略的不重要差异'),
                         ("", '插入 <span style="color:#8B8B8B">▾</span>'),
                         ("grow", "忽略行尾空白 · 忽略大小写"),
                         ("", '加载时间: <span class="num">0.05</span> 秒')], None))
    strips.append(strip("② 文件夹比较", "文件夹 / 压缩包",
                        "左：N 个文件, 大小 + 磁盘可用 ｜ 右：同左 ｜ 中：状态维度计数",
                        STD_COMMON,
                        '<span class="scell">3 个文件, 4.1 KB</span>'
                        '<span class="scell">130 GB 可用</span>'
                        '<span class="scell grow" style="color:#6E6E73">仅左 <b class="num">2</b> · 仅右 <b class="num">2</b> · '
                        '已修改 <b class="num">1</b> · 二进制不同 <b class="num">1</b> · 相同 <b class="num">3</b></span>'
                        '<span class="scell">4 个文件, 3.19 KB</span>'
                        '<span class="scell">130 GB 可用</span>'))
    strips.append(strip("③ 文本比较", "文本 / 配置 / 源代码",
                        "每栏：行号:列号 + 语法上下文（注释）",
                        STD_COMMON,
                        '<div class="pane-ft">'
                        '<span class="scell"><b class="num">2:1</b> <span style="color:#6E6E73">注释</span> '
                        '<span style="font-family:var(--mono)">★ main.c · 订单处理入口 · (v1) ¶</span></span>'
                        '<span class="scell"><b class="num">2:1</b> <span style="color:#6E6E73">注释</span> '
                        '<span style="font-family:var(--mono)">★ main.c · 订单处理入口 · (v2) ¶</span></span></div>'))
    strips.append(strip("④ 十六进制比较", "二进制",
                        "差异字节计数 · 当前序号 · 字节序",
                        STD_COMMON,
                        '<span class="scell">458 个差异字节</span><span class="scell">相同 <b class="num">397</b></span>'
                        '<span class="scell grow">当前差异 <b class="num">1/5</b> · F6 / F7 跳转</span>'
                        '<span class="scell">小尾值 <b class="num">0x0D000000</b></span>'))
    strips.append(strip("⑤ 表格比较", "CSV / TSV",
                        "差异行计数 · 当前单元格 · 差异列",
                        STD_COMMON,
                        '<span class="scell">2 个差异行</span><span class="scell">相同 <b class="num">10</b></span>'
                        '<span class="scell grow">A3 = <b class="num">3</b> · Wang Wu</span>'
                        '<span class="scell">2 个差异列（salary, joined）</span>'))
    strips.append(strip("⑥ 图片比较", "PNG / JPG",
                        "遮罩图例 · 差异像素 · 偏移",
                        STD_COMMON,
                        '<span class="scell"><span class="sw" style="background:var(--mask-l)"></span>仅左图有 &nbsp;'
                        '<span class="sw" style="background:var(--mask-r)"></span>仅右图有</span>'
                        '<span class="scell grow">不匹配范围模式 · 差异像素 <b class="num">31,842</b></span>'
                        '<span class="scell">偏移: <b class="num">0.0</b></span>'))
    strips.append(strip("⑦ 媒体比较", "WAV / MP3",
                        "差异项 · 播放进度 · 播放模式",
                        STD_COMMON,
                        '<span class="scell">1 处差异（音频内容）</span>'
                        '<span class="scell grow">播放 00:00:00.860 / 00:00:01.500</span>'
                        '<span class="scell">并列播放</span>'))
    strips.append(strip("⑧ 文本合并", "三栏 左 / 右 / 输出",
                        "冲突计数 · 解决进度 · 输出路径",
                        STD_COMMON,
                        '<span class="scell">2 个冲突区域</span><span class="scell">已解决 2 / 2</span>'
                        '<span class="scell grow">左右栏可编辑 · 输出栏只读</span>'
                        '<span class="scell">输出: /样例文件/out/app.conf</span>'))
    strips.append(strip("⑨ 主页", "会话中心",
                        "视图开关 · 会话数 · 磁盘",
                        [("", '<span class="chk on"></span>显示会话管理'), ("", "显示网络资源"),
                         ("grow", "6 个最近会话 · 磁盘 130 GB 可用"),
                         ("", '加载时间: <span class="num">0.01</span> 秒')], None))
    body = ('<div class="sheet" style="width:2560px">'
            '<div class="doc" style="width:2560px;padding:40px 0 60px">'
            '<h1 style="color:#fff">状态栏 · Status Bar</h1>'
            '<p class="lead" style="color:#E4E4E6">8 种会话各一条。面板底色 <b>#DFE4EA</b>，上边框 <b>#D4D7D8</b>，'
            '行高 24px，字号 11.5px；单元格之间用 1px 竖线 <b>#C7CDD4</b> 分隔，数字走等宽字体。</p>'
            '<h2 style="color:#fff;border-color:#5A5A5E">组件稿</h2></div>'
            + "".join(strips) + '</div>')
    return page("BC 设计稿 · 状态栏组件", body)

# ══════════════════════════════════════════════ 目录 / token / 组件清单
def page_index():
    cards = [
        ("screens/text-compare.html", "01", "文本比较", "双栏 + 行号槽 + 三色差异 + 行对齐留白 + 差异小地图"),
        ("screens/folder-compare.html", "02", "文件夹比较", "名称/大小/已修改/属性 + 仅左·仅右·已修改·二进制不同"),
        ("screens/image-compare.html", "03", "图片比较", "左右预览 + 差异遮罩（红=仅左、黄=仅右）"),
        ("screens/hex-compare.html", "04", "十六进制比较", "偏移列 + 16 字节列 + ASCII 列"),
        ("screens/table-compare.html", "05", "表格比较", "列字母行 + 行状态槽 + 单元格级差异标色"),
        ("screens/media-compare.html", "06", "媒体比较", "属性/元数据对照 + 波形"),
        ("screens/text-merge.html", "07", "文本合并", "三栏 左 / 右 / 输出，左右可编辑"),
        ("screens/home.html", "08", "主页", "左侧最近会话 + 右侧全部会话类型入口"),
        ("screens/menus.html", "09", "菜单展开态", "会话 / 窗口 / 编辑 / 视图，标注置灰项"),
        ("screens/status-bar.html", "10", "状态栏组件稿", "8 种会话各一条"),
    ]
    colors = [
        ("--bg-window", "#F0F4F5", "标题栏 / 标签栏 / 工具栏底", "截图取样"),
        ("--bg-panel", "#F7F9F8", "路径头 / 列头 / 分组头", "截图取样"),
        ("--bg-content", "#FFFFFF", "内容区底", "截图取样"),
        ("--bg-gutter", "#ECEFF0", "行号槽 / 分隔槽", "截图取样"),
        ("--bg-status", "#DFE4EA", "状态栏底", "截图取样"),
        ("--bg-sidebar", "#DEE0E2", "主页左侧栏底", "截图取样"),
        ("--line", "#D4D7D8", "区域分割线 1px", "截图取样"),
        ("--line-soft", "#E0E4E5", "行内细线", "截图取样"),
        ("--fg", "#1C1C1E", "正文 / 菜单可用项", "截图取样"),
        ("--fg2", "#4A4A4F", "次级文字", "截图取样"),
        ("--dim", "#8B8B8B", "禁用 / 辅助文字", "截图取样"),
        ("--accent", "#0078F0", "选中 · 焦点 · 主按钮", "截图取样"),
        ("--menu-hl", "#228EF4", "菜单高亮底（白字）", "截图取样"),
        ("--c-red / -bg", "#E01E10 / #FDE0DF", "删除 · 仅左侧", "截图取样"),
        ("--c-amber / -bg", "#A9761A / #FBF0C8", "修改行 · 仅右侧", "截图取样"),
        ("--c-green / -bg", "#1F7A3D / #CCE1D8", "新增（仅右侧有）", "截图取样"),
        ("--c-blue / -bg", "#0A63C9 / #DCE9FA", "已修改（文件夹）", "取自强调蓝"),
        ("--c-violet / -bg", "#6A3FA0 / #EFE6F8", "二进制不同", "截图取样"),
        ("--img-canvas", "#292821", "图片比较画布底", "截图取样"),
        ("--mask-l / --mask-r", "#E13C32 / #FFC83C", "差异遮罩：仅左 / 仅右", "截图取样"),
    ]
    crows = "".join('<tr><td class="mono">%s</td>'
                    '<td><span class="swatch" style="background:%s"></span><span class="mono">%s</span></td>'
                    '<td>%s</td><td>%s</td></tr>' % (n, v.split(" / ")[0], v, u, src)
                    for n, v, u, src in colors)
    type_scale = [
        ("顶级标题", "34px / 40px", "600", "规格页标题", "—"),
        ("区段标题", "20px / 28px", "600", "规格页小节", "—"),
        ("窗口/菜单文字", "13px / 22px", "400 · 高亮 600", "菜单栏、菜单项、对话框", "菜单行高 22px"),
        ("标签页文字", "12.5px / 29px", "600（当前）/ 400", "会话标签页", "标签高 29px"),
        ("工具栏标签", "10.5px / 12px", "400", "工具栏按钮下方文字", "图标 19px + 间距 3px"),
        ("列表正文", "12.5px / 26px", "400", "文件夹列表行", "行高 26px"),
        ("等宽正文", "12px / 20px", "400", "代码、Hex、CSV、路径", "行高 = 20px，行槽 56px"),
        ("状态栏", "11.5px / 24px", "400 · 数字等宽", "状态栏单元格", "每格内边距 13px"),
        ("辅助/注释", "11px / 16px", "400", "文件明细行、图例", "色值 --dim"),
    ]
    trows = "".join('<tr><td>%s</td><td class="mono">%s</td><td class="mono">%s</td><td>%s</td><td>%s</td></tr>' % r
                    for r in type_scale)
    comps = [
        ("会话标签页", "高 29px，圆角 5px 5px 0 0，当前页白底 + 1px 边框并与内容区相接；每页含类型色块 13px、名称 12.5px、会话对象 11px 等宽、关闭 ✕", "screens/home.html"),
        ("工具栏按钮", "宽 62px 高 46px，图标 19px 描边 1.5px + 文字 10.5px；选中态 #D3E3F8 底 + #0A4FA0 字；置灰 #B7B7BB", "screens/folder-compare.html"),
        ("视图模式切换", "分段控件高 26px，圆角 5px，选中段实心 #0078F0 + 白字；文本/Hex 用「边并排 / 上-下 / 缩略图」，图片用「容差 / 不匹配范围 / 混合」", "screens/text-compare.html"),
        ("差异色条", "每行高度铺满，宽 3px，居中于 22px 分隔槽；红=删除/仅左，琥珀=修改/仅右，绿=新增，蓝=已修改，紫=二进制不同", "screens/folder-compare.html"),
        ("差异小地图", "宽 76px，块内缩 16px、圆角 2px；当前视口框 1px #0078F0 + 7% 填充；底部标签「差异图」9.5px", "screens/text-compare.html"),
        ("状态栏", "高 48px = 两行 24px；通用行 + 类型行；单元格 1px 竖线分隔；数字等宽", "screens/status-bar.html"),
        ("菜单项", "高 22px，圆角 5px，内缩 4px；可用 #1C1C1E，置灰 #B0B0B2，勾选列 15px，快捷键右对齐；高亮 #228EF4 + 白字", "screens/menus.html"),
        ("文件信息头", "高 46px 两行：路径 12px 等宽 + 明细 11px（大小 · 编码 · 行尾 · 时间）", "screens/text-compare.html"),
        ("行对齐留白", "对侧无对应行时用 45° 斜纹填充 #ECEFF0（斜线 rgba(0,0,0,.055)，间距 4px）", "screens/text-compare.html"),
    ]
    krows = "".join('<tr><td><b>%s</b></td><td>%s</td><td class="mono">%s</td></tr>' % c for c in comps)
    body = ('<div class="doc">'
            '<h1>Beyond Compare 5.2.5 · 界面设计稿</h1>'
            '<p class="lead">2560×1320（macOS 1440p）固定画布，顶部菜单栏 30px。'
            '原生 macOS 桌面工具风：浅色、系统字体、紧凑行高、细分割线、低饱和；圆角 4–6px，不使用大卡片阴影。'
            '所有取色均从 14 张截图逐像素采样，未凭印象编造。</p>'
            '<h2>① 八个会话主界面 + 两份专项稿</h2>'
            '<div class="grid cards">'
            + "".join('<a class="card" href="%s"><div class="n">%s</div><div class="t">%s</div>'
                      '<div class="d">%s</div></a>' % c for c in cards)
            + '</div>'
            '<h2>② 设计 Token · 颜色</h2>'
            '<table class="spec"><tr><th>Token</th><th>值</th><th>用途</th><th>来源</th></tr>' + crows + '</table>'
            '<h2>③ 设计 Token · 字号与行高阶梯</h2>'
            '<table class="spec"><tr><th>层级</th><th>字号 / 行高</th><th>字重</th><th>用途</th><th>备注</th></tr>'
            + trows + '</table>'
            '<h2>④ 设计 Token · 间距 / 圆角 / 图标</h2>'
            '<div class="rowset" style="margin-bottom:6px">'
            '<div><b style="color:#1C1C1E">间距</b><br>2 · 4 · 6 · 8 · 12 · 16 · 24 · 32</div>'
            '<div><b style="color:#1C1C1E">内边距</b><br>状态栏格 13px ｜ 菜单项 8/12px ｜ 列表行 12px</div>'
            '<div><b style="color:#1C1C1E">圆角</b><br>3 · 4 · 5 · 6px（标签页仅上圆角）</div>'
            '<div><b style="color:#1C1C1E">图标</b><br>工具栏 19 ｜ 行内 13–14 ｜ 状态栏 12 ｜ 会话类型 30</div>'
            '<div><b style="color:#1C1C1E">描边</b><br>分割线 1px；差异色条 3px；选中框 2px</div>'
            '</div>'
            '<h2>⑤ 组件清单</h2>'
            '<table class="spec"><tr><th>组件</th><th>规格</th><th>示例页</th></tr>' + krows + '</table>'
            '<h2>⑥ 交给编码 Agent 实现</h2>'
            '<p class="lead">本稿是「对齐规格」，不是从零实现——bcr 已有 egui 版本。实现交接文档与机器可读 token：</p>'
            '<div class="grid cards">'
            '<a class="card" href="交接/实现交接.md"><div class="n">HANDOFF</div>'
            '<div class="t">实现交接说明</div><div class="d">设计稿→源码映射表 · token 差异与决策项 · 16 条置灰规则 · '
            'P0–P3 分阶段任务 · 可直接粘贴的提示词</div></a>'
            '<a class="card" href="交接/design-tokens.json"><div class="n">TOKENS</div>'
            '<div class="t">design-tokens.json</div><div class="d">色值 / 字号阶梯 / 间距 / 圆角 / 图标尺寸，'
            '以及菜单状态与 9 条状态栏的机器可读规格</div></a>'
            '<a class="card" href="screens/menus.html"><div class="n">STATES</div>'
            '<div class="t">菜单状态稿</div><div class="d">3 张画板 · 16 个置灰项 · 5 处标注</div></a>'
            '<a class="card" href="screens/status-bar.html"><div class="n">STATUSBAR</div>'
            '<div class="t">状态栏组件稿</div><div class="d">通用行 + 8 种会话各一条</div></a>'
            '</div>'
            '</div>')
    return page("Beyond Compare 5.2.5 · 界面设计稿", body)

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
