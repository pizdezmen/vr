# Builds the Quest APK. Result: target\release\apk\FantomBevy.apk
# Uses the Android SDK from Android Studio and the NDK that came with Unity.
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_ROOT = "C:\Program Files\Unity\Hub\Editor\6000.6.9f1\Editor\Data\PlaybackEngines\AndroidPlayer\NDK"
Set-Location $PSScriptRoot
cargo apk build --release --lib
