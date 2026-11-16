plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Versi dan nomor build diberikan CI lewat build-apk.sh
val appVersion: String = (project.findProperty("appVersion") as String?) ?: "3.0.4"
val appCode: Int = ((project.findProperty("appCode") as String?) ?: "1").toInt()
val updateRepo: String = (project.findProperty("updateRepo") as String?) ?: ""

android {
    namespace = "com.streamdeckhp.app"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.streamdeckhp.app"
        minSdk = 24
        targetSdk = 34
        versionCode = appCode
        versionName = appVersion
        buildConfigField("String", "UPDATE_REPO", "\"$updateRepo\"")
    }

    // Kunci rilis yang sama dengan versi Capacitor (alias streamdeckhp) supaya APK lama bisa di-update
    val ksFile = System.getenv("KEYSTORE_FILE")
    val ksPass = System.getenv("KEYSTORE_PASSWORD")
    if (ksFile != null && ksPass != null) {
        signingConfigs {
            create("release") {
                storeFile = file(ksFile)
                storePassword = ksPass
                keyAlias = "streamdeckhp"
                keyPassword = ksPass
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (ksFile != null && ksPass != null) signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    composeOptions { kotlinCompilerExtensionVersion = "1.5.14" }
    packaging { resources { excludes += "/META-INF/{AL2.0,LGPL2.1}" } }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.2")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.2")
    implementation(platform("androidx.compose:compose-bom:2024.06.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("io.coil-kt:coil-compose:2.7.0")
    implementation("io.coil-kt:coil-gif:2.7.0")
    implementation("androidx.camera:camera-core:1.3.4")
    implementation("androidx.camera:camera-camera2:1.3.4")
    implementation("androidx.camera:camera-lifecycle:1.3.4")
    implementation("androidx.camera:camera-view:1.3.4")
    implementation("com.google.zxing:core:3.5.3")
}
