# JNA + UniFFI generated bindings are reached through reflection / JNI.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class com.perseusoft.homefi.core.** { *; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# Media3 loads the FFmpeg renderer by reflection (DefaultRenderersFactory).
-keep class androidx.media3.decoder.ffmpeg.** { *; }
