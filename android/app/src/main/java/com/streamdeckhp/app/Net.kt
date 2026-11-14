package com.streamdeckhp.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.util.concurrent.TimeUnit

object Net {
    val client: OkHttpClient = OkHttpClient.Builder()
        .pingInterval(20, TimeUnit.SECONDS)
        .connectTimeout(10, TimeUnit.SECONDS)
        .build()

    /** Cek apakah ada server Gatito di alamat itu */
    suspend fun probe(hostPort: String, ms: Long = 800): Boolean = withContext(Dispatchers.IO) {
        try {
            val c = client.newBuilder().callTimeout(ms, TimeUnit.MILLISECONDS).build()
            c.newCall(Request.Builder().url("http://$hostPort/hello").build()).execute().use { r ->
                JSONObject(r.body?.string() ?: "{}").optString("app") == "streamdeck-hp"
            }
        } catch (e: Exception) {
            false
        }
    }

    /** Mengembalikan (token, namaPC) kalau berhasil */
    suspend fun pair(hostPort: String, name: String, code: String): Pair<String, String>? = withContext(Dispatchers.IO) {
        try {
            val body = JSONObject().put("name", name).put("code", code).toString().toRequestBody("application/json".toMediaType())
            val c = client.newBuilder().callTimeout(8, TimeUnit.SECONDS).build()
            c.newCall(Request.Builder().url("http://$hostPort/pair").post(body).build()).execute().use { r ->
                if (!r.isSuccessful) return@use null
                val j = JSONObject(r.body?.string() ?: "{}")
                val t = j.optString("token")
                if (t.isEmpty()) null else Pair(t, j.optString("pc", "PC"))
            }
        } catch (e: Exception) {
            null
        }
    }
}
