# Installs the latest build on the headset: over Wi-Fi if wifi-setup.ps1 was run, otherwise over the cable.
# The headset must be awake (put it on for a second) and on the same network as this PC.
param([string]$Apk = "$PSScriptRoot\dist\FantomBevy.apk")
$quest = "2G97C5ZH93002K"
$target = $null
if ((adb devices) -match "^$quest\s+device") {
    $target = $quest
} elseif (Test-Path "$PSScriptRoot\quest-ip.txt") {
    $ip = (Get-Content "$PSScriptRoot\quest-ip.txt" -TotalCount 1).Trim()
    adb connect "${ip}:5555" | Out-Null
    if ((adb devices) -match "^${ip}:5555\s+device") { $target = "${ip}:5555" }
}
if (-not $target) {
    Write-Host "Cannot reach the headset. Wake it up; if it was restarted, plug in the cable and run wifi-setup.ps1 once." -ForegroundColor Yellow
    exit 1
}
Write-Host "Installing on $target ..."
adb -s $target install -r $Apk
