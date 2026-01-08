# 用户交互流程控制说明

## 问题分析

当前的 `english-learning.yml` 配置中存在一个潜在问题：AI可能在没有用户输入的情况下尝试生成内容。

## 关键控制点

### 1. AI Chat 节点输入配置

**当前配置：**
```yaml
- id: ai-chat
  inputs:
    text: 
      source: doubao-asr/text
      queue_size: 1000
    topic: topic-generator/topic  # 这个输入可能触发AI自动生成
    control: session-controller/chat_control
```

**问题：**
- `topic` 输入独立于用户输入，可能导致AI在收到topic后立即开始生成，而不等待用户说话
- 需要确保AI只在收到 `text` 输入（用户语音转文本）后才响应

### 2. 推荐的流程控制策略

#### 策略A：仅响应用户输入（推荐）
AI chat 节点应该**只**在收到用户文本输入时才触发响应：

```yaml
- id: ai-chat
  inputs:
    text: doubao-asr/text  # 主要触发源 - 必须有用户输入
    # topic 应该作为上下文，而不是触发器
```

#### 策略B：使用会话管理器控制
添加一个会话管理器节点来协调输入：

```yaml
- id: session-manager
  inputs:
    user_text: doubao-asr/text
    topic: topic-generator/topic
  outputs:
    - chat_input  # 组合后的输入，确保有用户文本时才输出

- id: ai-chat
  inputs:
    text: session-manager/chat_input  # 只从会话管理器接收
```

### 3. MaaS Client 节点的行为

查看 `dora-maas-client` 的代码，我们可以看到它：
- 通过 `Event::Input` 接收输入
- 每次收到 `text` 输入时触发一次响应
- 支持会话管理和历史记录

**关键点：** 该节点在收到任何输入端口的数据时都会被触发。如果 `topic` 独立到达，可能导致不期望的行为。

## 修正方案

### 方案1：移除独立的 topic 输入

将 topic 整合到系统提示中，而不是作为独立输入：

```yaml
# 在 word-selector 输出后，直接将 topic 注入到会话上下文
- id: ai-chat
  inputs:
    text: doubao-asr/text  # 唯一触发源
    control: session-controller/chat_control
  env:
    MAAS_CONFIG_PATH: english_teacher_config.toml
    # Topic 通过配置文件或初始化时设置
```

### 方案2：添加输入门控节点

创建一个门控节点，确保只有当用户输入存在时才允许处理：

```yaml
- id: input-gate
  custom: gate-controller
  inputs:
    user_text: doubao-asr/text
    context: topic-generator/topic
  outputs:
    - gated_input  # 只有当 user_text 存在时才输出
  env:
    GATE_MODE: wait_for_user  # 等待用户输入模式

- id: ai-chat
  inputs:
    text: input-gate/gated_input  # 通过门控的输入
```

### 方案3：使用 dora-maas-client 的控制信号

利用现有的 `control` 输入来管理何时允许AI响应：

```yaml
- id: session-controller
  outputs:
    - chat_control  # 控制AI何时可以响应
  logic: |
    # 只在收到用户输入后发送 'enable' 信号
    on user_speech_detected:
      send chat_control: {command: "enable"}
    on ai_response_complete:
      send chat_control: {command: "wait"}  # 等待下一次用户输入

- id: ai-chat
  inputs:
    text: doubao-asr/text
    control: session-controller/chat_control  # 控制何时处理输入
```

## 实现建议

### 最简单的修正（立即可用）

修改 `english-learning.yml`，将 topic 从 ai-chat 的输入中移除：

```yaml
# ============ AI Conversation ============

# Chat completion using Doubao (豆包) API
- id: ai-chat
  build: cargo build --manifest-path ../../../node-hub/dora-maas-client/Cargo.toml
  path: ../../../node-hub/dora-maas-client/target/release/dora-maas-client
  inputs:
    text: 
      source: doubao-asr/text  # 只有这个输入
      queue_size: 1000
    control: session-controller/chat_control
  outputs:
    - response
    - status
    - log
  env:
    MAAS_CONFIG_PATH: english_teacher_config.toml
    DOUBAO_API_KEY: ${DOUBAO_API_KEY:-}
    LOG_LEVEL: INFO
```

### Topic 整合方式

有两种方式将 topic 信息传递给AI：

#### 方式1：通过初始系统消息
在 session 开始时，将 topic 作为系统消息的一部分：

```toml
# english_teacher_config.toml
[system_prompt]
content = """You are a professional English teacher...

Today's conversation topic will be provided at the session start.
Use the topic to guide the conversation naturally."""
```

然后在代码中注入：
```rust
// 在 word-selector 完成后，发送一个特殊的初始化消息
session.add_system_context(&format!("Today's topic: {}", topic));
```

#### 方式2：通过环境变量或配置
将 topic 写入一个临时配置文件，AI节点启动时读取。

## 测试检查清单

修改后，验证以下行为：

- [ ] AI 不会在启动后立即说话
- [ ] AI 只在用户说话后才响应
- [ ] 用户沉默时，AI 保持等待状态
- [ ] Topic 信息被正确整合到对话中
- [ ] 每次用户输入都触发一次AI响应
- [ ] AI 响应完成后，等待下一次用户输入

## 调试技巧

### 1. 添加日志输出

在 `doubao-asr` 节点中添加：
```rust
log::info!("User speech detected: {}", text);
```

在 `ai-chat` 节点中添加：
```rust
log::info!("AI generating response to user input");
```

### 2. 监控事件流

使用 dora 的事件监控：
```bash
dora list
dora logs ai-chat
```

### 3. 检查队列状态

```bash
# 查看输入队列是否堆积
dora inspect english-learning.yml
```

## 总结

关键原则：
1. **AI 输入应该只有一个主触发源**：用户的语音输入（经过ASR转换）
2. **上下文信息（如topic）应该通过其他方式传递**：系统提示、配置文件、或初始化消息
3. **使用控制信号来管理对话流程**：enable/wait/reset
4. **保持数据流的单向性**：User → ASR → AI → TTS → User

遵循这些原则，可以确保AI始终等待用户输入，不会自动连续生成内容。
