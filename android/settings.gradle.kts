pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // gpui-android-glue's Java half. GitHub Packages asks for a token even
        // to read: GITHUB_TOKEN on CI, `gpr.user`/`gpr.key` in
        // ~/.gradle/gradle.properties at a desk.
        maven {
            url = uri("https://maven.pkg.github.com/akergez/gpui-android-glue")
            credentials {
                username = providers.environmentVariable("GITHUB_ACTOR").orNull
                    ?: providers.gradleProperty("gpr.user").orNull
                password = providers.environmentVariable("GITHUB_TOKEN").orNull
                    ?: providers.gradleProperty("gpr.key").orNull
            }
        }
    }
}

rootProject.name = "MoneyManager"
include(":app")

// The checkout beside this one stands in for the published library.
// TODO: only with -PglueLocal once com.akergez.gpui:android-glue is published.
includeBuild("../../gpui-android-glue/android")
