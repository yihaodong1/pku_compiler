# Koopa IR: BasicBlock / BasicBlockData / BasicBlockNode 的关系

> 记录日期: 2026-06-04

---

## 1. 背景：数据与 Layout 分离

Koopa IR 的内存形式中，"IR 的数据" 和 "IR 的 layout" 是彼此分离表示的。指令数据统一存放在函数内的 `DataFlowGraph` 结构中，每条指令具有一个指令 ID（handle），通过 ID 在该结构中获取对应指令。指令列表中存放的实际上是 ID。

"指令 ID" 和 "指令数据" 的对应关系，类似于 C/C++ 中 "指针" 和 "指针所指向的内存" 的对应关系。这么做的原因：

- **所有权困境**：指令之间需要互相引用（如 br 指令指向目标基本块），在 Rust 的所有权模型下，自引用/循环引用结构会撞墙。标准做法需引入 `Rc<RefCell<T>>`，带来运行时开销和代码臃肿。
- **生命周期污染**：若用引用 `&T`，生命周期标注会传播到整个代码库，且存在不可变引用时无法修改指令列表。
- **ID 方案的优势**：ID 本质是整数索引，`Copy` 语义，无生命周期，可自由复制，修改列表不影响已有 ID 的有效性。

---

## 2. 三种类型的角色

| 类型 | 角色 | 所在文件 |
|---|---|---|
| `BasicBlock` | 轻量级句柄（`NonZeroU32` 的 newtype wrapper），`Copy` 语义 | `entities.rs` |
| `BasicBlockData` | 数据本体——基本块"是什么"：名字、参数、被谁使用 | `entities.rs` |
| `BasicBlockNode` | Layout 本体——基本块"在哪儿"：指令顺序、前后位置 | `layout.rs` |

### 2.1 BasicBlock（句柄）

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BasicBlock(pub(in crate::ir) BasicBlockId);
// BasicBlockId = NonZeroU32
```

只是一个整数 ID，可以任意拷贝、比较、用作 HashMap key。

### 2.2 BasicBlockData（数据）

```rust
pub struct BasicBlockData {
    name: Option<String>,        // 基本块名（%entry 等）
    params: Vec<Value>,          // SSA 块参数（phi 的替代方案）
    used_by: HashSet<Value>,     // 哪些跳转/分支指令指向此块
}
```

只管定义和使用关系，**不管指令顺序**。

### 2.3 BasicBlockNode（Layout）

```rust
pub struct BasicBlockNode {
    insts: InstList,                 // 指令 ID 列表（顺序核心）
    prev: Option<BasicBlock>,        // 前驱基本块 ID
    next: Option<BasicBlock>,        // 后继基本块 ID
}
```

只管指令的先后顺序和基本块之间的先后顺序。

---

## 3. 三者关系图

```
BasicBlock (只是一个 NonZeroU32)
    │
    ├──→ DataFlowGraph.bbs[bb] → BasicBlockData  ← "这个块是什么"
    │       ├── name: Option<String>
    │       ├── params: Vec<Value>
    │       └── used_by: HashSet<Value>
    │
    └──→ Layout.bbs[bb] → BasicBlockNode           ← "这个块怎么排"
            ├── insts: [Value, Value, Value, ...]   (也是 ID 列表)
            ├── prev: Option<BasicBlock>
            └── next: Option<BasicBlock>
```

同一个 `BasicBlock`（ID）同时作为 `DataFlowGraph` 和 `Layout` 的 key。增删指令只动 `Layout` 里的 `BasicBlockNode`，增删块参数只动 `DataFlowGraph` 里的 `BasicBlockData`，两边互不干扰。

完整的函数结构：

```
FunctionData {
    dfg: DataFlowGraph,    // owns values and basic block data
    layout: Layout,        // owns basic block nodes and instruction order
}
```

---

## 4. KeyNodeList：如何把 ID 和 Node 串起来

`KeyNodeList` 是 `BasicBlockNode` 所属的容器，定义来自独立的 `key-node-list` crate。

### 4.1 结构定义

```rust
pub struct KeyNodeList<K, N, M = HashMap<K, N>> {
    nodes: M,              // HashMap<K, N> — key 到 node 的映射
    head: Option<K>,       // 头结点的 key
    tail: Option<K>,       // 尾结点的 key
}
```

关键设计：**链表指针不在 `KeyNodeList` 结构体里，而在每个 Node 里；指针存的不是内存地址，而是 Key（ID）**。

类型别名：

```rust
type BasicBlockList = KeyNodeList<BasicBlock, BasicBlockNode, BasicBlockMap>;
type InstList       = KeyNodeList<Value, InstNode, InstMap>;
```

### 4.2 Node Trait

每个 Node 必须实现 `Node` trait，暴露 `prev`/`next` 指针：

```rust
pub trait Node {
    type Key;
    fn prev(&self) -> Option<&Self::Key>;
    fn next(&self) -> Option<&Self::Key>;
    fn prev_mut<T: NodeToken>(&mut self) -> &mut Option<Self::Key>;
    fn next_mut<T: NodeToken>(&mut self) -> &mut Option<Self::Key>;
}
```

`BasicBlockNode` 通过 `impl_node!` 宏自动实现：

```rust
impl_node!(BasicBlockNode { Key = BasicBlock, prev = prev, next = next });

// 展开等效：
// impl Node for BasicBlockNode {
//     type Key = BasicBlock;
//     fn prev(&self) -> Option<&BasicBlock> { self.prev.as_ref() }
//     fn next(&self) -> Option<&BasicBlock> { self.next.as_ref() }
//     fn prev_mut<__: NodeToken>(&mut self) -> &mut Option<BasicBlock> { &mut self.prev }
//     fn next_mut<__: NodeToken>(&mut self) -> &mut Option<BasicBlock> { &mut self.next }
// }
```

`NodeToken` 是 sealed trait——只有 `KeyNodeList` 自己的插入/删除方法可以修改 `prev`/`next`，外部代码只能读，保证链表完整性。

### 4.3 遍历过程

```rust
let mut cur = list.head;              // Option<BasicBlock> ← 只是一个 ID
while let Some(key) = cur {
    let node = list.nodes.get(&key);  // 用 ID 查 HashMap 拿到 BasicBlockNode
    // ... 处理 node ...
    cur = node.next();                // 从 node 里读出下一个 ID
}
```

示意图：

```
head → BasicBlock(id=3)
         │
         └──→ HashMap[3] → BasicBlockNode { prev: None, next: Some(7) }
                                                              │
                              BasicBlock(id=7) ←──────────────┘
                                │
                                └──→ HashMap[7] → BasicBlockNode { prev: Some(3), next: Some(12) }
                                                                                       │
                              BasicBlock(id=12) ←───────────────────────────────────────┘
                                ...
```

**指针即 ID，ID 查表得 Node，Node 里存着下一个 ID**。三步循环。

### 4.4 BasicBlockMap

`BasicBlockMap` 是 `KeyNodeList` 的第三个泛型参数，实现了 `Map<BasicBlock, BasicBlockNode>` trait。它在 `insert` 时会构造一个新的 `BasicBlockNode`（而非直接存入传入值），并维护 `inst_bb` 反向映射（`Weak<RefCell<HashMap<Value, BasicBlock>>>`），用于从指令 ID 反查所属基本块。

---

## 5. 总结

| 概念 | 一句话 |
|---|---|
| 数据与 Layout 分离 | `DataFlowGraph` 管 "是什么"，`Layout` 管 "怎么排" |
| ID 代替指针 | 所有引用用整数 ID，`Copy` 语义，无生命周期污染 |
| BasicBlockData | 基本块的数据面：名字、参数、use-def 链 |
| BasicBlockNode | 基本块的 layout 面：指令顺序、前后块顺序 |
| KeyNodeList | 用 HashMap + 节点内 prev/next ID 实现的无 unsafe 双向链表 |
| Node trait | 让 KeyNodeList 能通过统一接口操作任意节点的链表指针 |
