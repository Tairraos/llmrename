# 视觉模型契约（Vision Model Contract）

本文件定义 llmrename 与外部视觉模型的接口契约，是 `service` 层实现的唯一依据。

## 1. 支持的模型接入

- **默认**：OpenAI Chat Completions HTTP API（`https://api.openai.com/v1/chat/completions`）。
- **兼容**：任何实现了 OpenAI Chat Completions JSON 协议的端点（vLLM / OneAPI / 代理），通过 `base_url`（可含 `/v1`）配置。
- **模型发现**：`GET {base_url}/models`（OpenAI List Models 协议），供模型设置 dialog 的「⟳ 加载」按钮拉取可选模型；实现见 `service/models.rs`。
- **当前不做**：多模态流式、tools/function-calling、本地模型进程调用。

### 1.1 模型列表契约（GET /models）

```
GET {base_url}/models
Authorization: Bearer {api_key}   # api_key 为空时不带该头（本地服务常免鉴权）

200 OK
{ "object": "list", "data": [ { "id": "gpt-4o", ... }, ... ] }
```

- 只取 `data[].id`，排序去重后返回；其余字段忽略。
- 非 2xx 时透传服务端 `error.message`；响应不是合法 JSON 时报错并附 120 字符片段。
- 超时独立于 chat 调用，前端未传时默认 30s（clamp 1–3600）。

## 2. 请求构造

对每个资产发送一个请求：

```
POST {base_url}/chat/completions
Authorization: Bearer {api_key}
{
  "model": "{model}",
  "messages": [
    { "role": "system", "content": SYSTEM_PROMPT },
    { "role": "user", "content": [
        { "type": "text", "text": USER_PROMPT(asset.filename, template) },
        { "type": "image_url", "image_url": { "url": "data:image/{mime};base64,{b64}" } }
      ] }
  ],
  "temperature": 0.2,
  "max_tokens": 1024
}
```

- 图片以 data URL 内联（本地文件，无公网 URL 可用）。
- 请求体使用 `serde_json::json!` 构造，非逐字段手拼。

## 3. 响应解析（在边界 parse）

期望响应：

```json
{
  "choices": [
    { "message": { "content": "<JSON 字符串>" } }
  ]
}
```

`content` 为一段 JSON 文本，解析流程（逐级容错，命中即停）：

1. 取 `choices[0].message.content`，失败 → `JsonParse` 错误。
2. 剥离代码围栏（```json ... ``` 或 ``` ... ```），失败按原文本继续。
3. 依次尝试解析候选片段：整段原文，以及文本中所有「花括号配平」的片段
   （字节级扫描、尊重字符串内的花括号与转义）——模型常在 JSON 前后附加
   寒暄/解释文字（如 "Sure, here is the JSON object..."），剥掉后取对象本体。
4. 每个候选先按标准 JSON 解析；失败再把字符串外的单引号换成双引号重试
   （模型常输出 `{'人物': '一男一女'}`）。
5. 全部失败 → `JsonParse` 错误，附 serde 错误与响应前 120 字符片段。

`ExtractedFieldsJson`：模板字段名 → 字符串值 的 map，外加可选 `description` 残留处理：
- 若模板字段未覆盖，但模型返回额外字段（如 `description`、`category`），忽略额外字段，不报错。

## 4. 错误分类

| 状态 | 含义 | 处理 |
|---|---|---|
| HTTP 4xx | 鉴权/配额/参数错 | 该文件 `failed`，错误含状态码与响应体截断 |
| HTTP 5xx / 超时 | 服务端/网络 | 该文件 `failed`，错误含状态码或超时秒数 |
| 非 2xx 但解析出 JSON 错误体 | 携带 `error.message` | 尽量把 `error.message` 透传给用户 |

## 4.5 流式协议（当前默认启用）

请求体带 `"stream": true`，响应为标准 OpenAI SSE：

```
data: {"choices":[{"delta":{"content":"增量"}}]}
data: {"choices":[],"usage":{...}}   // 收尾块可能为空 choices，需容忍
data: [DONE]
```

- Service 层逐行解析，每段增量经 `on_delta` 回调上抛（Service 不感知 Tauri）
- Runtime 层把增量转成 `vision-stream` 事件：`{ path, filename, delta, done, error }`
- 前端在「待重命名」卡片的流式面板实时回显（文件名切换时重置面板）
- **回退**：服务端不支持流式（响应体不是 SSE）时，整体按非流式 JSON 解析
- 非 2xx 仍在发送阶段直接报错（透传 error.message）

## 5. 提示词

- `SYSTEM_PROMPT`（中文，稳定）：声明你是照片资产命名助手；只输出一个 JSON 对象，键为给定字段名；**值必须是简短中文短语**（适合文件名，无路径分隔符/斜杠/引号/冒号）；无法判断的字段值用「未知」；不得输出解释或其它文本。
- 产出文件名为中文（如 `{人物}-{场景}-{动作}-{日夜}` → `女人-街头-跳舞-夜晚.jpg`）。
- `USER_PROMPT`：**只列出需要提取的字段名及各自的提取指南**（`FIELD_GUIDES`，见 `service/prompts.rs`），不再发送文件名与模板——文件名/模板会干扰模型对图片内容的判断。
- 已知字段的提取指南（收录在 `service/prompts.rs::FIELD_GUIDES`）：
  - `{人物}`：按性别/年龄段，如 男孩子/女孩子/男生/女生/男人/女人/一男一女
  - `{人数}`：无人/一人/双人/多人
  - `{场景}`：先判断室内外；室外如 草地/森林/广场/公园/湖边/街道；室内如 健身房/卧室/厨房/客厅
  - `{动作}`：多人看互动；单人看正在做的事；判断不出做事则看姿势
  - `{季节}`：室外必须 春/夏/秋/冬；室内 未知
  - `{造型}`：上身 毛衣/春装/秋装/牛仔衣/皮衣/西装/休闲装；下身 裙子/牛仔裤/休闲裤/皮裤/牛仔短裤/短裤/短裙
  - `{天气}`：雨天/晴天/多云；室内 未知
  - `{日夜}`：晚上 夜晚；白天 空字符串
  - `{色调}`：红色调/黄色调/蓝色调/绿色调/灰色调/紫色调（取图内最多颜色）
  - `{智能}`：不提取属性，由模型综合整图内容直接起一个完整文件名——中文、不超过 12 个汉字、不要扩展名（字段级指南覆盖「无法判断用未知」的默认约定，起名即任务）
  - 未收录字段走通用指南：简短中文描述
- 提示词在本仓库 `service/prompts.rs` 内维护，变更即提交（让模型行为可审计）。

## 6. 保密与安全

- API Key 只进 `config.json`（应用数据目录），绝不入库、不进日志。
- 发送给模型的内容：图片像素 + 字段提取指南与字段名；**不含文件名、模板、目录路径**（避免干扰模型判断）。

## 7. 行为假设（当前版本接受的风险）

- 模型对视觉内容的提取结果**不保证精确**；预览阶段不做真实调用（成本与延迟），用户所见预览为字段名猜测，执行后以日志为准。
- 不重试（首版），失败即记 `failed`。
- 默认 `timeout_secs = 60`，可配置。

## 8. 实测记录（2026-09-19，本地 OpenAI 兼容服务 127.0.0.1:8317）

| 模型 | 文本 | 视觉 | 结论 |
|---|---|---|---|
| n/llama-3.2-11b-vision | ✅ | ✅（1x1 红图答 "Red."） | **选定**：唯一可用且支持视觉，SSE 标准格式 |
| n/gemma-4 | ❌ 连接断（HTTP 000） | ❌ | 上游不可用 |
| n/mistral-large-2 | 上游认证不可用 | ❌ | 纯文本 LLM，auth_unavailable |
| n/llama-3.1-nemotron | 上游认证不可用 | ❌ | 纯文本 LLM，auth_unavailable |

集成冒烟：`LLMRENAME_TEST_KEY=<key> cargo test vision_stream_smoke -- --ignored`（流式提取 + 字段 JSON 解析 + 增量回调非空）。