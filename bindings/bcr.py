#!/usr/bin/env python3
"""bcr — Beyond Compare 风格文件对比工具的 Python 绑定(薄封装)。

设计(见 docs/P27-python-binding-design.md):
- 零第三方依赖(仅标准库 subprocess/json/dataclasses)
- 通过 subprocess 调用 bcr CLI,解析 `--json` 契约输出
- 返回类型化 dataclass;字段缺失容错;时间自动转 datetime
- 用法:
    import bcr
    r = bcr.compare("/a", "/b", content=True)
    if r.has_differences:
        bcr.sync("/a", "/b", mode="mirror")
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Optional

# bcr 可执行文件:环境变量 BCR_BIN 覆盖,默认 "bcr"(要求已加入 PATH)
BCR_BIN = os.environ.get("BCR_BIN", "bcr")


class Error(Exception):
    """bcr 调用失败(退出码 2 / 无输出 / JSON 解析失败)。"""


def _run(args: list[str]) -> dict:
    """执行 bcr 子命令并解析 JSON 契约输出。"""
    cmd = [BCR_BIN, *args, "--json"]
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
    except FileNotFoundError:
        raise Error(f"找不到 bcr 可执行文件(设置 BCR_BIN 环境变量指向 bcr 二进制)") from None
    if not p.stdout.strip():
        raise Error(p.stderr.strip() or f"bcr 无输出,退出码 {p.returncode}")
    try:
        data = json.loads(p.stdout)
    except json.JSONDecodeError as e:
        raise Error(f"bcr JSON 解析失败: {e}\nstdout: {p.stdout[:500]}") from None
    if not data.get("ok", False):
        raise Error(data.get("error") or f"bcr 失败,退出码 {p.returncode}")
    data["_exit"] = p.returncode
    return data


def _parse_mtime(v: Any) -> Optional[datetime]:
    """ISO-8601 UTC → datetime;空值返回 None。"""
    if not v:
        return None
    try:
        return datetime.fromisoformat(v.replace("Z", "+00:00"))
    except ValueError:
        return None


@dataclass
class Meta:
    """单侧文件元数据。"""
    size: Optional[int] = None
    mtime: Optional[datetime] = None
    mode: Optional[int] = None
    symlink: Optional[str] = None

    @classmethod
    def from_json(cls, d: Any) -> "Meta | None":
        if not d:
            return None
        return cls(
            size=d.get("size"),
            mtime=_parse_mtime(d.get("mtime")),
            mode=d.get("mode"),
            symlink=d.get("symlink"),
        )


@dataclass
class Entry:
    """单条比较结果。status: same|left_only|right_only|differ|moved"""
    rel: str
    status: str
    left: Optional[Meta] = None
    right: Optional[Meta] = None
    moved_to: Optional[str] = None
    attrs_differ: bool = False


@dataclass
class Stats:
    same: int = 0
    left_only: int = 0
    right_only: int = 0
    differ: int = 0
    moved: int = 0


@dataclass
class CompareResult:
    stats: Stats
    has_differences: bool
    entries: list[Entry]
    warnings: list[str] = field(default_factory=list)
    raw: dict = field(default_factory=dict)

    @property
    def differences(self) -> list[Entry]:
        """便捷过滤:仅差异条目(排除 same)。"""
        return [e for e in self.entries if e.status != "same"]


@dataclass
class SyncPlanItem:
    op: str          # copy|delete|rename|rmdir|skip|conflict
    rel: str = ""
    to: Optional[str] = None
    from_: Optional[str] = None
    reason: Optional[str] = None
    size: Optional[int] = None


@dataclass
class SyncStats:
    copy: int = 0
    delete: int = 0
    rename: int = 0
    rmdir: int = 0
    skip: int = 0
    conflict: int = 0
    errors: int = 0


@dataclass
class SyncResult:
    dry_run: bool
    mode: str
    plan: list[SyncPlanItem]
    stats: SyncStats
    raw: dict = field(default_factory=dict)


@dataclass
class CsvStats:
    same: int = 0
    left_only: int = 0
    right_only: int = 0
    modified: int = 0


@dataclass
class CsvResult:
    stats: CsvStats
    has_differences: bool
    raw: dict = field(default_factory=dict)


@dataclass
class TriStats:
    same: int = 0
    base_only: int = 0
    left_only: int = 0
    right_only: int = 0
    left_deleted: int = 0
    right_deleted: int = 0
    left_modified: int = 0
    right_modified: int = 0
    both_modified: int = 0
    conflict: int = 0


@dataclass
class TriEntry:
    rel: str
    status: str


@dataclass
class Compare3Result:
    stats: TriStats
    has_differences: bool
    entries: list[TriEntry]
    raw: dict = field(default_factory=dict)


@dataclass
class MergeResult:
    conflicts: int
    has_conflicts: bool
    output: Optional[str]
    raw: dict = field(default_factory=dict)


@dataclass
class Mp3Field:
    name: str
    left: Optional[str]
    right: Optional[str]
    diff: bool


@dataclass
class Mp3Result:
    fields: list[Mp3Field]
    has_differences: bool
    raw: dict = field(default_factory=dict)


@dataclass
class ImgResult:
    left_size: tuple[int, int]
    right_size: tuple[int, int]
    size_differs: bool
    diff_pixels: int
    total_pixels: int
    diff_ratio: float
    bounds: Optional[tuple[int, int, int, int]]
    has_differences: bool
    raw: dict = field(default_factory=dict)


def compare(
    left: str,
    right: str,
    *,
    content: bool = False,
    includes: Optional[list[str]] = None,
    excludes: Optional[list[str]] = None,
    show_same: bool = False,
    detect_moves: bool = True,
    compare_attrs: bool = False,
    compare_version: bool = False,
    profile: Optional[str] = None,
) -> CompareResult:
    """目录/文件对比,返回结构化结果。"""
    args = ["compare", left, right]
    if content:
        args.append("--compare-content")
    for g in includes or []:
        args += ["--include", g]
    for g in excludes or []:
        args += ["--exclude", g]
    if show_same:
        args.append("--show-same")
    if not detect_moves:
        args.append("--detect-moves=false")
    if compare_attrs:
        args.append("--compare-attrs")
    if compare_version:
        args.append("--compare-version")
    if profile:
        args += ["--profile", profile]
    data = _run(args)
    r = data["result"]
    s = r.get("stats", {})
    return CompareResult(
        stats=Stats(**{k: s.get(k, 0) for k in ("same", "left_only", "right_only", "differ", "moved")}),
        has_differences=r.get("has_differences", False),
        entries=[
            Entry(
                rel=e["rel"],
                status=e["status"],
                left=Meta.from_json(e.get("left")),
                right=Meta.from_json(e.get("right")),
                moved_to=e.get("moved_to"),
                attrs_differ=e.get("attrs_differ", False),
            )
            for e in r.get("entries", [])
        ],
        warnings=data.get("warnings", []),
        raw=r,
    )


def sync(
    left: str,
    right: str,
    *,
    mode: str = "update",
    reverse: bool = False,
    dry_run: bool = False,
    content: bool = False,
    includes: Optional[list[str]] = None,
    excludes: Optional[list[str]] = None,
) -> SyncResult:
    """目录同步;dry_run=True 只返回计划不执行。"""
    args = ["sync", left, right, "--mode", mode]
    if reverse:
        args.append("--reverse")
    if dry_run:
        args.append("--dry-run")
    if content:
        args.append("--compare-content")
    for g in includes or []:
        args += ["--include", g]
    for g in excludes or []:
        args += ["--exclude", g]
    data = _run(args)
    r = data["result"]
    plan = [
        SyncPlanItem(
            op=p.get("op", ""),
            rel=p.get("rel", ""),
            to=p.get("to"),
            from_=p.get("from"),
            reason=p.get("reason"),
            size=p.get("size"),
        )
        for p in r.get("plan", [])
    ]
    st = r.get("stats", {})
    return SyncResult(
        dry_run=r.get("dry_run", False),
        mode=r.get("mode", mode),
        plan=plan,
        stats=SyncStats(**{k: st.get(k, 0) for k in ("copy", "delete", "rename", "rmdir", "skip", "conflict", "errors")}),
        raw=r,
    )


def run(args: list[str]) -> dict:
    """底层透传:执行任意 bcr 子命令并返回解析后的 JSON dict。"""
    return _run(args)


def compare3(
    base: str,
    left: str,
    right: str,
    *,
    content: bool = False,
    includes: Optional[list[str]] = None,
    excludes: Optional[list[str]] = None,
    show_same: bool = False,
) -> Compare3Result:
    """三路文件夹对比。"""
    args = ["compare3", base, left, right]
    if content:
        args.append("--compare-content")
    for g in includes or []:
        args += ["--include", g]
    for g in excludes or []:
        args += ["--exclude", g]
    if show_same:
        args.append("--show-same")
    data = _run(args)
    r = data["result"]
    s = r.get("stats", {})
    keys = ("same", "base_only", "left_only", "right_only",
            "left_deleted", "right_deleted", "left_modified",
            "right_modified", "both_modified", "conflict")
    return Compare3Result(
        stats=TriStats(**{k: s.get(k, 0) for k in keys}),
        has_differences=r.get("has_differences", False),
        entries=[TriEntry(rel=e["rel"], status=e["status"]) for e in r.get("entries", [])],
        raw=r,
    )


def csv(
    left: str,
    right: str,
    *,
    key: Optional[str] = None,
    delimiter: str = ",",
    no_header: bool = False,
) -> CsvResult:
    """CSV/表格对比。"""
    args = ["csv", left, right]
    if key:
        args += ["--key", key]
    if delimiter != ",":
        args += ["--delimiter", delimiter]
    if no_header:
        args.append("--no-header")
    data = _run(args)
    r = data["result"]
    s = r.get("stats", {})
    return CsvResult(
        stats=CsvStats(**{k: s.get(k, 0) for k in ("same", "left_only", "right_only", "modified")}),
        has_differences=r.get("has_differences", False),
        raw=r,
    )


def merge(
    base: str,
    left: str,
    right: str,
    *,
    output: Optional[str] = None,
    algo: str = "patience",
    labels: Optional[list[str]] = None,
) -> MergeResult:
    """三路合并;返回冲突统计(JSON 模式不输出合并内容)。"""
    args = ["merge", base, left, right, "--algo", algo]
    if output:
        args += ["-o", output]
    for lb in labels or []:
        args += ["-L", lb]
    data = _run(args)
    r = data["result"]
    return MergeResult(
        conflicts=r.get("conflicts", 0),
        has_conflicts=r.get("has_conflicts", False),
        output=r.get("output"),
        raw=r,
    )


def mp3tag(left: str, right: str) -> Mp3Result:
    """MP3 标签对比(ID3v1/v2 字段级)。"""
    data = _run(["mp3tag", left, right])
    r = data["result"]
    return Mp3Result(
        fields=[
            Mp3Field(
                name=f["name"],
                left=f.get("left"),
                right=f.get("right"),
                diff=f.get("diff", False),
            )
            for f in r.get("fields", [])
        ],
        has_differences=r.get("has_differences", False),
        raw=r,
    )


def imgcmp(left: str, right: str) -> ImgResult:
    """图片对比(逐像素差异统计)。"""
    data = _run(["imgcmp", left, right])
    r = data["result"]
    ls = r.get("left_size") or [0, 0]
    rs = r.get("right_size") or [0, 0]
    b = r.get("bounds")
    return ImgResult(
        left_size=(ls[0], ls[1]),
        right_size=(rs[0], rs[1]),
        size_differs=r.get("size_differs", False),
        diff_pixels=r.get("diff_pixels", 0),
        total_pixels=r.get("total_pixels", 0),
        diff_ratio=r.get("diff_ratio", 0.0),
        bounds=tuple(b) if b else None,
        has_differences=r.get("has_differences", False),
        raw=r,
    )


if __name__ == "__main__":
    # 简易自检:python3 bcr.py /a /b
    if len(sys.argv) == 3:
        r = compare(sys.argv[1], sys.argv[2])
        print(f"same={r.stats.same} left_only={r.stats.left_only} "
              f"right_only={r.stats.right_only} differ={r.stats.differ} moved={r.stats.moved}")
        for e in r.differences:
            print(f"  [{e.status}] {e.rel}")
        sys.exit(1 if r.has_differences else 0)
    print(__doc__)
