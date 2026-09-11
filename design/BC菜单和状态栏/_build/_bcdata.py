# -*- coding: utf-8 -*-
"""从真实样例文件提取内容（确保设计稿里的数据不是编造的）"""
import os, difflib, stat, time, wave, html as _h

S = "/Users/ycy/Downloads/BC界面截图参考/样例文件"
def rd(p): return open(os.path.join(S, p), encoding="utf-8", errors="replace").read()
def fmt(n): return "{:,}".format(n)
def dt(ts): return time.strftime("%Y/%m/%d %H:%M", time.localtime(ts))
def size(n):
    if n >= 1048576: return "%.2f MB" % (n / 1048576)
    if n >= 1024: return "%.2f KB" % (n / 1024)
    return "%d 字节" % n

# ── 文本比较：真实行对齐 ─────────────────────────────────
def _align(a, b):
    sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
    rows, blocks = [], 0
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            for k in range(i2 - i1):
                rows.append(("same", i1 + k + 1, a[i1 + k], j1 + k + 1, b[j1 + k]))
        elif tag == "delete":
            for k in range(i1, i2): rows.append(("del", k + 1, a[k], None, None))
            blocks += 1
        elif tag == "insert":
            for k in range(j1, j2): rows.append(("ins", None, None, k + 1, b[k]))
            blocks += 1
        else:
            n = min(i2 - i1, j2 - j1); blocks += 1
            for k in range(n):
                rows.append(("mod", i1 + k + 1, a[i1 + k], j1 + k + 1, b[j1 + k]))
            for k in range(i1 + n, i2): rows.append(("del", k + 1, a[k], None, None))
            for k in range(j1 + n, j2): rows.append(("ins", None, None, k + 1, b[k]))
    return rows, blocks

def text_data(n=26):
    a = rd("text/order_main_v1.c").split("\n")
    b = rd("text/order_main_v2.c").split("\n")
    rows, blocks = _align(a, b)
    return rows[:n], blocks, len(a), len(b), os.path.getsize(os.path.join(S, "text/order_main_v1.c")), \
           os.path.getsize(os.path.join(S, "text/order_main_v2.c"))

def inline_hl(l, r):
    """把同一行里真正不同的片段标出来（BC 的行内差异高亮）"""
    if l is None or r is None: return l, r
    sm = difflib.SequenceMatcher(None, l, r, autojunk=False)
    lo, ro = [], []
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            lo.append(_h.escape(l[i1:i2])); ro.append(_h.escape(r[j1:j2]))
        else:
            if i2 > i1: lo.append('<span class="hl">%s</span>' % _h.escape(l[i1:i2]))
            if j2 > j1: ro.append('<span class="hl">%s</span>' % _h.escape(r[j1:j2]))
    return "".join(lo), "".join(ro)

# ── 文件夹比较（webapp） ─────────────────────────────────
def _scan(d):
    out = {}
    for e in os.scandir(os.path.join(S, d)):
        st = e.stat()
        out[e.name] = dict(dir=e.is_dir(), size=st.st_size, mtime=st.st_mtime,
                           mode=stat.filemode(st.st_mode))
    return out

def folder_data(v1="folders/webapp-v1", v2="folders/webapp-v2"):
    L, R = _scan(v1), _scan(v2)
    names = sorted(set(L) | set(R))
    rows = []
    for n in names:
        l, r = L.get(n), R.get(n)
        for side in (l, r):
            if side: side["mtime"] = dt(side["mtime"])
        if l and r:
            if l["dir"] or r["dir"]: st = "same"
            elif l["size"] != r["size"]: st = "m"
            else: st = "same"
            if n.lower().endswith((".png", ".jpg", ".bin", ".zip")) and l["size"] != r["size"]: st = "b"
            rows.append((st, n, fmt(l["size"]), l["mtime"], l["mode"],
                         n, fmt(r["size"]), r["mtime"], r["mode"]))
        elif l:
            rows.append(("l", n, fmt(l["size"]), l["mtime"], l["mode"], None, None, None, None))
        else:
            rows.append(("r", None, None, None, None, n, fmt(r["size"]), r["mtime"], r["mode"]))
    cnt = dict(l=sum(1 for x in rows if x[0] == "l"), r=sum(1 for x in rows if x[0] == "r"),
               m=sum(1 for x in rows if x[0] == "m"), b=sum(1 for x in rows if x[0] == "b"),
               s=sum(1 for x in rows if x[0] == "same"))
    def tot(d):
        fs = [x["size"] for x in d.values() if not x["dir"]]
        return len(fs), sum(fs)
    return rows, cnt, tot(L), tot(R)

# ── 压缩包内容（zip 以文件夹会话打开） ────────────────────
def archive_data():
    import zipfile
    def scan(p):
        with zipfile.ZipFile(os.path.join(S, p)) as z:
            return {i.filename: dict(dir=i.filename.endswith("/"), size=i.file_size,
                                     mtime="%04d/%02d/%02d %02d:%02d" % i.date_time[:5], mode="-rw-r--r--")
                    for i in z.infolist()}
    L, R = scan("archive/pkg_v1.zip"), scan("archive/pkg_v2.zip")
    rows = []
    for n in sorted(set(L) | set(R)):
        l, r = L.get(n), R.get(n)
        if l and r:
            st = "b" if n.endswith(".png") and l["size"] != r["size"] else ("m" if l["size"] != r["size"] else "same")
            rows.append((st, n, fmt(l["size"]), l["mtime"], l["mode"], n, fmt(r["size"]), r["mtime"], r["mode"]))
        elif l:
            rows.append(("l", n, fmt(l["size"]), l["mtime"], l["mode"], None, None, None, None))
        else:
            rows.append(("r", None, None, None, None, n, fmt(r["size"]), r["mtime"], r["mode"]))
    cnt = dict(l=sum(1 for x in rows if x[0] == "l"), r=sum(1 for x in rows if x[0] == "r"),
               m=sum(1 for x in rows if x[0] == "m"), b=sum(1 for x in rows if x[0] == "b"),
               s=sum(1 for x in rows if x[0] == "same"))
    def tot(d):
        fs = [x["size"] for x in d.values() if not x["dir"]]
        return len(fs), sum(fs)
    return rows, cnt, tot(L), tot(R), os.path.getsize(os.path.join(S, "archive/pkg_v1.zip")), \
           os.path.getsize(os.path.join(S, "archive/pkg_v2.zip"))

# ── 十六进制 ─────────────────────────────────────────────
def hex_data(nrows=26):
    h1 = open(os.path.join(S, "hex/firmware_v1.bin"), "rb").read()
    h2 = open(os.path.join(S, "hex/firmware_v2.bin"), "rb").read()
    diff = [i for i in range(min(len(h1), len(h2))) if h1[i] != h2[i]]
    ranges, start = [], None
    for i, p in enumerate(diff):
        if start is None: start = p
        if i + 1 == len(diff) or diff[i + 1] != p + 1:
            ranges.append((start, p)); start = None
    rows = []
    for r in range(min(nrows, (len(h1) + 15) // 16)):
        off = r * 16
        b1, b2 = h1[off:off + 16], h2[off:off + 16]
        d = {i for i in range(len(b1)) if b1[i] != b2[i]}
        rows.append(("0x%08X" % off,
                     " ".join("%02X" % x for x in b1), " ".join("%02X" % x for x in b2), d))
    return rows, len(h1), len(diff), ranges

# ── 表格 ─────────────────────────────────────────────────
def csv_data():
    L = [l.split(",") for l in rd("csv/staff_v1.csv").strip().split("\n")]
    R = [l.split(",") for l in rd("csv/staff_v2.csv").strip().split("\n")]
    hdr, body_l, body_r = L[0], L[1:], R[1:]
    rows, mrows = [], 0
    for i in range(max(len(body_l), len(body_r))):
        l = body_l[i] if i < len(body_l) else None
        r = body_r[i] if i < len(body_r) else None
        if l and r:
            st = "s" if l == r else "m"
            if st == "m": mrows += 1
            rows.append((st, l, r))
        elif l: rows.append(("d", l, None))
        else: rows.append(("i", None, r))
    dcols = [c for c in range(len(hdr))
             if any(l and r and l[c] != r[c] for _, l, r in rows if l and r)]
    return hdr, rows, mrows, dcols

# ── 图片 / 媒体 ──────────────────────────────────────────
def image_data():
    from PIL import Image
    out = {}
    for k, f in (("l", "image/logo_v1.png"), ("r", "image/logo_v2.png")):
        p = os.path.join(S, f)
        out[k] = dict(size=fmt(os.path.getsize(p)), wh=Image.open(p).size)
    return out

def media_data():
    out = {}
    for k, f in (("l", "media/tone_v1.wav"), ("r", "media/tone_v2.wav")):
        p = os.path.join(S, f)
        with wave.open(p) as w:
            out[k] = dict(size=fmt(os.path.getsize(p)), rate=w.getframerate(),
                          ch=w.getnchannels(), bits=w.getsampwidth() * 8,
                          dur="00:00:%06.3f" % (w.getnframes() / w.getframerate()))
    return out

def wave_svg(path, samples=900, h=132):
    """用真实 WAV 采样画波形（v1 440Hz / v2 880Hz 一眼可辨）"""
    import array
    with wave.open(path) as w:
        ch, n = w.getnchannels(), min(w.getnframes(), samples)
        raw = w.readframes(n)
    a = array.array("h"); a.frombytes(raw)
    if ch > 1: a = a[0::ch]
    a = a[:samples]
    m = max(1, max(abs(v) for v in a))
    pts = " ".join("%.1f,%.1f" % (i * (1000 / max(1, len(a) - 1)), h / 2 - v / m * (h / 2 - 6))
                   for i, v in enumerate(a))
    return ('<div class="wave2"><svg viewBox="0 0 1000 %d" preserveAspectRatio="none">'
            '<line x1="0" y1="%d" x2="1000" y2="%d" stroke="#D4D7D8" stroke-width="1"/>'
            '<polyline points="%s" fill="none" stroke="#1F7A3D" stroke-width="1.4"/></svg></div>'
            % (h, h // 2, h // 2, pts))

def text_pair(rev=False, n=26):
    """rev=True 即 BC 的「交换两边」：v2 在左、v1 在右，新增就变成删除"""
    lf, rf = ("text/order_main_v2.c", "text/order_main_v1.c") if rev else \
             ("text/order_main_v1.c", "text/order_main_v2.c")
    a, b = rd(lf).split("\n"), rd(rf).split("\n")
    rows, blocks = _align(a, b)
    return dict(rows=rows[:n], blocks=blocks, lname=lf.split("/")[-1], rname=rf.split("/")[-1],
                lsz=os.path.getsize(os.path.join(S, lf)), rsz=os.path.getsize(os.path.join(S, rf)),
                nl=len(a), nr=len(b),
                ndel=sum(1 for r in rows if r[0] == "del"), nins=sum(1 for r in rows if r[0] == "ins"),
                nmod=sum(1 for r in rows if r[0] == "mod"))
