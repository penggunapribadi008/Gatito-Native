package com.streamdeckhp.app

import android.os.Handler
import android.os.Looper
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.EnumMap
import java.util.concurrent.Executors

/** Pemindai QR layar penuh: kamera belakang, teks di bawah, tombol Batal putih (seperti #scan di versi web) */
@Composable
fun ScanOverlay(onResult: (String?) -> Unit) {
    val ctx = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val pv = remember { PreviewView(ctx).apply { scaleType = PreviewView.ScaleType.FILL_CENTER } }

    DisposableEffect(Unit) {
        val exec = Executors.newSingleThreadExecutor()
        val future = ProcessCameraProvider.getInstance(ctx)
        var provider: ProcessCameraProvider? = null
        var done = false
        future.addListener({
            val p = future.get()
            provider = p
            val preview = Preview.Builder().build().also { it.setSurfaceProvider(pv.surfaceProvider) }
            val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
            val hints = EnumMap<DecodeHintType, Any>(DecodeHintType::class.java)
            hints[DecodeHintType.POSSIBLE_FORMATS] = listOf(BarcodeFormat.QR_CODE)
            hints[DecodeHintType.TRY_HARDER] = true
            val reader = MultiFormatReader().apply { setHints(hints) }
            analysis.setAnalyzer(exec) { img ->
                try {
                    if (!done) {
                        val plane = img.planes[0]
                        val buf = plane.buffer
                        val data = ByteArray(buf.remaining())
                        buf.get(data)
                        val src = PlanarYUVLuminanceSource(data, plane.rowStride, img.height, 0, 0, img.width, img.height, false)
                        val r = reader.decodeWithState(BinaryBitmap(HybridBinarizer(src)))
                        done = true
                        Handler(Looper.getMainLooper()).post { onResult(r.text) }
                    }
                } catch (e: Exception) {
                } finally {
                    reader.reset()
                    img.close()
                }
            }
            try {
                p.unbindAll()
                p.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
            } catch (e: Exception) {
            }
        }, ContextCompat.getMainExecutor(ctx))
        onDispose {
            done = true
            provider?.unbindAll()
            exec.shutdown()
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView({ pv }, Modifier.fillMaxSize())
        Column(Modifier.align(Alignment.BottomCenter), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                "Arahkan kamera ke kode QR di PC", color = Color(0xFFF2F3F7), fontSize = 16.sp,
                modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(Color(0x99000000)).padding(horizontal = 14.dp, vertical = 8.dp),
            )
            Box(
                Modifier.padding(top = 12.dp, bottom = 28.dp).clip(RoundedCornerShape(12.dp)).background(Color.White).clickable { onResult(null) }.padding(horizontal = 28.dp, vertical = 12.dp),
                contentAlignment = Alignment.Center,
            ) { Text("Batal", color = Color.Black, fontWeight = FontWeight.Bold, fontSize = 16.sp) }
        }
    }
}
