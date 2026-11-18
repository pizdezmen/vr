# Type-checks the code for the headset without building the APK (fast).
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_ROOT = "C:\Program Files\Unity\Hub\Editor\6000.6.9f1\Editor\Data\PlaybackEngines\AndroidPlayer\NDK"
$env:ANDROID_NDK_HOME = $env:ANDROID_NDK_ROOT
$tc = "$env:ANDROID_NDK_ROOT\toolchains\llvm\prebuilt\windows-x86_64\bin"
$env:CC_aarch64_linux_android = "$tc\aarch64-linux-android29-clang.cmd"
$env:AR_aarch64_linux_android = "$tc\llvm-ar.exe"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$tc\aarch64-linux-android29-clang.cmd"
Set-Location $PSScriptRoot
cargo check --release --lib --target aarch64-linux-android --message-format short
