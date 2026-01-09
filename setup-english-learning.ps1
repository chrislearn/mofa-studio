# English Learning Companion Setup Script

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "English Learning Companion - Setup" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if .env file exists
$envFile = "apps\colang\.env"
if (-Not (Test-Path $envFile)) {
    Write-Host "Creating .env file..." -ForegroundColor Yellow
    
    @"
# Doubao (豆包) Volcanic Engine API Credentials
# Get these from: https://www.volcengine.com
DOUBAO_APP_ID=your_app_id_here
DOUBAO_ACCESS_TOKEN=your_access_token_here
DOUBAO_API_KEY=your_api_key_here

# Database Configuration
DATABASE_URL=sqlite:///$(Get-Location)\apps\colang\learning_companion.db

# Logging
LOG_LEVEL=INFO

# Optional: Custom voice and language settings
VOICE_TYPE=BV700_V2_streaming
LANGUAGE=en
"@ | Out-File -FilePath $envFile -Encoding UTF8
    
    Write-Host "✓ Created .env file at $envFile" -ForegroundColor Green
    Write-Host "  Please edit this file and add your Doubao API credentials!" -ForegroundColor Yellow
    Write-Host ""
} else {
    Write-Host "✓ .env file already exists" -ForegroundColor Green
}

# Build all nodes
Write-Host "Building Dora nodes..." -ForegroundColor Cyan
Write-Host ""

$nodes = @(
    @{Name="Word Selector"; Path="rust-nodes\dora-word-selector"},
    @{Name="Topic Generator"; Path="rust-nodes\dora-topic-generator"},
    @{Name="Session Context"; Path="rust-nodes\dora-session-context"},
    @{Name="Doubao ASR"; Path="rust-nodes\dora-doubao-asr"},
    @{Name="Doubao TTS"; Path="rust-nodes\dora-doubao-tts"},
    @{Name="Conversation Analyzer"; Path="rust-nodes\dora-conversation-analyzer"}
)

foreach ($node in $nodes) {
    Write-Host "Building $($node.Name)..." -ForegroundColor Yellow
    $manifestPath = Join-Path $node.Path "Cargo.toml"
    
    if (Test-Path $manifestPath) {
        cargo build --release --manifest-path $manifestPath
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  ✓ $($node.Name) built successfully" -ForegroundColor Green
        } else {
            Write-Host "  ✗ Failed to build $($node.Name)" -ForegroundColor Red
        }
    } else {
        Write-Host "  ✗ Manifest not found: $manifestPath" -ForegroundColor Red
    }
    Write-Host ""
}

# Check database
$dbPath = "apps\colang\learning_companion.db"
if (Test-Path $dbPath) {
    Write-Host "✓ Database already exists at $dbPath" -ForegroundColor Green
} else {
    Write-Host "Database will be created on first run" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Setup Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "1. Edit apps\colang\.env with your Doubao API credentials"
Write-Host "2. Run: cd apps\colang\dataflow"
Write-Host "3. Run: dora start english-learning.yml"
Write-Host ""
Write-Host "For more information, see:" -ForegroundColor Cyan
Write-Host "  apps\colang\ENGLISH_LEARNING_README.md"
Write-Host ""
