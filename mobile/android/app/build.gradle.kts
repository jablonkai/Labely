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

android {
    namespace = "org.labely.app"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }

    defaultConfig {
        applicationId = "org.labely.app"
        minSdk { version = release(26) }
        targetSdk { version = release(37) }
        versionCode = 1
        versionName = "0.1.0"
        // Only the ABIs the engine is built for; JNA alone would add mips, armeabi and x86.
        ndk {
            abiFilters +=
                providers.gradleProperty("labely.abis").get().split(",").map(String::trim).filter(String::isNotEmpty)
        }
    }

    buildTypes {
        release {
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
