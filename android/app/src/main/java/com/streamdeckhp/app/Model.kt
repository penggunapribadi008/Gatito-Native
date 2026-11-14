package com.streamdeckhp.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CompletableDeferred
import org.json.JSONObject

enum class Mode { PAIR, CONNECTING, PAIRING, FAIL, DECK }

data class KeyDef(val label: String, val color: String, val action: String, val to: String, val img: String?, val crt: Boolean)
data class UsedVk(val vk: String, val label: String, val img: String, val color: String, val prof: String, val page: Int)
data class Deck(
    val cols: Int,
    val rows: Int,
    val pages: List<Map<Int, KeyDef>>,
    val profiles: List<String>,
    val cur: Int,
    val used: List<UsedVk>,
)

/** Status OBS dari PC. 'at' = waktu terima, supaya lamanya dihitung terus di HP */
data class ObsInfo(
    val link: String,
    val live: Boolean,
    val recon: Boolean,
    val liveMs: Long,
    val rec: Boolean,
    val paused: Boolean,
    val recMs: Long,
    val at: Long,
)

class Field(val placeholder: String, val numeric: Boolean = false, val uri: Boolean = false)

/** Dialog di dalam aplikasi (pengganti alert dan prompt) */
class Dlg(
    title: String,
    initialText: String?,
    val fields: List<Field>,
    val ok: String,
    val cancel: Boolean,
) {
    var title by mutableStateOf(title)
    var text by mutableStateOf(initialText)
    var hideOk by mutableStateOf(false)
    val result = CompletableDeferred<List<String>?>()
}

fun parseDeck(j: JSONObject): Deck {
    val pj = j.getJSONArray("pages")
    val pages = (0 until pj.length()).map { i ->
        val keys = pj.getJSONObject(i).optJSONObject("keys")
        val m = mutableMapOf<Int, KeyDef>()
        if (keys != null) {
            for (k in keys.keys()) {
                val o = keys.getJSONObject(k)
                val idx = k.toIntOrNull() ?: continue
                m[idx] = KeyDef(
                    label = o.optString("label"),
                    color = o.optString("color").ifEmpty { "#4f6df5" },
                    action = o.optString("action"),
                    to = o.optString("to"),
                    img = o.optString("img").ifEmpty { null },
                    crt = o.optString("crt") == "1",
                )
            }
        }
        m
    }
    val profs = j.optJSONArray("profiles")?.let { a -> (0 until a.length()).map { a.getString(it) } } ?: emptyList()
    val ua = j.optJSONArray("usedVk")
    val used = if (ua == null) emptyList() else (0 until ua.length()).map {
        val u = ua.getJSONObject(it)
        UsedVk(u.optString("vk"), u.optString("label"), u.optString("img"), u.optString("color"), u.optString("prof"), u.optInt("page"))
    }
    return Deck(j.optInt("cols", 5), j.optInt("rows", 3), pages, profs, j.optInt("cur", 0), used)
}

fun parseObs(j: JSONObject): ObsInfo = ObsInfo(
    link = j.optString("link", "off"),
    live = j.optBoolean("live"),
    recon = j.optBoolean("recon"),
    liveMs = j.optLong("liveMs"),
    rec = j.optBoolean("rec"),
    paused = j.optBoolean("paused"),
    recMs = j.optLong("recMs"),
    at = System.currentTimeMillis(),
)
