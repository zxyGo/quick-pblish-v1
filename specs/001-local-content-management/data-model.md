# Data Model: 本地内容基座

**Feature**: 001-local-content-management | **Date**: 2026-06-21

真相来源是磁盘上的 Markdown 文件（含 YAML front matter）与 `assets/` 素材；SQLite 是可重建的派生缓存。
本文件定义领域实体、字段、关系、校验规则与状态。

---

## 实体

### Workspace（工作目录）

应用级配置，非内容的一部分。存于 OS 应用配置目录。

| 字段 | 类型 | 说明 | 校验 |
|------|------|------|------|
| `path` | string（绝对路径）| 工作目录根路径 | 必须存在且可读写；否则进入"需重新选择"状态 |
| `name` | string | 显示名（默认取目录名）| 非空 |
| `last_opened` | datetime | 最近打开时间 | — |

- **关系**：一个 Workspace 包含 0..N 个 Article 与 0..N 个 Asset（以文件系统层级组织）。
- **持久化**：`current` + `recent[]` 存于 Tauri 应用配置文件。
- **状态**：`Active`（可访问）/ `Unavailable`（被移动/删除/无权限 → 引导重选，FR-003）。

### Article（文章）

一篇 Markdown 文档，**内容为真相来源**。物理表现为工作目录内的一个 `.md` 文件。

| 字段 | 类型 | 来源 | 校验/说明 |
|------|------|------|-----------|
| `relative_path` | string | 文件在工作目录内的相对路径 | 唯一标识；跨平台以 `/` 归一化 |
| `title` | string | front matter `title` | 缺失时回退文件名（去扩展名）（FR-016/Q2）|
| `tags` | string[] | front matter `tags` | 默认空数组；去重 |
| `created` | datetime | front matter `created` | 缺失时回退文件创建时间或首次入库时间 |
| `updated` | datetime | front matter `updated` | 保存时更新；缺失回退文件 mtime |
| `excerpt` | string | 正文派生（前 N 字摘要）| 仅用于列表展示，不落 front matter |
| `body` | string | front matter 之后的正文 | Markdown 文本 |
| `content_hash` | string | 正文内容哈希 | 用于冲突检测（FR-019），不落盘到 front matter |

**front matter schema（落盘形态）**：

```yaml
---
title: 文章标题
tags: [标签1, 标签2]
created: 2026-06-21T10:00:00
updated: 2026-06-21T12:30:00
---
```

- **校验规则**：
  - front matter 缺失或损坏时，以默认值降级构造 Article，**不得**中断列表加载（FR-018）。
  - 新建/重命名导致文件名冲突时拒绝并提示，不静默覆盖（FR-020）。
- **关系**：Article 可引用 0..N 个 Asset（正文中的相对路径图片链接）。

### Asset（素材）

文章引用的非 Markdown 文件（主要是图片），物理存于工作目录的 `assets/`。

| 字段 | 类型 | 说明 |
|------|------|------|
| `relative_path` | string | 在工作目录内的相对路径（通常 `assets/...`）|
| `file_name` | string | 文件名 |
| `kind` | enum | `image` / `other` |

- **导入规则**：插入图片时复制进 `assets/`，正文写相对路径（FR-014a/Q4）。

### FileNode（文件树节点）

文件树的展示模型，直接映射磁盘结构（FR-012）。

| 字段 | 类型 | 说明 |
|------|------|------|
| `relative_path` | string | 相对工作目录路径 |
| `name` | string | 显示名 |
| `kind` | enum | `file` / `directory` |
| `is_article` | bool | 是否为 `.md` 文章 |
| `children` | FileNode[] | 子节点（目录时）|

---

## 派生缓存（SQLite）

**非真相来源，可随时从文件重建（FR-008a）。** 存于 OS 应用数据目录，按工作目录隔离。

### 表 `articles`

| 列 | 类型 | 说明 |
|----|------|------|
| `relative_path` | TEXT PRIMARY KEY | 文章相对路径 |
| `title` | TEXT | 标题（已应用回退）|
| `tags` | TEXT | 标签（JSON 数组序列化）|
| `created` | TEXT | ISO8601 |
| `updated` | TEXT | ISO8601 |
| `excerpt` | TEXT | 摘要 |
| `size` | INTEGER | 文件字节数（增量同步用）|
| `mtime` | INTEGER | 文件修改时间（增量同步用）|
| `content_hash` | TEXT | 正文哈希 |

### 表 `articles_fts`（FTS5 虚拟表）

- 索引 `title`、`tags`、`body` 用于检索（FR-017、SC-004）。

### 同步策略

- 启动/切换工作目录：扫描目录，按 `(size, mtime)` 与缓存比对，对新增/变更/删除做增量更新。
- 提供 `rebuild_index` command：清空并全量重建（缓存损坏时的恢复路径）。

---

## 关键状态流转

### 文章编辑会话

```text
Closed ──open──▶ Clean ──edit──▶ Dirty ──save──▶ Clean
                   ▲                 │
                   │                 ├─ save 检测到外部修改 ─▶ Conflict
                   │                 │                          ├─ overwrite ─▶ Clean
                   │                 │                          ├─ discard&reload ─▶ Clean
                   └──────── close（Dirty 时提示保存/放弃，FR-007）        └─ save-as ─▶ Clean(新文件)
```

### 工作目录

```text
None ──select──▶ Active ──switch──▶ Active(新目录)
                   │
                   └─ 目录不可访问 ─▶ Unavailable ──reselect──▶ Active
```
