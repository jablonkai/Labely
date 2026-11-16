plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

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
}

dependencies {
    implementation(project(":ffi"))
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.activity.compose)
    debugImplementation(libs.compose.ui.tooling)
}
