param([string]$SourceRoot=".")
$ErrorActionPreference="Stop"

Push-Location $SourceRoot
try {
  if(!(Test-Path package-lock.json)){throw "package-lock.json missing"}
  if(!(Test-Path src-tauri\Cargo.lock)){throw "src-tauri\Cargo.lock missing"}

  $npmBefore=(Get-FileHash package-lock.json -Algorithm SHA256).Hash
  $cargoBefore=(Get-FileHash src-tauri\Cargo.lock -Algorithm SHA256).Hash

  npm ci
  npm audit --audit-level=high
  npm run contract:check
  npm run qa:static
  npm run build

  Push-Location src-tauri
  try {
    cargo check --locked
    cargo test --locked -- --test-threads=1
  } finally { Pop-Location }

  npm run desktop:build

  if($npmBefore -ne (Get-FileHash package-lock.json -Algorithm SHA256).Hash){
    throw "package-lock.json changed during build"
  }
  if($cargoBefore -ne (Get-FileHash src-tauri\Cargo.lock -Algorithm SHA256).Hash){
    throw "Cargo.lock changed during build"
  }
} finally { Pop-Location }
