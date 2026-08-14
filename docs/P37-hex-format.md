# P37-1d hex 字节地址格式切换（对标 BC Hex Compare 视图菜单）

> 背景：BC 帮助文档 `commandshex.html` 确认视图菜单差距：
> 字节地址格式（hex/dec）、小尾/大端值显示、显示/隐藏字节地址。

## BC 命令语义（帮助文档原文）

| BC 菜单项 | 语义 |
|---|---|
| Current Byte Address | 显示当前字节地址（偏移列显示） |
| Little Endian Values | 按小端序解释字节值（首字节为最低有效位） |
| Big Endian Values | 按大端序解释字节值（首字节为最高有效位） |
| Byte Addresses | 显示或隐藏字节地址列 |

## 实施内容

### hexview.rs
- 新增 `pub enum HexValueMode { Raw, LittleEndian, BigEndian }`
- 新增 `pub fn format_offset(offset: usize, hex: bool) -> String`（{:08x} / {:08}）
- 新增 `pub fn hex_values_text(bytes: &[u8], mode: HexValueMode) -> String`
  （Raw = 现有逐字节 `{:02X}`；LE/BE = 每 4 字节一组按 u32 解释显示，不足 4 字节按剩余字节）

### difftab.rs（HexTabData + 工具栏）
- `HexTabData` 加字段：`addr_hex: bool`（默认 true）、`value_mode: HexValueMode`（默认 Raw）、`show_addr: bool`（默认 true）
- 工具栏（hex 模式时）加：
  - checkbox「显示地址」（show_addr）
  - ComboBox 地址格式（Hex / Dec）
  - ComboBox 值格式（字节 / 小尾 / 大尾）
- `paint_hex_row` 按模式渲染：偏移列按 addr_hex 格式化、可隐藏；hex 字节按 value_mode 渲染

### i18n
- 新 key ×10 语言：HexAddrHex / HexAddrDec / HexValRaw / HexValLittle / HexValBig / HexShowAddr

### 测试
- hexview 单元测试：format_offset 两种格式；hex_values_text 的 LE/BE 与 Raw 差异
- uikit 测试：hex 模式渲染不 panic + 切换值格式下拉
