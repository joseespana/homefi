import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// ---------------------------------------------------------------------------
// Versioning: the workspace version in the root Cargo.toml is the single
// source of truth (bump it with `cargo xtask bump <version>`).
// ---------------------------------------------------------------------------
val repoRoot: File = rootDir.resolve("../..").canonicalFile

val homefiVersion: String = Regex("""(?s)\[workspace\.package].*?\nversion\s*=\s*"([^"]+)"""")
    .find(repoRoot.resolve("Cargo.toml").readText())
    ?.groupValues?.get(1)
    ?: error("workspace version not found in Cargo.toml")

/** 1.2.3 -> 1_002_003. Pre-release suffixes (`-beta.1`) share the code of their base version. */
fun versionCodeOf(version: String): Int {
    val (major, minor, patch) = version.substringBefore('-').split('.').map(String::toInt)
    return major * 1_000_000 + minor * 1_000 + patch
}

// ---------------------------------------------------------------------------
// Rust: cargo-ndk builds libhomefi_ffi.so, uniffi-bindgen generates Kotlin.
// ---------------------------------------------------------------------------
val abis: List<String> = (findProperty("homefi.abis") as String? ?: "arm64-v8a")
    .split(',').map(String::trim).filter(String::isNotEmpty)
val skipRust: Boolean = (findProperty("homefi.skipRust") as String?).toBoolean()
val jniLibsDir: File = projectDir.resolve("src/main/jniLibs")
val bindingsDir: File = layout.buildDirectory.dir("generated/uniffi/kotlin").get().asFile
val cargoPath: String = listOf(System.getProperty("user.home") + "/.cargo/bin", System.getenv("PATH")).joinToString(File.pathSeparator)

android {
    namespace = "com.perseusoft.homefi"
    compileSdk = 36
    ndkVersion = findProperty("homefi.ndkVersion") as String? ?: "29.0.14206865"

    defaultConfig {
        // Astell&Kern players only install APKs dropped in a known app's slot
        // (OpenService/Download/Spotify.apk), so a local DAP build can take
        // that app's id: see docs/releasing.md.
        applicationId = findProperty("homefi.applicationId") as String? ?: "com.perseusoft.homefi"
        minSdk = (findProperty("homefi.minSdk") as String?)?.toInt() ?: 26
        targetSdk = 36
        versionName = homefiVersion
        versionCode = (findProperty("homefi.versionCode") as String?)?.toInt() ?: versionCodeOf(homefiVersion)
        ndk { abiFilters += abis }
    }

    signingConfigs {
        // Release signing comes from the environment (CI secrets) or
        // ~/.gradle/gradle.properties; without it the release APK is signed
        // with the debug key so it can still be installed for testing.
        create("release") {
            val store = System.getenv("HOMEFI_KEYSTORE_FILE") ?: findProperty("homefi.keystore.file") as String?
            if (store != null) {
                storeFile = file(store)
                storePassword = System.getenv("HOMEFI_KEYSTORE_PASSWORD") ?: findProperty("homefi.keystore.password") as String?
                keyAlias = System.getenv("HOMEFI_KEY_ALIAS") ?: findProperty("homefi.key.alias") as String?
                keyPassword = System.getenv("HOMEFI_KEY_PASSWORD") ?: findProperty("homefi.key.password") as String?
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            val release = signingConfigs.getByName("release")
            signingConfig = if (release.storeFile != null) release else signingConfigs.getByName("debug")
        }
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets.getByName("main").java.srcDir(bindingsDir)

    packaging {
        jniLibs.useLegacyPackaging = (findProperty("homefi.compressLibs") as String?).toBoolean()
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

kotlin {
    compilerOptions { jvmTarget.set(JvmTarget.JVM_17) }
}

androidComponents {
    onVariants { variant ->
        variant.outputs.forEach { output ->
            (output as? com.android.build.api.variant.impl.VariantOutputImpl)
                ?.outputFileName?.set("homefi-$homefiVersion-${variant.buildType}.apk")
        }
    }
}

val cargoNdkBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Builds libhomefi_ffi.so for $abis with cargo-ndk"
    workingDir = repoRoot
    inputs.dir(repoRoot.resolve("crates"))
    inputs.file(repoRoot.resolve("Cargo.toml"))
    outputs.dir(jniLibsDir)
    environment("PATH", cargoPath)
    val ndkDir = androidComponents.sdkComponents.ndkDirectory
    val ndkVersionHint = android.ndkVersion
    doFirst {
        if (System.getenv("ANDROID_NDK_HOME") == null) {
            runCatching { ndkDir.get().asFile.absolutePath }
                .onSuccess { environment("ANDROID_NDK_HOME", it) }
                .onFailure { logger.warn("NDK $ndkVersionHint not found, cargo-ndk will look for one itself") }
        }
    }
    commandLine(
        buildList {
            addAll(listOf("cargo", "ndk", "--platform", "26"))
            abis.forEach { addAll(listOf("-t", it)) }
            addAll(listOf("-o", jniLibsDir.absolutePath, "build", "-p", "homefi-ffi", "--release"))
        }
    )
}

val hostLibrary: String = when {
    org.gradle.internal.os.OperatingSystem.current().isMacOsX -> "libhomefi_ffi.dylib"
    org.gradle.internal.os.OperatingSystem.current().isWindows -> "homefi_ffi.dll"
    else -> "libhomefi_ffi.so"
}

val cargoHostBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Builds the host library and uniffi-bindgen"
    workingDir = repoRoot
    inputs.dir(repoRoot.resolve("crates/homefi-ffi/src"))
    outputs.file(repoRoot.resolve("target/debug/$hostLibrary"))
    environment("PATH", cargoPath)
    commandLine("cargo", "build", "-p", "homefi-ffi", "--features", "bindgen")
}

val uniffiBindings by tasks.registering(Exec::class) {
    group = "rust"
    description = "Generates the Kotlin bindings of homefi-ffi"
    dependsOn(cargoHostBuild)
    workingDir = repoRoot
    inputs.file(repoRoot.resolve("target/debug/$hostLibrary"))
    outputs.dir(bindingsDir)
    environment("PATH", cargoPath)
    doFirst { bindingsDir.deleteRecursively() }
    commandLine(
        repoRoot.resolve("target/debug/uniffi-bindgen").absolutePath,
        "generate", "--library", repoRoot.resolve("target/debug/$hostLibrary").absolutePath,
        "--language", "kotlin", "--out-dir", bindingsDir.absolutePath, "--no-format",
    )
}

if (!skipRust) {
    tasks.named("preBuild") { dependsOn(cargoNdkBuild, uniffiBindings) }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.service)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material.icons)
    implementation(libs.zxing.core)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.coil.compose)
    implementation(libs.coil.network)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.session)
    implementation(libs.media3.ffmpeg)
    implementation(variantOf(libs.jna) { artifactType("aar") })
    debugImplementation(libs.androidx.compose.ui.tooling)
}
