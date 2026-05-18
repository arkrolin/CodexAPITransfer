$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$configPath = "$env:USERPROFILE\.codex\config.toml"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Codex APP 转接服务" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 1. 询问是否初始化
$initChoice = Read-Host "是否进行初始化配置 (Y/N)?"
if ($initChoice -match "^[Yy]") {
    if (-not (Test-Path "$env:USERPROFILE\.codex")) {
        New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.codex" | Out-Null
    }
    
    # 替换 config.toml 到目标位置
    if (Test-Path "$scriptDir\config.toml") {
        Copy-Item -Path "$scriptDir\config.toml" -Destination $configPath -Force
        Write-Host "已复制 config.toml 到 $configPath" -ForegroundColor Green
    }
    else {
        Write-Host "[警告] 当前目录下未找到 config.toml！跳过复制文件步骤。" -ForegroundColor Yellow
    }

    # 提示用户选择本地代理商
    Write-Host ""
    Write-Host "请选择模型商:" -ForegroundColor Yellow
    Write-Host "[1] DeepSeek (deepseek-v4-pro)" -ForegroundColor Blue
    Write-Host "[2] Kimi (kimi-k2.6)" -ForegroundColor Magenta
    Write-Host "[3] Qwen (qwen-max)" -ForegroundColor Cyan
    Write-Host "[4] SiliconFlow (DeepSeek-V3)" -ForegroundColor DarkCyan
    Write-Host "[5] Zhipu (glm-4-plus)" -ForegroundColor DarkMagenta
    $proxyChoice = Read-Host "请选择 (1-5, 默认: 1)"
    
    $providerName = "deepseek"
    $newProvider = "deepseek-relay"
    $newModel = "deepseek-v4-pro"
    $newReview = "deepseek-v4-flash"
    $envPrefix = "DEEPSEEK"
    $upstream = "https://api.deepseek.com/v1"
    
    switch ($proxyChoice) {
        "2" {
            $providerName = "kimi"
            $newProvider = "kimi-relay"
            $newModel = "kimi-k2.6"
            $newReview = "kimi-k2.5"
            $envPrefix = "KIMI"
            $upstream = "https://api.moonshot.cn/v1"
        }
        "3" {
            $providerName = "qwen"
            $newProvider = "qwen-relay"
            $newModel = "qwen3-max"
            $newReview = "qwen3.5-plus"
            $envPrefix = "QWEN"
            $upstream = "https://dashscope.aliyuncs.com/compatible-mode/v1"
        }
        "4" {
            $providerName = "siliconflow"
            $newProvider = "siliconflow-relay"
            $newModel = "deepseek-ai/DeepSeek-V3"
            $newReview = "Qwen/Qwen3-235B-A22B"
            $envPrefix = "SILICONFLOW"
            $upstream = "https://api.siliconflow.cn/v1"
        }
        "5" {
            $providerName = "zhipu"
            $newProvider = "zhipu-relay"
            $newModel = "glm-5.1"
            $newReview = "glm-4-flash"
            $envPrefix = "ZHIPU"
            $upstream = "https://open.bigmodel.cn/api/paas/v4"
        }
    }
    
    # 写入文件名为记录当前选择的provider，方便后续启动读取
    Set-Content -Path "$scriptDir\.current_provider" -Value "$providerName`n$envPrefix`n$upstream"

    if ($true) {
        $newEffort = "xhigh"
        $newContext = "model_context_window = 1000000"
        $newCompact = "model_auto_compact_token_limit = 900000"
        
        if (Test-Path $configPath) {
            $config = Get-Content $configPath -Raw -Encoding UTF8
            
            $lines = $config -split "`n"
            $newLines = @()
            $providerReplaced = $false
            $modelReplaced = $false
            $reviewReplaced = $false
            $effortReplaced = $false
            $contextReplaced = $false
            $compactReplaced = $false

            foreach ($line in $lines) {
                $trimmed = $line.TrimStart()
                if ($trimmed -match '^model_provider\s*=') {
                    if (-not $providerReplaced) {
                        $newLines += "model_provider = `"$newProvider`""
                        $providerReplaced = $true
                    }
                }
                elseif ($trimmed -match '^model\s*=' -and $trimmed -notmatch 'model_provider|model_reasoning|model_context|model_auto') {
                    if (-not $modelReplaced) {
                        $newLines += "model = `"$newModel`""
                        $modelReplaced = $true
                    }
                }
                elseif ($trimmed -match '^review_model\s*=') {
                    if (-not $reviewReplaced) {
                        $newLines += "review_model = `"$newReview`""
                        $reviewReplaced = $true
                    }
                }
                elseif ($trimmed -match '^model_reasoning_effort\s*=') {
                    if (-not $effortReplaced) {
                        $newLines += "model_reasoning_effort = `"$newEffort`""
                        $effortReplaced = $true
                    }
                }
                elseif ($trimmed -match '^#?\s*model_context_window\s*=') {
                    if (-not $contextReplaced) {
                        $newLines += $newContext
                        $contextReplaced = $true
                    }
                }
                elseif ($trimmed -match '^#?\s*model_auto_compact_token_limit\s*=') {
                    if (-not $compactReplaced) {
                        $newLines += $newCompact
                        $compactReplaced = $true
                    }
                }
                else {
                    $newLines += $line
                }
            }

            if (-not $providerReplaced) { $newLines += "model_provider = `"$newProvider`"" }
            if (-not $modelReplaced) { $newLines += "model = `"$newModel`"" }
            if (-not $reviewReplaced) { $newLines += "review_model = `"$newReview`"" }
            if (-not $effortReplaced) { $newLines += "model_reasoning_effort = `"$newEffort`"" }
            if (-not $contextReplaced) { $newLines += $newContext }
            if (-not $compactReplaced) { $newLines += $newCompact }

            $newConfig = $newLines -join "`r`n"
            [System.IO.File]::WriteAllText($configPath, $newConfig, [System.Text.UTF8Encoding]::new($false))
            Write-Host "已在 config.toml 中设置本地代理。" -ForegroundColor Green
        }
        
        Write-Host ""
        $envKeyName = "${envPrefix}_API_KEY"
        $userKey = Read-Host "请输入你的 $envPrefix API Key"
        if (-not [string]::IsNullOrWhiteSpace($userKey)) {
            [Environment]::SetEnvironmentVariable($envKeyName, $userKey, "User")
            Set-Item -Path "Env:$envKeyName" -Value $userKey
            Write-Host "$envKeyName 已成功设置为用户环境变量。" -ForegroundColor Green
        }
    }
}

Write-Host ""
$startChoice = Read-Host "是否启动服务? (Y/N)"

if ($startChoice -match "^[Yy]") {
    $providerName = "deepseek"
    $envPrefix = "DEEPSEEK"
    $upstream = "https://api.deepseek.com/v1"
    if (Test-Path "$scriptDir\.current_provider") {
        $pInfo = Get-Content "$scriptDir\.current_provider" | Where-Object { $_ -ne "" }
        if ($pInfo.Count -ge 3) {
            $providerName = $pInfo[0]
            $envPrefix = $pInfo[1]
            $upstream = $pInfo[2]
        }
    }

    $envKeyName = "${envPrefix}_API_KEY"
    $apiKey = [Environment]::GetEnvironmentVariable($envKeyName, "User")
    if (-not $apiKey) {
        $apiKey = Get-Content Env:$envKeyName -ErrorAction SilentlyContinue
    }

    if (-not $apiKey) {
        Write-Host "[错误] 未找到 $envKeyName 环境变量，请重新运行脚本进行初始化配置。" -ForegroundColor Red
        pause
        exit 1
    }

    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  Codex + $providerName 服务启动中..." -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""

    # 启动 codex-relay
    Write-Host "[1/2] 启动 codex-relay (端口 4446, 上游: $upstream)..." -ForegroundColor Green
    $env:CODEX_RELAY_UPSTREAM = $upstream
    $env:CODEX_RELAY_API_KEY = $apiKey
    $env:CODEX_RELAY_PORT = "4446"

    $relayJob = Start-Job -Name "codex-relay" -ScriptBlock {
        $env:CODEX_RELAY_UPSTREAM = $using:env:CODEX_RELAY_UPSTREAM
        $env:CODEX_RELAY_API_KEY = $using:env:CODEX_RELAY_API_KEY
        $env:CODEX_RELAY_PORT = "4446"
        codex-relay 2>&1 | Out-File "$env:TEMP\codex-relay.log"
    }

    Start-Sleep -Seconds 2
    Write-Host "  -> relay 已启动" -ForegroundColor Gray

    # 启动模型代理
    Write-Host "[2/2] 启动模型转接 (端口 4447)..." -ForegroundColor Green
    $proxyJob = Start-Job -Name "model-proxy" -ScriptBlock {
        param($scriptDir)
        python "$scriptDir\model-proxy.py" 2>&1 | Out-File "$env:TEMP\codex-proxy.log"
    } -ArgumentList $scriptDir

    Start-Sleep -Seconds 1
    Write-Host "  -> 转接已启动" -ForegroundColor Gray

    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  启动成功，请重新打开 Codex APP" -ForegroundColor Cyan
    Write-Host "  按 Ctrl+C 停止所有服务" -ForegroundColor Yellow
    Write-Host "========================================" -ForegroundColor Cyan

    try {
        while ($true) {
            Start-Sleep -Seconds 1
        }
    }
    finally {
        Write-Host ""
        Write-Host "接收到停止信号，正在清理服务..." -ForegroundColor Yellow

        Stop-Job -Name "codex-relay" -ErrorAction SilentlyContinue
        Remove-Job -Name "codex-relay" -ErrorAction SilentlyContinue
        Stop-Job -Name "model-proxy" -ErrorAction SilentlyContinue
        Remove-Job -Name "model-proxy" -ErrorAction SilentlyContinue

        Write-Host "已停止，可以关闭此窗口。" -ForegroundColor Green
    }
}
else {
    Write-Host "服务取消启动。"
}