# 用户输入等待 - 快速参考

## 🎯 核心修改

### 问题
❌ AI 在 topic 到达时就开始说话，不等用户

### 解决
✅ 添加 session-context 节点作为门控，只有用户说话才触发 AI

## 📊 数据流对比

### 之前（错误）
```
topic-generator ──→ ai-chat ❌ 自动触发
```

### 现在（正确）
```
topic-generator ──→ session-context ──┐
                         ↑            │
doubao-asr ─────────────┘             │
                                      ↓
                              (仅在用户输入时)
                                      ↓
                                  ai-chat ✓
```

## 🔧 快速验证

### 启动后检查
```bash
# 看到这个 = 正常 ✓
[session-context] Waiting for user input...

# 看到这个 = 有问题 ❌
[ai-chat] AI generating response
```

### 用户说话后
```bash
# 应该看到这个顺序 ✓
[doubao-asr] User speech detected: ...
[session-context] User input received: ...
[session-context] Forwarded user input to AI
[ai-chat] AI generating response
```

## 🚀 快速构建

```powershell
# 构建新节点
cargo build --release --manifest-path node-hub\dora-session-context\Cargo.toml

# 启动
cd apps\colang\dataflow
dora start english-learning.yml

# 监控
dora logs session-context --follow
```

## ✅ 检查清单

- [ ] session-context 节点已构建
- [ ] english-learning.yml 已更新
- [ ] 启动后 AI 不自动说话
- [ ] 用户说话后 AI 正常响应
- [ ] AI 响应后再次等待用户

## 📝 关键文件

- **新节点**: `node-hub/dora-session-context/`
- **配置**: `apps/colang/dataflow/english-learning.yml`
- **详细文档**: `apps/colang/WAIT_FOR_USER_UPDATE.md`
- **验证指南**: `apps/colang/USER_INPUT_WAIT_VERIFICATION.md`

## 🎓 工作原理

```
1. Topic arrives    → Stored in memory (不输出)
2. User speaks      → Combines (user + topic) → Outputs
3. AI receives      → Generates response
4. AI completes     → Back to step 2 (等待用户)
```

## 💡 记住

**Golden Rule**: 
> AI 的输入只来自 session-context/chat_input
> 
> session-context 只在收到用户输入时才输出

这确保了：**用户说 → AI 答**，永不反转！
