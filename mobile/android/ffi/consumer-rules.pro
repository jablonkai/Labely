# JNA and the UniFFI bindings are reached by reflection and from native code.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-keep class org.labely.ffi.** { *; }
-dontwarn java.awt.**
