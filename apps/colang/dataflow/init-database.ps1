#!/usr/bin/env pwsh
# Initialize English Learning Companion Database
# This script creates the SQLite database and populates it with seed data

$ErrorActionPreference = "Stop"

$dbPath = Join-Path $PSScriptRoot "learning_companion.db"
$migrationsPath = Join-Path (Split-Path $PSScriptRoot -Parent) "migrations"
$seedDataPath = Join-Path (Split-Path $PSScriptRoot -Parent) "seed_data.sql"

Write-Host "Initializing English Learning Companion Database..." -ForegroundColor Cyan
Write-Host "Database path: $dbPath" -ForegroundColor Gray

# Check if database already exists
if (Test-Path $dbPath) {
    $response = Read-Host "Database already exists. Recreate? (y/N)"
    if ($response -ne "y" -and $response -ne "Y") {
        Write-Host "Keeping existing database." -ForegroundColor Yellow
        exit 0
    }
    Remove-Item $dbPath -Force
    Write-Host "Removed existing database." -ForegroundColor Yellow
}

# Check if sqlite3 is available
$sqliteCmd = Get-Command sqlite3 -ErrorAction SilentlyContinue
if (-not $sqliteCmd) {
    Write-Host "Error: sqlite3 command not found. Please install SQLite." -ForegroundColor Red
    Write-Host "Download from: https://www.sqlite.org/download.html" -ForegroundColor Yellow
    exit 1
}

# Run migrations
Write-Host "`nApplying migrations..." -ForegroundColor Cyan
$migrationFiles = Get-ChildItem -Path $migrationsPath -Filter "*.sql" | Sort-Object Name

foreach ($migrationFile in $migrationFiles) {
    Write-Host "  - $($migrationFile.Name)" -ForegroundColor Gray
    Get-Content $migrationFile.FullName | sqlite3 $dbPath
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error applying migration: $($migrationFile.Name)" -ForegroundColor Red
        exit 1
    }
}

Write-Host "Migrations completed successfully." -ForegroundColor Green

# Load seed data
if (Test-Path $seedDataPath) {
    Write-Host "`nLoading seed data..." -ForegroundColor Cyan
    Get-Content $seedDataPath | sqlite3 $dbPath
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Warning: Some seed data may not have loaded correctly." -ForegroundColor Yellow
    } else {
        Write-Host "Seed data loaded successfully." -ForegroundColor Green
    }
} else {
    Write-Host "`nWarning: Seed data file not found at $seedDataPath" -ForegroundColor Yellow
}

# Verify database
Write-Host "`nVerifying database..." -ForegroundColor Cyan
$tableCount = sqlite3 $dbPath "SELECT COUNT(*) FROM sqlite_master WHERE type='table';"
Write-Host "  - Tables created: $tableCount" -ForegroundColor Gray

$wordCount = sqlite3 $dbPath "SELECT COUNT(*) FROM issue_words;"
Write-Host "  - Issue words: $wordCount" -ForegroundColor Gray

Write-Host "`nDatabase initialization complete!" -ForegroundColor Green
Write-Host "Database location: $dbPath" -ForegroundColor Cyan
