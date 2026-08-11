# bcr — Python 绑定

Beyond Compare 风格文件对比工具（Rust CLI）的官方 Python 薄封装。

- **零第三方依赖**：仅标准库（`subprocess` / `json` / `dataclasses`），Python ≥ 3.9
- **类型化返回**：dataclass 结果（`CompareResult` / `SyncResult` / `Compare3Result` / `MergeResult` / `Mp3Result` / `ImgResult` / `CsvResult`）
- **契约稳定**：基于 bcr CLI 的版本化 `--json` 输出（compare.v1 / sync.v1 / …）

## 安装

```bash
# 需要 bcr 二进制在 PATH（或设置 BCR_BIN 环境变量）
pip install .

# 或直接用仓库内单文件（无需安装）
export BCR_BIN=/path/to/bcr
```

## 快速开始

```python
import bcr

r = bcr.compare("/data/in", "/data/out", content=True)
print(r.stats)               # Stats(same=..., left_only=..., right_only=..., differ=..., moved=...)
if r.has_differences:
    for e in r.differences:
        print(f"[{e.status}] {e.rel}")
```

完整 API 参考与场景示例见仓库 `docs/automation.md`。
