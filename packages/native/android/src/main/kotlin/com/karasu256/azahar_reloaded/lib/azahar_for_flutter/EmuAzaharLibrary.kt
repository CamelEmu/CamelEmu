package com.karasu256.azahar_reloaded.lib.azahar_for_flutter

import android.view.Surface
import androidx.annotation.Keep

/**
 * The library of the Rust core. A game that runs only in the Rust core gets its screen surfaces
 * from here, without going through the C++ core.
 */
object EmuAzaharLibrary {
    init {
        System.loadLibrary("emu_azahar")
    }

    /** Hands [surface] to the Rust core, or takes it away when it is null. */
    @Keep
    @JvmStatic
    external fun surfaceChanged(surface: Surface?, isSecondary: Boolean)
}
