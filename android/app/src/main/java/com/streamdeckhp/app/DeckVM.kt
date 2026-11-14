package com.streamdeckhp.app

import android.app.Application
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject

class DeckVM(app: Application) : AndroidViewModel(app) {
    val prefs = Prefs(app)

    var mode by mutableStateOf(Mode.CONNECTING)
    var failMsg by mutableStateOf("")
    var deck by mutableStateOf<Deck?>(null)
    var page by mutableStateOf(0)
    var obs by mutableStateOf<ObsInfo?>(null)
    var stat by mutableStateOf("")
    var offVisible by mutableStateOf(false)
    var showSettings by mutableStateOf(false)
    var showProfiles by mutableStateOf(false)
    var showVkeys by mutableStateOf(false)
    var dlg by mutableStateOf<Dlg?>(null)
    var keepOn by mutableStateOf(true)
    var ks by mutableStateOf(prefs.ks)
    var sleepMin by mutableStateOf(prefs.sleep)
    var upBtn by mutableStateOf("Cek update")
    var scanning by mutableStateOf(false)

    var launchScan: (() -> Unit)? = null
    var finishApp: (() -> Unit)? = null

    private var ws: WebSocket? = null
    private var fails = 0
    private var lastCur = -1
    private var offJob: Job? = null
    private var idleJob: Job? = null
    private var upInfo: Updater.Release? = null
    private var upBusy = false

    val version: String get() = Updater.currentVersion(getApplication())
    val host: String get() = prefs.host ?: ""
    val pcName: String get() = prefs.pc ?: "PC"

    init {
        bump()
        start()
        viewModelScope.launch {
            delay(5000)
            upCheck(true)
        }
    }

    // ---------------------------------------------------------------- dialog
    suspend fun ask(title: String, text: String? = null, fields: List<Field> = emptyList(), ok: String = "OK", cancel: Boolean = false): List<String>? {
        val d = Dlg(title, text, fields, ok, cancel)
        dlg = d
        val r = d.result.await()
        if (dlg === d) dlg = null
        return r
    }

    fun dlgDone(v: List<String>?) {
        val d = dlg ?: return
        dlg = null
        d.result.complete(v)
    }

    // ---------------------------------------------------------------- layar dan tombol kembali
    /** Tombol kembali Android: tutup lapisan teratas, lalu mundur satu halaman. Keluar hanya kalau tidak ada yang bisa ditutup. */
    fun back(): Boolean {
        if (scanning) { scanDone(null); return true }
        if (dlg != null) { dlgDone(null); return true }
        if (showVkeys) { showVkeys = false; return true }
        if (showProfiles) { showProfiles = false; return true }
        if (showSettings) { showSettings = false; return true }
        if (mode == Mode.DECK && page > 0) { page--; return true }
        return false
    }

    @Suppress("DEPRECATION")
    fun vibrate() {
        try {
            val c = getApplication<Application>()
            val v: Vibrator = if (Build.VERSION.SDK_INT >= 31) c.getSystemService(VibratorManager::class.java).defaultVibrator
            else c.getSystemService(Context.VIBRATOR_SERVICE) as Vibrator
            if (Build.VERSION.SDK_INT >= 26) v.vibrate(VibrationEffect.createOneShot(15, VibrationEffect.DEFAULT_AMPLITUDE))
            else v.vibrate(15)
        } catch (e: Exception) {
        }
    }

    // ---------------------------------------------------------------- layar tetap menyala
    /** Layar tetap menyala selama aplikasi dibuka; bisa diatur mati setelah beberapa menit tanpa disentuh */
    fun bump() {
        keepOn = true
        idleJob?.cancel()
        val m = sleepMin
        if (m > 0) idleJob = viewModelScope.launch {
            delay(m * 60000L)
            keepOn = false
        }
    }

    fun setKs(v: Int) {
        ks = v
        prefs.ks = v
    }

    fun setSleep(m: Int) {
        sleepMin = m
        prefs.sleep = m
        bump()
    }

    // ---------------------------------------------------------------- pairing
    fun start() {
        if (prefs.host != null && prefs.token != null) {
            mode = Mode.CONNECTING
            connect()
        } else {
            mode = Mode.PAIR
        }
    }

    private fun fail(msg: String) {
        failMsg = msg
        mode = Mode.FAIL
    }

    fun retry() = start()

    fun scan() {
        launchScan?.invoke()
    }

    fun scanDone(data: String?) {
        if (!scanning) return
        scanning = false
        onScan(data)
    }

    fun onScan(data: String?) {
        if (data == null) { start(); return }
        val u = try { Uri.parse(data) } catch (e: Exception) { null }
        val auth = u?.authority
        if (u == null || auth.isNullOrEmpty() || u.scheme != "http") { fail("Kode QR tidak dikenali."); return }
        val port = if (u.port > 0) u.port else 3000
        val hs = listOf(auth) + (u.getQueryParameter("h") ?: "").split(',').filter { it.isNotEmpty() }.map { "$it:$port" }
        pairWith(hs.distinct(), u.getQueryParameter("code"))
    }

    fun manual() {
        viewModelScope.launch {
            val r = ask(
                "Isi manual", "Alamat dan kode ada di jendela Sambungkan HP di PC.",
                listOf(Field("Alamat PC, contoh 192.168.1.5:3000", uri = true), Field("Kode 6 angka", numeric = true)), "Sambung", true,
            )
            if (r == null || r[0].isEmpty() || r[1].isEmpty()) return@launch
            pairWith(listOf(if (r[0].contains(':')) r[0] else r[0] + ":3000"), r[1])
        }
    }

    private fun pairWith(hosts: List<String>, code: String?) {
        viewModelScope.launch {
            mode = Mode.PAIRING
            for (h in hosts) {
                if (!Net.probe(h)) continue
                val r = Net.pair(h, "HP ${prefs.id}", code ?: "")
                if (r != null) {
                    prefs.host = h
                    prefs.token = r.first
                    prefs.pc = r.second
                    connect()
                    return@launch
                }
            }
            fail("Tidak bisa tersambung. Pastikan HP dan PC satu WiFi, lalu scan kode QR yang terbaru di PC.")
        }
    }

    // ---------------------------------------------------------------- koneksi WebSocket
    private fun armOff() {
        if (offJob == null) offJob = viewModelScope.launch {
            delay(5000)
            offVisible = true
        }
    }

    private fun disarm() {
        offJob?.cancel()
        offJob = null
        offVisible = false
    }

    fun offRetry() {
        disarm()
        val o = ws
        ws = null
        o?.cancel()
        connect()
    }

    private fun connect() {
        val h = prefs.host ?: return
        val t = prefs.token ?: return
        ws?.let { ws = null; it.cancel() }
        var opened = false
        var closed = false
        armOff()
        var w: WebSocket? = null
        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                viewModelScope.launch { opened = true; fails = 0; disarm() }
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                viewModelScope.launch { onMsg(text) }
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = lost()
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = lost()

            private fun lost() {
                viewModelScope.launch {
                    if (closed) return@launch
                    closed = true
                    val me = w
                    if (ws !== me) return@launch
                    armOff()
                    obs = null
                    if (!opened && ++fails >= 4 && Net.probe(prefs.host ?: "")) {
                        prefs.token = null
                        fails = 0
                        start()
                        return@launch
                    }
                    stat = "Menyambung…"
                    delay(1200)
                    if (ws === me) connect()
                }
            }
        }
        w = Net.client.newWebSocket(Request.Builder().url("ws://$h/ws?token=$t").build(), listener)
        ws = w
    }

    private fun onMsg(text: String) {
        try {
            val m = JSONObject(text)
            when (m.optString("t")) {
                "obs" -> obs = parseObs(m.getJSONObject("obs"))
                "state" -> {
                    val d = parseDeck(m.getJSONObject("state"))
                    deck = d
                    if (d.cur != lastCur) { lastCur = d.cur; page = 0 }
                    if (page >= d.pages.size) page = d.pages.size - 1
                    stat = ""
                    mode = Mode.DECK
                }
            }
        } catch (e: Exception) {
        }
    }

    private fun send(s: String) {
        ws?.send(s)
    }

    fun press(i: Int, k: KeyDef) {
        val d = deck ?: return
        if (k.action == "nav") {
            vibrate()
            page = if (k.to == "prev") maxOf(0, page - 1) else minOf(d.pages.size - 1, page + 1)
            return
        }
        send("""{"t":"press","p":$page,"k":$i}""")
        vibrate()
    }

    fun prevPage() { if (deck != null) page = maxOf(0, page - 1) }
    fun nextPage() { val d = deck ?: return; page = minOf(d.pages.size - 1, page + 1) }
    fun pickProfile(i: Int) { send("""{"t":"prof","i":$i}"""); showProfiles = false }
    fun vsend(vk: String) { vibrate(); send("""{"t":"vsend","vk":"$vk"}""") }

    fun reset() {
        disarm()
        showSettings = false
        val w = ws
        ws = null
        w?.cancel()
        prefs.reset()
        ks = prefs.ks
        sleepMin = prefs.sleep
        deck = null
        obs = null
        lastCur = -1
        start()
    }

    // ---------------------------------------------------------------- update aplikasi
    fun upButton() {
        if (upInfo != null) upAsk() else upCheck(false)
    }

    private fun upAsk() {
        val info = upInfo ?: return
        if (upBusy) return
        viewModelScope.launch {
            val ok = ask("Update tersedia", "Versi ${info.tag} sudah tersedia. Update sekarang? Aplikasi akan diunduh lalu dipasang.", ok = "Update", cancel = true)
            if (ok != null) upGo()
        }
    }

    private fun upGo() {
        val info = upInfo ?: return
        val apk = info.apk ?: return
        val ctx = getApplication<Application>()
        viewModelScope.launch {
            if (!Updater.canInstall(ctx)) {
                Updater.openInstallPermission(ctx)
                ask("Izin diperlukan", "Aktifkan izin \"Pasang aplikasi tidak dikenal\" untuk Gatito di pengaturan yang terbuka, lalu tekan Update lagi.")
                return@launch
            }
            upBusy = true
            val d = Dlg("Mengunduh update", "0%", emptyList(), "OK", false)
            d.hideOk = true
            dlg = d
            val f = Updater.download(ctx, apk) { p -> d.text = "$p%" }
            upBusy = false
            if (dlg === d) dlg = null
            if (f != null && Updater.install(ctx, f)) return@launch
            ask("Gagal update", "Tidak bisa mengunduh update. Cek internet lalu coba lagi.")
        }
    }

    private fun upCheck(auto: Boolean) {
        viewModelScope.launch {
            val repo = BuildConfig.UPDATE_REPO
            if (!Regex("^[\\w.-]+/[\\w.-]+$").matches(repo)) { if (!auto) upBtn = "Repo belum diatur"; return@launch }
            if (upBusy) return@launch
            if (!auto) upBtn = "Memeriksa…"
            val j = Updater.latest(repo)
            if (j == null) { upBtn = if (auto) "Cek update" else "Gagal memeriksa"; return@launch }
            val cur = version
            if (cur.isNotEmpty() && Updater.newer(j.tag, cur)) {
                val apk = j.apk
                if (apk == null || !apk.startsWith("https://github.com/")) { upBtn = "APK belum diunggah"; return@launch }
                upInfo = Updater.Release(j.tag.removePrefix("v"), apk)
                upBtn = "Update ke v${upInfo!!.tag}"
                if (auto && dlg != null) return@launch
                upAsk()
                return@launch
            }
            upInfo = null
            upBtn = "Sudah versi terbaru"
        }
    }

    override fun onCleared() {
        ws?.cancel()
        super.onCleared()
    }
}
