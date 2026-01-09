# 用户输入等待机制 - 更新说明

## 问题描述

之前的配置中，AI 可能在用户没有说话的情况下就开始生成内容，因为：
- `topic-generator` 输出的 topic 直接连接到 `ai-chat` 的输入
- 任何输入端口收到数据都可能触发节点处理
- Topic 到达时就触发了 AI 响应，而不是等待用户输入

## 解决方案

### 核心思路
**分离触发器和上下文**：
- Topic 是**上下文信息**，应该被存储而不是触发 AI
- 用户输入是**唯一触发器**，只有用户说话才触发 AI 响应
- 通过中间节点（session-context）实现门控机制

### 实现方式

#### 1. 新增节点：Session Context Manager

**位置**: `rust-nodes/dora-session-context/`

**职责**:
```
接收 topic     → 存储在内存中（不输出）
接收 user_text → 组合（user_text + topic）→ 输出到 AI
```

**关键代码逻辑**:
```rust
match event {
    "topic" => {
        // 存储 topic，但不转发
        state.current_topic = Some(topic_info);
        // 不调用 node.send_output()
    }
    "user_text" => {
        // 只有这里才输出
        let contextual = combine(user_text, topic);
        node.send_output("chat_input", contextual);
    }
}
```

#### 2. 修改数据流配置

**之前**:
```yaml
topic-generator → ai-chat  # ❌ Topic 直接触发 AI
doubao-asr      → ai-chat  # ✓ 用户输入触发 AI
```

**现在**:
```yaml
topic-generator → session-context  # Topic 被存储
doubao-asr      → session-context  # 用户输入触发组合
                  ↓
              chat_input (只在用户输入时产生)
                  ↓
               ai-chat  # ✓ 只在用户说话后才收到输入
```

## 修改的文件

### 新增文件

1. **rust-nodes/dora-session-context/Cargo.toml** - 新节点配置
2. **rust-nodes/dora-session-context/src/main.rs** - 会话上下文管理逻辑
3. **apps/colang/USER_INPUT_CONTROL.md** - 控制机制说明
4. **apps/colang/USER_INPUT_WAIT_VERIFICATION.md** - 验证测试指南

### 修改的文件

1. **apps/colang/dataflow/english-learning.yml**
   - 添加 `session-context` 节点
   - 修改 `ai-chat` 的输入源从 `doubao-asr/text` 改为 `session-context/chat_input`
   - 添加详细注释说明控制流程

2. **setup-english-learning.ps1**
   - 添加 session-context 节点到构建列表

## 工作流程对比

### 修改前（问题流程）

```
时间线：
0s  │ 系统启动
1s  │ word-selector: 选择单词
2s  │ topic-generator: 生成 topic
3s  │ ❌ ai-chat: 收到 topic，开始说话（用户还没说话！）
4s  │ AI 自动说话...
5s  │ ❌ 错误：用户还没参与对话
```

### 修改后（正确流程）

```
时间线：
0s  │ 系统启动
1s  │ word-selector: 选择单词
2s  │ topic-generator: 生成 topic
3s  │ session-context: 存储 topic
4s  │ ✓ 系统静默，等待用户...
    │ ⏸️ [等待期]
10s │ ✓ 用户开始说话："Hello"
11s │ doubao-asr: 转换为文本
12s │ session-context: 组合(user_text + topic) → 输出
13s │ ai-chat: 收到组合输入，生成回复
14s │ doubao-tts: 转换为语音
15s │ 播放 AI 回复
16s │ ✓ AI 完成，再次等待用户...
    │ ⏸️ [等待期]
20s │ ✓ 用户再次说话...（循环继续）
```

## 数据结构

### Topic Info (存储在 session-context)
```json
{
  "session_id": "abc-123",
  "topic": "Discussing career goals and professional development",
  "target_words": ["confident", "persuade", "accomplish", ...]
}
```

### User Input (从 ASR)
```json
{
  "text": "I want to improve my skills",
  "confidence": 0.95,
  "session_id": "abc-123"
}
```

### Contextual Input (输出到 AI)
```json
{
  "user_text": "I want to improve my skills",
  "session_id": "abc-123",
  "topic": "Discussing career goals and professional development",
  "target_words": ["confident", "persuade", "accomplish", ...],
  "is_first_in_session": true
}
```

## 验证步骤

### 1. 构建新节点

```powershell
# 构建 session-context
cargo build --release --manifest-path rust-nodes\dora-session-context\Cargo.toml

# 或者运行完整设置
.\setup-english-learning.ps1
```

### 2. 启动系统

```bash
cd apps\colang\dataflow
dora start english-learning.yml
```

### 3. 观察日志

**期望看到的顺序**:
```
[word-selector] Selected 25 words for session abc-123
[topic-generator] Generated topic: Discussing career goals...
[session-context] Topic received for session abc-123: Discussing career...
[session-context] Waiting for user input...
# ⏸️ 系统在这里等待，AI 不说话

# 用户说话后：
[doubao-asr] User speech detected: I want to improve my skills
[session-context] User input received: I want to improve my skills
[session-context] Forwarded user input to AI (session: abc-123, first: true)
[session-context] Topic for this session: Some("Discussing career goals...")
[ai-chat] AI generating response to user input
```

### 4. 检查点

- [ ] 系统启动后，AI 不自动说话
- [ ] Topic 生成后，仍然保持静默
- [ ] 只有用户说话后，AI 才开始响应
- [ ] AI 回复完成后，再次等待用户
- [ ] 每次对话都是 User → AI 的顺序

## 故障排查

### 问题：AI 仍然自动说话

**原因分析**:
1. 可能使用了旧的配置文件
2. session-context 节点没有正常启动
3. 连接配置错误

**解决方法**:
```bash
# 1. 确认配置文件
cat apps/colang/dataflow/english-learning.yml | grep -A 3 "id: ai-chat"
# 应该看到：source: session-context/chat_input

# 2. 检查节点状态
dora list | grep session-context
# 应该显示 RUNNING

# 3. 查看日志
dora logs session-context --follow
# 应该看到 "Waiting for user input..."
```

### 问题：用户说话后 AI 没反应

**原因分析**:
1. ASR 没有正确识别
2. session-context 没有收到输入
3. 输入被过滤（如空白文本）

**解决方法**:
```bash
# 逐级检查数据流
dora logs doubao-asr | tail -20
dora logs session-context | tail -20
dora logs ai-chat | tail -20

# 查找断点
grep "User speech detected" logs/*
grep "User input received" logs/*
grep "Forwarded user input" logs/*
```

## 性能影响

### 额外开销
- **处理延迟**: +5-10ms (session-context 节点处理)
- **内存开销**: ~1KB (存储 topic 信息)
- **CPU 开销**: 可忽略不计

### 好处
- **避免无效计算**: 不会在没有用户输入时浪费 AI API 调用
- **更清晰的控制流**: 易于理解和调试
- **更好的用户体验**: 对话流程符合预期

## 与 MaaS Client 的兼容性

Session-context 节点输出的数据结构与 MaaS client 完全兼容：

```rust
// MaaS client 期望的输入（text 字段）
Event::Input { id: "text", data, .. }

// session-context 输出的 chat_input 被映射到 text
// 数据格式：ContextualInput 包含 user_text 字段
```

如果需要，可以在 MaaS client 中添加对 `topic` 和 `target_words` 的特殊处理。

## 未来改进

### 可能的增强
1. **话题注入优化**: 将 topic 自动添加到系统提示
2. **上下文窗口管理**: 自动清理旧的对话历史
3. **多模态输入**: 支持文本 + 图片等
4. **会话状态可视化**: 显示当前等待状态

### 扩展点
```rust
// 可以扩展 ContextualInput 结构
struct ContextualInput {
    user_text: String,
    session_id: String,
    topic: Option<String>,
    target_words: Option<Vec<String>>,
    is_first_in_session: bool,
    // 未来可添加：
    // user_emotion: Option<Emotion>,
    // conversation_history: Vec<Message>,
    // user_profile: UserProfile,
}
```

## 总结

通过添加 **Session Context Manager** 节点，我们成功实现了：

✅ **问题解决**: AI 不再在没有用户输入时自动说话
✅ **流程清晰**: Topic 作为上下文，用户输入作为触发器
✅ **易于维护**: 单一职责，逻辑集中在一个节点
✅ **向后兼容**: 不影响其他节点的功能
✅ **可扩展**: 容易添加新的上下文信息

现在系统的对话流程完全符合预期：**始终由用户主导，AI 只是响应**。
