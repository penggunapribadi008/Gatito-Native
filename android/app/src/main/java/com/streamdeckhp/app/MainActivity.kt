package com.streamdeckhp.app

import android.os.Build
import android.os.Bundle
import android.view.MotionEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.Color
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val vm: DeckVM by viewModels()

    private val camPerm = registerForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) {
            vm.scanning = true
        } else {
            lifecycleScope.launch {
                vm.ask("Izin kamera dibutuhkan", "Izinkan kamera untuk aplikasi ini agar bisa scan QR.")
                vm.start()
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 28) {
            val lp = window.attributes
            lp.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            window.attributes = lp
        }
        hideBars()
        vm.finishApp = { finish() }
        vm.launchScan = {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) vm.scanning = true
            else camPerm.launch(Manifest.permission.CAMERA)
        }
        setContent {
            MaterialTheme(
                colorScheme = darkColorScheme(
                    primary = Color(0xFF4F6DF5),
                    background = Color(0xFF0E0F12),
                    surface = Color(0xFF1D2028),
                    onSurface = Color(0xFFF2F3F7),
                    onBackground = Color(0xFFF2F3F7),
                ),
            ) {
                // Layar tetap menyala selama aplikasi dibuka
                LaunchedEffect(vm.keepOn) {
                    if (vm.keepOn) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                    else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                }
                AppRoot(vm)
            }
        }
    }

    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        if (ev.actionMasked == MotionEvent.ACTION_DOWN) vm.bump()
        return super.dispatchTouchEvent(ev)
    }

    override fun onResume() {
        super.onResume()
        vm.bump()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideBars()
    }

    /** Layar penuh: bar sistem disembunyikan, muncul sebentar kalau digeser dari tepi */
    private fun hideBars() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val c = WindowCompat.getInsetsController(window, window.decorView)
        c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        c.hide(WindowInsetsCompat.Type.systemBars())
    }
}
