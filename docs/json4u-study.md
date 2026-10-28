# json4u 执行流程研究与 JSON Lens 改进

本轮以相邻的 `json4u` 源码为依据，优化独立的 Rust 项目 `json-lens`。
没有修改浏览器项目。研究重点是交互如何触发解析、定位、编辑与视口更新。

## 原版行为与迁移结果

| 原版入口 | 执行行为 | JSON Lens 本轮改进 |
| --- | --- | --- |
| `src/containers/editor/graph/Graph.tsx` → `useVirtualGraph.ts` → `src/lib/graph/virtual.ts` | 根据视口生成可渲染子图，裁剪属性行，并控制屏外连线数量 | 按布局列建立索引；缓存折叠状态、边界和可见节点；静止帧不重复扫描整个图；每个可见父节点最多处理 10 条屏外子节点连接 |
| `src/containers/editor/graph/KV.tsx` → `useClickNode.ts` → `Editor.tsx::useRevealNode` | 键和值点击生成 reveal position，再选中编辑器中的节点文本 | 卡片属性行有独立 JSON Pointer；点击定位到原始文本范围；大文件预览移动到对应位置 |
| `EditableText.tsx` → `statusStore.ts::editQueue` → `Editor.tsx::useEditTree` | 双击编辑键或值，通过版本化编辑队列写回源码 | 双击值打开节点编辑器，支持编辑 JSON 值与重命名属性；冲突和非法 JSON 返回错误；仅替换目标源码范围 |
| `src/lib/editor/editor.ts::delayParseAndSet` → worker `parseAndFormat` | 编辑变化先去抖，再异步解析并更新视图 | 停止输入 500 ms 后自动刷新；后台解析期间仍允许输入；响应带 revision，旧结果不能覆盖新输入 |
| `src/containers/editor/graph/Toolbar.tsx` | 父节点导航、分支折叠、兄弟分支折叠 | 增加 Parent 和 Node 菜单中的分支/兄弟分支操作 |
| `Graph.tsx::panOnScroll` 与 `MouseButton.tsx` | 支持鼠标、触控板操作差异 | 滚动平移、Shift + 滚动水平平移、Ctrl/Cmd + 滚动缩放；支持触控板捏合 |
| `ViewSearchInput.tsx` → worker `searchInView` | 搜索结果携带节点 ID、目标和路径，选中后统一定位 | 保留键和值的路径定位；重用路径缓冲，减少全量遍历时的临时分配 |

浏览器版明确设置 `nodesDraggable={false}`。JSON Lens 仍保留已有的卡片拖动，
这是原生版提供的额外操作，并不是对原版配置的逐项照搬。

## 性能变化

原生版旧实现的主要重复工作：

1. 即使只格式化已解析的值，也重新构建图并重置画布。
2. 每帧扫描全部节点，计算隐藏状态、边界和连线。
3. JSON Pointer 定位逐次线性遍历节点。
4. 结构搜索为每个待访问节点复制路径和属性名。

现在格式化/压缩共享 `Arc<Value>` 和 `Arc<Graph>`；数据变化时按 Pointer 迁移
卡片位置和折叠状态。图索引支持按列二分查询视口候选项；移动的卡片另行检测，
保证拖动后的图也能正确裁剪。隐藏状态只在布局或折叠变化时重新计算。

性能基准使用同一台 Apple M4 Pro、release 构建和相同生成数据：809,839 字节、
10,000 张图卡片。每项预热后取 7 次运行的中位数。

| 操作 | 本轮改动前 | 最终实现 |
| --- | ---: | ---: |
| 1,000 次 Pointer 定位 | 12.892 ms | 0.045 ms |
| 结构搜索 | 7.445 ms | 2.391 ms |
| 10 次静止画布更新 | 1.454 ms | 0.295 ms |
| 图构建 | 4.817 ms | 4.485 ms |

同一次最终运行中，格式化后重建图耗时 5.600 ms，复用图耗时 0.984 ms。
复用路径已经接入真实的 `Job::Transform`；测试验证共享图和已解析值的 Arc。
图构建的小幅变化受运行波动影响，不作为主要优化结论。

画布基准通过 egui 的真实更新路径运行，但不创建 GPU 窗口，因此这些数字
代表 CPU 侧的帧准备成本，不能解释为 GPU 帧率。图构建和搜索计时包含临时结果
的销毁；数据生成、磁盘读取及首次字体加载在计时前完成或未包含在该基准中。
这些是 JSON Lens 前后对比，不是与浏览器版 json4u 的直接速度比较。

复现命令：

```sh
cargo bench --bench workspace --locked
cargo bench --bench processing --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

源码鼠标点击已补充反向联动图视图：后台从 UTF-8 偏移解析 Pointer，展开祖先并居中卡片；图引起的源码选中不会再次触发反向定位。

## 验证与边界

25 项检查覆盖 Unicode/转义 Pointer、重复属性的源码定位、保留周围格式的值编辑、
重命名冲突、视口索引与全量扫描的一致性、真实 egui 卡片点击/拖动、格式化图复用、
视口状态迁移，以及旧解析结果被新编辑 revision 拒绝。
原生 Metal 窗口也验证了节点编辑器的渲染。

图卡片仍最多 10,000 张，属性预览最多 8 行。文本和结构搜索覆盖完整文档，
导航结果分别限制为 10,000 和 1,000 项。单个节点编辑限制为 1 MiB。

仍未完整迁移的原版能力包括 jq、JSONPath、模糊搜索、源码光标反向联动表格、
完整的日期/URL/图片等值预览，以及浏览器的分享和持久化流程。
