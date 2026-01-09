# Colang Learning 架构重构说明

## 重构目标

将数据库操作集中到专门的两个节点，实现关注点分离和职责单一原则。

## 架构变更

### 新增节点

#### 1. **learning-db-writer** (学习数据写入器)
- **路径**: `rust-nodes/dora-learning-db-writer/`
- **职责**: 专门负责将英语学习问题写入数据库
- **输入**: 
  - `analysis`: 来自 grammar-checker 的分析结果 (JSON格式)
- **输出**:
  - `result`: 存储结果 (success, issues_stored, pronunciation_issues_stored)
  - `status`, `log`: 状态和日志信息
- **数据库操作**:
  - 将文本问题写入 `conversation_annotations` 表
  - 将问题词汇写入 `issue_words` 表
  - 将发音问题写入 `issue_words` 表

#### 2. **learning-db-reader** (学习数据读取器)
- **路径**: `rust-nodes/dora-learning-db-reader/`
- **职责**: 专门负责从数据库随机读取需要复习的词汇
- **输入**:
  - `trigger`: 来自 session-controller 的触发信号
- **输出**:
  - `selected_words`: 选择的词汇列表 (JSON: words, word_details, session_id)
  - `status`, `log`: 状态和日志信息
- **数据库操作**:
  - 基于间隔重复算法从 `issue_words` 表读取词汇
  - 创建学习会话记录到 `learning_sessions` 表
- **配置**:
  - `MIN_WORDS`: 最少选择词汇数 (默认 20)
  - `MAX_WORDS`: 最多选择词汇数 (默认 30)

### 重构的节点

#### 3. **conversation-analyzer** (对话分析器)
- **变更**: 移除所有数据库操作代码
- **新职责**: 只负责调用 AI API 分析文本
- **输入**: 
  - `user_text`: 用户输入的文本
- **输出**:
  - `analysis`: 完整的分析结果 JSON，包含:
    - `session_id`: 会话ID
    - `user_text`: 用户原文
    - `issues`: 文本问题列表 (语法、用词等)
    - `pronunciation_issues`: 发音问题列表
- **依赖移除**: 移除了 `sqlx` 依赖

### 移除的节点

- **report-storage**: 由新的 `learning-db-writer` 替代

## 数据流

```
┌─────────────────────┐
│   用户输入           │
│ (文字/语音)         │
└──────┬──────────────┘
       │
       v
┌─────────────────────┐
│ grammar-checker     │
│ (只做AI分析)        │
└──────┬──────────────┘
       │
       │ analysis (JSON)
       ├──────────────────────────┐
       │                          │
       v                          v
┌─────────────────────┐  ┌──────────────────┐
│ learning-db-writer  │  │ english-teacher  │
│ (写数据库)          │  │ (生成回复)       │
└─────────────────────┘  └──────────────────┘
       │
       │ result
       v
┌─────────────────────┐
│ mofa-chat-display   │
│ (显示界面)          │
└─────────────────────┘

┌─────────────────────┐
│ session-controller  │
│ (会话控制)          │
└──────┬──────────────┘
       │ trigger
       v
┌─────────────────────┐
│ learning-db-reader  │
│ (读数据库)          │
└──────┬──────────────┘
       │ selected_words
       v
┌─────────────────────┐
│ mofa-chat-display   │
└─────────────────────┘
```

## 优势

1. **职责分离**: 每个节点职责单一，易于维护
2. **数据库隔离**: 只有两个节点操作数据库，便于管理和调试
3. **易于测试**: 各节点可独立测试
4. **易于扩展**: 可以轻松替换或增强数据库操作逻辑
5. **性能优化**: DB节点可以专门优化数据库连接和查询

## 迁移说明

### 编译新节点

```powershell
# 编译 DB Writer
cargo build --release --manifest-path rust-nodes/dora-learning-db-writer/Cargo.toml

# 编译 DB Reader
cargo build --release --manifest-path rust-nodes/dora-learning-db-reader/Cargo.toml

# 重新编译 Analyzer (移除了 sqlx)
cargo build --release --manifest-path rust-nodes/dora-conversation-analyzer/Cargo.toml
```

### 配置文件变更

- 更新 `apps/colang/dataflow/learning.yml`
- 移除了 `report-storage` 节点
- 添加了 `learning-db-writer` 和 `learning-db-reader` 节点
- 更新了日志聚合配置

## 数据格式

### AnalysisOutput (从 grammar-checker 输出)

```json
{
  "session_id": "abc-123",
  "user_text": "I goed to store yesterday",
  "issues": [
    {
      "type": "grammar",
      "original": "goed",
      "suggested": "went",
      "description": "'Go' 的过去式是 'went'，不规则动词",
      "severity": "high"
    }
  ],
  "pronunciation_issues": [
    {
      "word": "yesterday",
      "confidence": 0.65
    }
  ]
}
```

### WordSelectionOutput (从 learning-db-reader 输出)

```json
{
  "words": ["went", "yesterday", "store"],
  "word_details": [
    {
      "id": 1,
      "word": "went",
      "issue_type": "grammar",
      "issue_description": "不规则动词过去式",
      "difficulty_level": 3,
      "context": "I goed to store yesterday"
    }
  ],
  "session_id": "def-456",
  "total_selected": 3
}
```

### StorageResult (从 learning-db-writer 输出)

```json
{
  "success": true,
  "issues_stored": 5,
  "pronunciation_issues_stored": 2,
  "error": null
}
```

## 注意事项

1. 数据库连接由两个 DB 节点独立管理
2. 数据库迁移在两个 DB 节点启动时自动运行
3. 其他节点不再需要 `DATABASE_URL` 环境变量
4. `conversation-analyzer` 现在是纯计算节点，不依赖数据库

## 测试建议

1. 测试 grammar-checker 只返回分析结果，不写数据库
2. 测试 learning-db-writer 正确存储所有类型的问题
3. 测试 learning-db-reader 正确读取并应用间隔重复算法
4. 集成测试：确认数据流正确传递

## 未来优化方向

1. 可考虑添加数据库连接池配置
2. 可添加数据库写入批处理优化
3. 可添加词汇选择算法的可配置策略
4. 可添加数据库备份和恢复功能
