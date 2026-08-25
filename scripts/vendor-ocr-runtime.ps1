param(
  [string]$Manifest="config/ocr-runtime.json",
  [string]$SourceRoot="."
)
$ErrorActionPreference="Stop"

$config=Get-Content (Join-Path $SourceRoot $Manifest) -Raw | ConvertFrom-Json
$ocrRoot=Join-Path $SourceRoot "src-tauri\resources\ocr"
$tesseractDir=Join-Path $ocrRoot "vendor\tesseract"
$popplerDir=Join-Path $ocrRoot "vendor\poppler"
$tessdataDir=Join-Path $ocrRoot "tessdata"
$temp=Join-Path ([System.IO.Path]::GetTempPath()) ("tahrir-ocr-"+[guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $temp | Out-Null

function Fetch-Verified($url,$sha256,$name){
  if(!$url -or !$sha256 -or $sha256 -notmatch '^[a-fA-F0-9]{64}$'){
    throw "$name is not pinned with a valid SHA256"
  }
  $out=Join-Path $temp $name
  Invoke-WebRequest -Uri $url -OutFile $out
  $actual=(Get-FileHash $out -Algorithm SHA256).Hash.ToLowerInvariant()
  if($actual -ne $sha256.ToLowerInvariant()){throw "$name SHA256 mismatch: expected $sha256 got $actual"}
  return $out
}

try {
  New-Item -ItemType Directory -Force -Path $tesseractDir | Out-Null
  New-Item -ItemType Directory -Force -Path $popplerDir | Out-Null
  New-Item -ItemType Directory -Force -Path $tessdataDir | Out-Null

  # --- Tesseract (Inno Setup installer, extracted with innoextract - the installer
  #     is NEVER executed, so there is no risk of it hanging on a GUI/UAC prompt
  #     that can never be answered on a headless CI runner) ---
  $tesseractInstaller=Fetch-Verified $config.tesseract.url $config.tesseract.sha256 "tesseract-installer.exe"
  if($config.tesseract.installerKind -ne "innosetup"){
    throw "unrecognized tesseract installerKind '$($config.tesseract.installerKind)': extraction rules must be reviewed for a new installer format before release"
  }
  if(!(Get-Command innoextract -ErrorAction SilentlyContinue)){
    throw "innoextract is required to safely extract the Inno Setup installer (it must never be executed) but was not found on PATH"
  }
  $tesseractExtractDir=Join-Path $temp "tesseract-extract"
  New-Item -ItemType Directory -Force -Path $tesseractExtractDir | Out-Null
  & innoextract -e -d $tesseractExtractDir $tesseractInstaller
  if($LASTEXITCODE -ne 0){throw "innoextract exited with code $LASTEXITCODE"}
  $tesseractExe=Get-ChildItem -Path $tesseractExtractDir -Recurse -Filter "tesseract.exe" | Select-Object -First 1
  if(!$tesseractExe){throw "tesseract.exe not found in extracted installer contents"}
  Copy-Item $tesseractExe.FullName $tesseractDir -Force
  Get-ChildItem $tesseractExe.Directory -Filter "*.dll" | Copy-Item -Destination $tesseractDir -Force

  # --- Poppler (plain zip release, pdftotext/pdftoppm + DLLs staged out of Library\bin) ---
  $popplerZip=Fetch-Verified $config.poppler.url $config.poppler.sha256 "poppler.zip"
  $popplerExtractDir=Join-Path $temp "poppler-extract"
  Expand-Archive -Path $popplerZip -DestinationPath $popplerExtractDir -Force
  $popplerBin=Get-ChildItem -Path $popplerExtractDir -Recurse -Directory -Filter "bin" | Select-Object -First 1
  if(!$popplerBin){throw "poppler release archive layout changed: no 'bin' directory found, review before release"}
  Copy-Item (Join-Path $popplerBin.FullName "pdftotext.exe") $popplerDir -Force
  Copy-Item (Join-Path $popplerBin.FullName "pdftoppm.exe") $popplerDir -Force
  Get-ChildItem $popplerBin.FullName -Filter "*.dll" | Copy-Item -Destination $popplerDir -Force
  if(!(Test-Path (Join-Path $popplerDir "pdftotext.exe"))){throw "pdftotext.exe not found after poppler extraction"}
  if(!(Test-Path (Join-Path $popplerDir "pdftoppm.exe"))){throw "pdftoppm.exe not found after poppler extraction"}

  # --- tessdata (heb/ara/eng, each independently pinned and verified) ---
  foreach($lang in @("heb","ara","eng")){
    $entry=$config.tessdata.languages.$lang
    if(!$entry){throw "tessdata language '$lang' missing from manifest"}
    $file=Fetch-Verified $entry.url $entry.sha256 "$lang.traineddata"
    Copy-Item $file (Join-Path $tessdataDir "$lang.traineddata") -Force
  }

  Write-Host "OCR runtime vendored:"
  Write-Host "  $tesseractDir"
  Write-Host "  $popplerDir"
  Write-Host "  $tessdataDir"
}
finally {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}
