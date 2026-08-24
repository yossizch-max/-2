param(
  [Parameter(Mandatory=$true)][string]$Manifest,
  [string]$SourceRoot="."
)
$ErrorActionPreference="Stop"

$config=Get-Content $Manifest -Raw | ConvertFrom-Json
$ocrRoot=Join-Path $SourceRoot "src-tauri\resources\ocr"
$temp=Join-Path ([System.IO.Path]::GetTempPath()) ("tahrir-ocr-"+[guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $temp | Out-Null

function Fetch-Verified($item,$name){
  if(!$item.url -or !$item.sha256 -or $item.sha256 -notmatch '^[a-fA-F0-9]{64}$'){
    throw "$name is not pinned with a valid SHA256"
  }
  $out=Join-Path $temp $name
  Invoke-WebRequest -Uri $item.url -OutFile $out
  $actual=(Get-FileHash $out -Algorithm SHA256).Hash.ToLowerInvariant()
  if($actual -ne $item.sha256.ToLowerInvariant()){throw "$name SHA256 mismatch"}
  return $out
}

try {
  $tesseract=Fetch-Verified $config.tesseract "tesseract.pkg"
  $poppler=Fetch-Verified $config.poppler "poppler.zip"
  $tessdata=Fetch-Verified $config.tessdata "tessdata.zip"

  Write-Host "All OCR archives downloaded and SHA256 verified."
  Write-Host "Extraction/staging is distribution-specific and must copy runtimes into:"
  Write-Host "  $ocrRoot\vendor\tesseract"
  Write-Host "  $ocrRoot\vendor\poppler"
  Write-Host "  $ocrRoot\tessdata"
  throw "FAIL-CLOSED: runtime extraction rules must be approved for the selected distributions before release."
}
finally {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}
