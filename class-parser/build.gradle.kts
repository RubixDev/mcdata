plugins {
    kotlin("jvm") version "1.9.23"
    kotlin("plugin.serialization") version "1.9.23"
    application
}

group = "de.rubixdev"
version = "1.0-SNAPSHOT"

application {
    mainClass = "de.rubixdev.MainKt"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.apache.bcel:bcel:6.9.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3")
}

kotlin {
    jvmToolchain(21)
}
