# CodexTransfer

CodexTransfer  是以[codex-relay](https://github.com/anthropics/codex-relay) 为基础的API转接，为 Codex APP 提供多模型供应商的转接服务。

## 特别感谢

本项目基于 **[codex-relay](https://github.com/MetaFARS/codex-relay)** 构建，感谢原项目作者及贡献者提供了优秀的 Codex 本地中继方案。codex-relay 负责将 Codex APP 的请求转发到上游 API.

## 支持的模型供应商

| 供应商      | 模型            |
| ----------- | --------------- |
| DeepSeek    | deepseek-v4-pro |
| Qwen        | qwen3-max       |
| SiliconFlow | DeepSeek-V3     |
| Zhipu       | glm-5.1         |

## 前置依赖

### 1. 安装 codex-relay

```bash
pip install codex-relay
```

### 2. 验证安装

```bash
codex-relay --help
```

确保上述命令正常输出帮助信息后再继续。

### 3. Python 环境

脚本需要 Python 环境来运行 `model-proxy.py`。

## 启动方式

在项目目录下运行 `run.ps1`：

```powershell
.\run.ps1
```

### 权限提示（默认无法运行 .ps1）

如果提示“在此系统上禁止运行脚本”，请在 PowerShell 中执行以下任一方式：

```powershell
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

或仅本次绕过：

```powershell
powershell -ExecutionPolicy Bypass -File .\run.ps1
```

脚本会引导你完成以下步骤：

1. **初始化配置** — 将 `config.toml` 复制到 `~/.codex/` 目录，并选择模型供应商
2. **设置 API Key** — 输入对应供应商的 API Key（写入用户环境变量）
3. **启动服务** — 后台启动 codex-relay（端口 4446）和模型代理（端口 4447）

启动成功后，重新打开 Codex APP 即可使用。

## 文件说明

| 文件                | 说明                             |
| ------------------- | -------------------------------- |
| `run.ps1`           | 主启动脚本，交互式配置和启动     |
| `config.toml`       | Codex APP 的供应商配置文件模版   |
| `model-proxy.py`    | 模型名称转换代理                 |
| `.current_provider` | 记录当前选择的供应商（自动生成） |

## 端口说明

| 服务           | 端口 |
| -------------- | ---- |
| codex-relay    | 4446 |
| model-proxy    | 4447 |
| Codex APP 连接 | 4447 |
