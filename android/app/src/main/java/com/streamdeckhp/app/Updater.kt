package com.streamdeckhp.app

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.io.File
import java.util.concurrent.TimeUnit

/** Update aplikasi HP: cek rilis terbaru di GitHub, unduh APK, lalu pasang lewat penginstal Android */
object Updater {
    data class Release(val tag: String, val apk: String?)

    private fun verOf(v: String): List<Int> =
        v.removePrefix("v").removePrefix("V").split('.', '-', '+').take(3).map { it.toIntOrNull() ?: 0 }

    fun newer(a: String, b: String): Boolean {
        val x = verOf(a)
        val y = verOf(b)
        for (i in 0 until 3) {
            val p = x.getOrElse(i) { 0 }
            val q = y.getOrElse(i) { 0 }
            if (p != q) return p > q
        }
        return false
    }

    fun currentVersion(ctx: Context): String = try {
        ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName ?: ""
    } catch (e: Exception) {
        ""
    }

    suspend fun latest(repo: String): Release? = withContext(Dispatchers.IO) {
        try {
            val c = Net.client.newBuilder().callTimeout(8, TimeUnit.SECONDS).build()
            val req = Request.Builder().url("https://api.github.com/repos/$repo/releases/latest").header("Accept", "application/vnd.github+json").build()
            c.newCall(req).execute().use { r ->
                if (!r.isSuccessful) return@use null
                val j = JSONObject(r.body?.string() ?: return@use null)
                val tag = j.optString("tag_name")
                if (tag.isEmpty()) return@use null
                var apk: String? = null
                val assets = j.optJSONArray("assets")
                if (assets != null) {
                    for (i in 0 until assets.length()) {
                        val a = assets.getJSONObject(i)
                        if (a.optString("name").endsWith(".apk", ignoreCase = true)) {
                            apk = a.optString("browser_download_url")
                            break
                        }
                    }
                }
                Release(tag, apk)
            }
        } catch (e: Exception) {
            null
        }
    }

    fun canInstall(ctx: Context): Boolean = Build.VERSION.SDK_INT < 26 || ctx.packageManager.canRequestPackageInstalls()

    fun openInstallPermission(ctx: Context) {
        try {
            ctx.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + ctx.packageName)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (e: Exception) {
        }
    }

    /** Unduh APK dari GitHub (progres 0..99 lewat onPct). Mengembalikan berkasnya kalau ukuran cocok. */
    suspend fun download(ctx: Context, url: String, onPct: (Int) -> Unit): File? = withContext(Dispatchers.IO) {
        try {
            val f = File(ctx.cacheDir, "update.apk")
            val c: OkHttpClient = Net.client.newBuilder().readTimeout(20, TimeUnit.SECONDS).build()
            c.newCall(Request.Builder().url(url).header("User-Agent", "gatito-stream-tool").build()).execute().use { r ->
                if (r.code != 200) return@use null
                val body = r.body ?: return@use null
                val total = body.contentLength()
                var got = 0L
                var last = -1
                f.outputStream().use { out ->
                    body.byteStream().use { inp ->
                        val buf = ByteArray(32768)
                        while (true) {
                            val n = inp.read(buf)
                            if (n <= 0) break
                            out.write(buf, 0, n)
                            got += n
                            if (total > 0) {
                                val p = minOf(99L, got * 100 / total).toInt()
                                if (p != last) {
                                    last = p
                                    onPct(p)
                                }
                            }
                        }
                    }
                }
                if (total <= 0 || got == total) f else null
            }
        } catch (e: Exception) {
            null
        }
    }

    fun install(ctx: Context, f: File): Boolean = try {
        val u = FileProvider.getUriForFile(ctx, ctx.packageName + ".fileprovider", f)
        val i = Intent(Intent.ACTION_VIEW).setDataAndType(u, "application/vnd.android.package-archive")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        ctx.startActivity(i)
        true
    } catch (e: Exception) {
        false
    }
}
