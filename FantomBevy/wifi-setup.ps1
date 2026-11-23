# Turns on installing over Wi-Fi. Run ONCE with the headset connected by cable
# (and again after every restart of the headset, because the headset forgets it on reboot).
# After that the cable can be unplugged and install.ps1 works over the network.
# The headset and the PC must be on the same Wi-Fi / home network.
$quest = "2G97C5ZH93002K"
$found = (adb devices) -match "^$quest\s+device"
if (-not $found) {
    Write-Host "Headset is not visible over the cable. Plug it in, put it on, allow USB debugging, and run this again." -ForegroundColor Yellow
    exit 1
}
$route = adb -s $quest shell ip -f inet addr show wlan0
$ip = ([regex]::Match(($route -join " "), "inet (\d+\.\d+\.\d+\.\d+)")).Groups[1].Value
if (-not $ip) {
    Write-Host "The headset is not connected to Wi-Fi. Connect it to the same network as this PC and run this again." -ForegroundColor Yellow
    exit 1
}
adb -s $quest tcpip 5555 | Out-Null
Start-Sleep -Seconds 3
$result = adb connect "${ip}:5555"
Set-Content -Path "$PSScriptRoot\quest-ip.txt" -Value $ip -Encoding ascii
Write-Host $result
Write-Host "Done. Headset address $ip saved. You can unplug the cable now." -ForegroundColor Green
