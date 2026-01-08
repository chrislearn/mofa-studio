# 用户输入等待机制 - 验证和测试

## 修改总结

已实施以下更改以确保 AI 只在用户说话后才响应：

### 1. 新增组件

#### Session Context Manager (会话上下文管理器)
- **位置**: `node-hub/dora-session-context/`
- **功能**: 
  - 接收 topic，但**不立即转发**
  - 只在收到用户输入时才组合上下文并转发给 AI
  - 确保 topic 不会触发 AI 自动响应

**工作流程**:
```
Topic arrives → Store in memory (不输出)
↓
等待...
↓
User speaks → Combine (user_text + topic) → Forward to AI
```

### 2. 修改的配置

#### english-learning.yml
```yaml
# 原来：
ai-chat:
  inputs:
    text: doubao-asr/text
    topic: topic-generator/topic  # ❌ 会触发AI

# 现在：
session-context:
  inputs:
    topic: topic-generator/topic      # 接收但不转发
    user_text: doubao-asr/text        # 主触发器
  outputs:
    - chat_input                       # 只在用户输入时输出

ai-chat:
  inputs:
    text: session-context/chat_input  # ✅ 只在用户说话后收到
```

## 数据流分析

### 正常流程（用户主导）

```
1. 系统启动
   └─> word-selector: 选择单词
       └─> topic-generator: 生成话题
           └─> session-context: 存储话题 (不转发)
               └─> [等待用户输入] ⏸️

2. 用户说话
   └─> mofa-audio-input: 捕获音频
       └─> doubao-asr: 语音转文字
           └─> session-context: 收到用户文本
               └─> 组合 (文本 + 话题) → chat_input
                   └─> ai-chat: 生成回复 ✓
                       └─> doubao-tts: 语音合成
                           └─> mofa-audio-player: 播放

3. AI 回复完成
   └─> [等待下一次用户输入] ⏸️

4. 用户再次说话
   └─> 重复步骤 2-3
```

### 错误流程（已修复）

```
❌ 之前的问题：

1. topic-generator: 生成话题
   └─> ai-chat: 直接接收 topic
       └─> AI 立即开始说话 (用户还没说话！)
           └─> 不合理的对话流程

✅ 现在已修复：

1. topic-generator: 生成话题
   └─> session-context: 存储话题
       └─> 不输出任何东西
           └─> AI 保持沉默 ✓
               └─> 等待用户输入
```

## 验证清单

### 基本功能测试

- [ ] **测试 1: 启动不自动说话**
  ```bash
  # 启动系统
  dora start english-learning.yml
  
  # 预期：系统启动完成后，AI 不说话，等待用户
  # 检查日志：
  # ✓ "Topic received for session..."
  # ✓ "Waiting for user input..."
  # ✗ 不应该看到 "AI generating response"
  ```

- [ ] **测试 2: 用户说话触发 AI**
  ```bash
  # 用户说话（通过麦克风或测试输入）
  
  # 预期：AI 收到输入并响应
  # 检查日志：
  # ✓ "User input received: ..."
  # ✓ "Forwarded user input to AI"
  # ✓ "AI generating response"
  ```

- [ ] **测试 3: AI 响应后等待**
  ```bash
  # AI 说完一段话后
  
  # 预期：AI 停止，不继续说话，等待用户
  # 检查日志：
  # ✓ "TTS generated ... bytes"
  # ✓ Audio 播放完成
  # ✗ 不应该立即看到新的 "AI generating"
  ```

- [ ] **测试 4: 多轮对话**
  ```bash
  # 用户 → AI → 用户 → AI → ...
  
  # 预期：每次都是用户先说，AI 再回应
  # 检查对话历史顺序：
  # User: "Hello"
  # AI: "Hi, how are you?"
  # User: "I'm good"
  # AI: "That's great!"
  ```

### 边界情况测试

- [ ] **测试 5: 空白输入**
  ```bash
  # 用户说话但识别为空
  
  # 预期：session-context 忽略空输入，AI 不响应
  # 检查日志：
  # ✓ "Ignoring empty user input"
  ```

- [ ] **测试 6: 快速连续输入**
  ```bash
  # 用户快速说多句话
  
  # 预期：每句话都触发 AI 响应
  # 检查：没有输入被丢失
  ```

- [ ] **测试 7: Topic 更新**
  ```bash
  # 新会话开始，新 topic 生成
  
  # 预期：新 topic 存储，但仍等待用户
  # 检查日志：
  # ✓ "Topic received for session {new_id}"
  # ✓ "Waiting for user input..."
  ```

- [ ] **测试 8: 会话重置**
  ```bash
  # 发送重置信号
  
  # 预期：清除 topic，等待新的用户输入
  # 检查日志：
  # ✓ "Resetting session context"
  ```

## 日志监控命令

### 实时监控关键节点

```bash
# 监控 session-context（最关键）
dora logs session-context --follow

# 监控 AI chat
dora logs ai-chat --follow

# 监控 ASR
dora logs doubao-asr --follow

# 查看所有节点状态
dora list
```

### 关键日志模式

**正常模式（用户主导）：**
```
[session-context] Topic received for session abc123: "Discussing career goals"
[session-context] Waiting for user input...
⏸️  (静默期 - AI 在等待)
[doubao-asr] User speech detected: I want to improve my English
[session-context] User input received: I want to improve my English
[session-context] Forwarded user input to AI (session: abc123, first: true)
[ai-chat] AI generating response to user input
```

**错误模式（需要修复）：**
```
[topic-generator] Generated topic: ...
[ai-chat] AI generating response    # ❌ 没有用户输入就开始！
```

## 性能监控

### 响应时间检查

```bash
# 测量从用户说话到 AI 开始响应的时间
# 应该在 1-3 秒之间

# 时间分解：
# 1. 用户说话 → ASR 转文字: ~500-1000ms
# 2. session-context 处理: <10ms
# 3. AI 开始生成: ~500-1500ms
# 总计: 1-2.5 秒
```

### 队列监控

```bash
# 检查是否有数据积压
dora inspect english-learning.yml

# 关注：
# - session-context/chat_input 队列大小
# - ai-chat/text 队列大小
# - 应该接近 0（处理快）
```

## 调试技巧

### 问题：AI 仍然自动说话

**检查项：**
1. 确认使用的是修改后的 `english-learning.yml`
2. 检查 session-context 节点是否正常运行
3. 查看 ai-chat 的输入源是否正确

```bash
# 检查节点配置
cat apps/colang/dataflow/english-learning.yml | grep -A 5 "id: ai-chat"

# 应该看到：
# text:
#   source: session-context/chat_input  # ✓ 正确
```

### 问题：用户说话后 AI 不响应

**检查项：**
1. session-context 是否收到用户输入
2. 是否正确转发到 ai-chat
3. ai-chat 是否正常工作

```bash
# 逐个检查
dora logs doubao-asr | grep "User speech"
dora logs session-context | grep "User input received"
dora logs session-context | grep "Forwarded"
dora logs ai-chat | grep "generating"
```

### 问题：Topic 没有被使用

**检查项：**
1. session-context 是否收到 topic
2. 第一次用户输入时是否包含 topic

```bash
dora logs session-context | grep "Topic received"
dora logs session-context | grep "is_first_in_session: true"
```

## 成功标志

系统正常工作时，你应该观察到：

✅ **启动阶段**
- Word selector 选择单词
- Topic generator 生成话题
- Session context 存储话题
- **系统静默，等待用户**

✅ **对话阶段**
- 用户说话 → ASR 转文字
- Session context 组合输入
- AI 生成响应
- TTS 播放音频
- **AI 停止，等待用户下一句**

✅ **持续交互**
- 始终是：User → AI → User → AI
- 从不是：AI → AI 或自动循环

## 构建和运行

### 构建新节点

```bash
# 构建 session-context
cargo build --release --manifest-path node-hub/dora-session-context/Cargo.toml

# 验证可执行文件
ls node-hub/dora-session-context/target/release/dora-session-context.exe
```

### 完整启动流程

```bash
# 1. 构建所有节点（包括新的 session-context）
.\setup-english-learning.ps1

# 或手动：
cargo build --release --manifest-path node-hub/dora-session-context/Cargo.toml

# 2. 启动 dataflow
cd apps/colang/dataflow
dora start english-learning.yml

# 3. 观察日志
# 在另一个终端：
dora logs session-context --follow
```

## 总结

通过添加 `session-context` 节点，我们实现了：

1. **Topic 不触发 AI** - Topic 被存储，不直接转发
2. **用户输入是唯一触发器** - 只有用户说话才触发 AI
3. **上下文保留** - Topic 在用户说话时被注入
4. **清晰的控制流** - 易于理解和调试

这确保了对话始终由用户主导，AI 永远不会在没有用户输入的情况下自动说话。
