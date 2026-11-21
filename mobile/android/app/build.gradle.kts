import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.ksp)
}

val repoRoot: File = rootDir.resolve("../..").canonicalFile
val cargoBin: String =
    providers.environmentVariable("CARGO").orNull
        ?: File(System.getProperty("user.home"), ".cargo/bin/cargo").takeIf(File::exists)?.path
        ?: "cargo"

// The version is the workspace version of Cargo.toml (ADR-0039).
val labelyVersion: String =
    Regex("""(?m)^version\s*=\s*"([^"]+)"""")
        .find(providers.fileContents(layout.projectDirectory.file("../../../Cargo.toml")).asText.get())
        ?.groupValues
        ?.get(1)
        ?: error("Cargo.toml has no workspace version")

// Play wants a version code that grows: 1.2.3 is 1002003. `labely.versionCode` overrides it,
// for a second upload of the same version.
val labelyVersionCode: Int =
    providers.gradleProperty("labely.versionCode").orNull?.toInt()
        ?: labelyVersion.substringBefore('-').split('.').map(String::toInt).let { (major, minor, patch) ->
            major * 1_000_000 + minor * 1_000 + patch
        }

// The upload key of a release build. Without the four variables the release is not signed.
val keystoreFile: String? = providers.environmentVariable("LABELY_KEYSTORE_FILE").orNull?.takeIf(String::isNotEmpty)

android {
    namespace = "org.labely.app"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }

    defaultConfig {
        applicationId = "org.labely.app"
        minSdk { version = release(26) }
        targetSdk { version = release(37) }
        versionCode = labelyVersionCode
        versionName = labelyVersion
        // Only the ABIs the engine is built for; JNA alone would add mips, armeabi and x86.
        ndk {
            abiFilters +=
                providers.gradleProperty("labely.abis").get().split(",").map(String::trim).filter(String::isNotEmpty)
        }
    }

    signingConfigs {
        if (keystoreFile != null) {
            create("upload") {
                storeFile = file(keystoreFile)
                storePassword = providers.environmentVariable("LABELY_KEYSTORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("LABELY_KEY_ALIAS").get()
                keyPassword = providers.environmentVariable("LABELY_KEY_PASSWORD").get()
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("upload")
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }

    // Per-app language (Settings → Language) from res/values-* (ADR-0027).
    androidResources {
        generateLocaleConfig = true
    }

    // Robolectric reads the merged manifest and resources (AppFunctions tests, ADR-0028).
    testOptions {
        unitTests.isIncludeAndroidResources = true
    }
}

/** Builds the engine for the host, so Robolectric tests run the real engine through JNA (ADR-0028). */
abstract class CargoHostTask @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val cargo: Property<String>
    @get:Internal abstract val workspace: DirectoryProperty

    @TaskAction
    fun build() {
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(cargo.get(), "build", "-p", "labely-ffi", "--lib")
        }
    }
}

val cargoHost =
    tasks.register<CargoHostTask>("cargoHost") {
        group = "rust"
        description = "Builds the Labely engine for the host, for unit tests."
        cargo = cargoBin
        workspace = layout.dir(provider { repoRoot })
        // cargo decides what is up to date.
        outputs.upToDateWhen { false }
    }

tasks.withType<Test>().configureEach {
    dependsOn(cargoHost)
    systemProperty("jna.library.path", repoRoot.resolve("target/debug").path)
}

dependencies {
    implementation(project(":ffi"))
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.activity.compose)
    debugImplementation(libs.compose.ui.tooling)

    // AppFunctions (ADR-0011, ADR-0028).
    implementation(libs.appfunctions)
    ksp(libs.appfunctions.compiler)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)
    testImplementation(libs.appfunctions.testing)
    // The desktop JNA jar carries the host's jnidispatch; the app's JNA is the Android aar.
    testImplementation(libs.jna)
}
