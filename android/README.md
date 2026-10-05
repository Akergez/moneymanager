# Money Manager for Android

The application is the same Rust crate as on the desktop, built as a shared
object and loaded by a `NativeActivity`. The platform layer is
[gpui-mobile](https://github.com/longbridge/gpui-mobile); the Java classes in
`app/src/main/java/dev/gpui/mobile` and the Gradle project come from its
example, under the same licence terms.

The ledger and the settings are kept in the application's private directory
(`android_main` in `crates/money-manager/src/lib.rs` points the XDG variables
at it). The manifest declares the network permission and nothing else: no
storage permission exists to be asked for.

## What is needed

- The Android SDK with a platform (`android-36`) and build tools, and an NDK
  (r27 is what `app/build.gradle.kts` names).
- `cargo install cargo-ndk`, and `rustup target add aarch64-linux-android`.
- A JDK, 17 or later.

## Building

From the repository root, the library first:

```sh
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973

cargo ndk -t arm64-v8a --platform 26 -o android/app/src/main/jniLibs \
    build -p money-manager --lib --release
```

Build it `--release`: a debug library is very large, and the renderer's debug
labels crash the emulator's Vulkan driver.

and then the package:

```sh
cd android
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

The log goes to logcat under the tag `money-manager`.

## A package to hand out

`build-aux/build-android.sh` does both steps with the `dist` profile and
leaves a release package in `dist/`; it is what CI runs. The package's version
is the one in the workspace's `Cargo.toml`, or the one the script is given,
which also asks for the release key: see "Signing the Android package" in the
top-level README.
