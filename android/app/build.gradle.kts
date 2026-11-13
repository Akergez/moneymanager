// Packages the Rust library into an APK.
//
// Gradle does not build the library: `cargo ndk` does, into
// app/src/main/jniLibs/<abi>/, and that has to happen first. See
// android/README.md.

plugins {
    id("com.android.application")
}

// The version is the release's: CI hands over its tag, without the `v`. A
// build at the desk takes what the crates say.
val release: String = providers.environmentVariable("MONEY_MANAGER_VERSION").orNull
    ?.takeIf { it.isNotEmpty() }
    ?: Regex("""\[workspace\.package\][^\[]*?\nversion = "([^"]+)"""")
        .find(rootDir.parentFile.resolve("Cargo.toml").readText())!!
        .groupValues[1]
// The number the system compares to tell a newer package from an older one.
val (major, minor, patch) = release.split(".").map { it.toInt() }

// The key a release is signed with, as build-aux/build-android.sh hands it
// over. Without one the package is signed with this machine's debug key: it
// installs, but not over a package signed with another key.
val keystore: String? = providers.environmentVariable("ANDROID_KEYSTORE").orNull
    ?.takeIf { it.isNotEmpty() }

android {
    namespace = "app.akergez.moneymanager"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.akergez.moneymanager"
        minSdk = 26          // Vulkan 1.0 is mandatory from API 26
        targetSdk = 34
        versionCode = major * 1_000_000 + minor * 1_000 + patch
        versionName = release

        ndk {
            abiFilters += listOf("arm64-v8a")
        }

        // What NativeActivity loads: the cdylib's name.
        manifestPlaceholders["nativeLibraryName"] = "money_manager"
    }

    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS") ?: "money-manager"
                // A PKCS#12 keystore has one password for itself and its keys.
                keyPassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
                ?: signingConfigs.getByName("debug")
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isDebuggable = true
            isJniDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    // Strips the debug information out of the library on the way into the
    // package: a debug build of it is over a gigabyte.
    ndkVersion = "27.0.12077973"

    packaging {
        jniLibs {
            // gpui-mobile is a cdylib of its own as well, and `cargo ndk`
            // copies that next to ours. It is already linked into ours.
            excludes += "**/libgpui_mobile-*.so"
        }
    }

    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }
}

dependencies {
    // The Java half of gpui-android-glue: the classes gpui-mobile and the
    // glue call by name, and the androidx libraries they use. The same
    // version as the crate in crates/money-manager/Cargo.toml.
    implementation("com.akergez.gpui:android-glue:0.4.8")
}

// Gradle packs whatever library is in jniLibs; it does not build it. An APK
// built after a Rust change without rebuilding the library would carry the
// old one, and the interface would behave as the old code did — the system's
// font size and weight among it. The build script stamps the library with the
// commit it was built from; refuse to pack one stamped with another commit (or
// with none). A tree without git (a tarball) is not checked.
val rustLibrary = file("src/main/jniLibs/arm64-v8a/libmoney_manager.so")
val rustRevision = file("src/main/jniLibs/.rust-revision")
tasks.register("checkRustLibrary") {
    doLast {
        val head = try {
            val process = ProcessBuilder("git", "rev-parse", "HEAD")
                .directory(rootDir.parentFile)
                .redirectErrorStream(true)
                .start()
            val output = process.inputStream.bufferedReader().readText().trim()
            if (process.waitFor() == 0) output else null
        } catch (_: Exception) {
            null
        } ?: return@doLast
        if (!rustLibrary.isFile || !rustRevision.isFile || rustRevision.readText().trim() != head) {
            throw GradleException(
                "The Rust library in jniLibs is missing or was not built from this commit. " +
                    "Gradle does not build it; run build-aux/build-android.sh " +
                    "(or cargo ndk ... --platform 26 and stamp it) before packing the APK."
            )
        }
    }
}
tasks.named("preBuild") {
    dependsOn("checkRustLibrary")
}
