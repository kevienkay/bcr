"""bcr.py 的 pytest 测试套件。

运行:
    BCR_BIN=target/debug/bcr pytest tests/py/ -v

依赖:仅 pytest(标准库 + 已构建的 bcr 二进制)。
测试用 tempfile 构造临时目录,覆盖 compare/sync/csv/compare3/merge/mp3tag/imgcmp
的 JSON 契约与 bcr.py 封装行为。
"""

import os
import subprocess
import sys
from contextlib import contextmanager
from pathlib import Path

import pytest

# 把 bindings/ 加入导入路径
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "bindings"))

import bcr  # noqa: E402

# 确保 bcr 二进制可用(默认 debug 构建;可用 BCR_BIN 覆盖)
BCR_BIN = os.environ.get("BCR_BIN", str(ROOT / "target/debug/bcr"))


def _run_cli(args):
    p = subprocess.run([BCR_BIN, *args], capture_output=True, text=True, encoding="utf-8")
    # compare/sync 有差异时退出码为 1(契约预期);仅 2 表示错误
    assert p.returncode in (0, 1), f"bcr {' '.join(args)} 失败: {p.stderr}"
    return p.stdout


# ── compare ──────────────────────────────────────────────

@pytest.fixture
@contextmanager
def _tmpdirs():
    import tempfile
    import shutil
    d1 = tempfile.mkdtemp()
    d2 = tempfile.mkdtemp()
    try:
        yield Path(d1), Path(d2)
    finally:
        shutil.rmtree(d1, ignore_errors=True)
        shutil.rmtree(d2, ignore_errors=True)


def test_compare_contract_shape():
    from contextlib import contextmanager
    with tempfile_dir() as (d1, d2):
        (d1 / "a.txt").write_text("aaa")
        (d1 / "same.txt").write_text("same")
        (d2 / "a.txt").write_text("bbb")
        (d2 / "same.txt").write_text("same")
        out = _run_cli(["compare", str(d1), str(d2), "--json"])
        import json
        data = json.loads(out)
        assert data["schema"] == "compare.v1"
        assert data["ok"] is True
        r = data["result"]
        assert r["has_differences"] is True
        rels = {e["rel"]: e["status"] for e in r["entries"]}
        assert rels["a.txt"] == "differ"


def tempfile_dir():
    import tempfile
    import shutil
    from contextlib import contextmanager

    @contextmanager
    def ctx():
        d1 = tempfile.mkdtemp()
        d2 = tempfile.mkdtemp()
        try:
            yield Path(d1), Path(d2)
        finally:
            shutil.rmtree(d1, ignore_errors=True)
            shutil.rmtree(d2, ignore_errors=True)

    return ctx()


def test_compare_python_api():
    d1 = Path(tempfile_dir := os.environ.get("TMPDIR", "/tmp"))
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        a = Path(t) / "a"; b = Path(t) / "b"
        a.mkdir(); b.mkdir()
        (a / "x.txt").write_text("1")
        (b / "x.txt").write_text("2")
        r = bcr.compare(str(a), str(b), content=True)
        assert r.has_differences
        assert r.stats.differ == 1
        assert r.differences[0].rel == "x.txt"
        assert r.differences[0].left.size == 1
        assert r.differences[0].left.mtime is not None


# ── sync ─────────────────────────────────────────────────

def test_sync_dry_run_then_execute():
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        src = Path(t) / "src"; dst = Path(t) / "dst"
        src.mkdir(); dst.mkdir()
        (src / "new.txt").write_text("hello")
        plan = bcr.sync(str(src), str(dst), mode="mirror", dry_run=True)
        assert plan.dry_run
        assert any(p.op == "copy" for p in plan.plan)
        res = bcr.sync(str(src), str(dst), mode="mirror")
        assert not res.dry_run
        assert res.stats.copy == 1
        assert (dst / "new.txt").read_text() == "hello"


# ── csv ──────────────────────────────────────────────────

def test_csv_contract():
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        c1 = Path(t) / "a.csv"; c2 = Path(t) / "b.csv"
        c1.write_text("id,name\n1,apple\n")
        c2.write_text("id,name\n1,pear\n")
        r = bcr.csv(str(c1), str(c2), key="id")
        assert r.has_differences
        assert r.stats.modified == 1


# ── compare3 / merge ─────────────────────────────────────

def test_compare3_and_merge_conflict():
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        bdir = Path(t) / "b"; ldir = Path(t) / "l"; rdir = Path(t) / "r"
        for d in (bdir, ldir, rdir):
            d.mkdir()
        (bdir / "f.txt").write_text("base")
        (ldir / "f.txt").write_text("left")
        (rdir / "f.txt").write_text("right")
        t3 = bcr.compare3(str(bdir), str(ldir), str(rdir))
        assert t3.stats.conflict == 1
        m = bcr.merge(str(bdir / "f.txt"), str(ldir / "f.txt"), str(rdir / "f.txt"))
        assert m.has_conflicts
        assert m.conflicts == 1


# ── mp3tag ───────────────────────────────────────────────

def test_mp3tag_same_file_no_diff():
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        p = Path(t) / "x.mp3"
        # 构造带 ID3v2 标签的最小 mp3
        frames = b""
        for fid, val in (("TIT2", "Song"), ("TPE1", "Artist")):
            data = b"\x03" + val.encode()
            frames += fid.encode() + len(data).to_bytes(4, "big") + b"\x00\x00" + data
        size = len(frames)
        p.write_bytes(b"ID3\x03\x00\x00" + bytes([
            (size >> 21) & 0x7F, (size >> 14) & 0x7F,
            (size >> 7) & 0x7F, size & 0x7F,
        ]) + frames)
        r = bcr.mp3tag(str(p), str(p))
        assert not r.has_differences
        assert r.fields == []


# ── imgcmp ───────────────────────────────────────────────

def test_imgcmp_identical():
    try:
        from PIL import Image
    except ImportError:
        pytest.skip("PIL 不可用,跳过图片测试")
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        p1 = Path(t) / "a.png"; p2 = Path(t) / "b.png"
        Image.new("RGB", (4, 4), (255, 0, 0)).save(p1)
        Image.new("RGB", (4, 4), (255, 0, 0)).save(p2)
        r = bcr.imgcmp(str(p1), str(p2))
        assert not r.has_differences
        assert r.diff_pixels == 0
