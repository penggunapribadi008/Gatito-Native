package com.streamdeckhp.app

import android.content.res.Configuration
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.*
import androidx.compose.foundation.*
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import coil.ImageLoader
import coil.compose.AsyncImage
import coil.decode.GifDecoder
import coil.decode.ImageDecoderDecoder
import kotlinx.coroutines.delay

private val BG = Color(0xFF0E0F12)
private val ACCENT = Color(0xFF4F6DF5)
private val CARD = Color(0xFF1D2028)
private val SHEET = Color(0xFF15171D)
private val MUTED = Color(0xFF9AA0AE)
private val PILL = Color(0xFF191B22)
private val TEXT = Color(0xFFF2F3F7)

private fun parseColor(s: String): Color = try {
    Color(android.graphics.Color.parseColor(s))
} catch (e: Exception) {
    ACCENT
}

@Composable
fun AppRoot(vm: DeckVM) {
    val ctx = LocalContext.current
    val loader = remember {
        ImageLoader.Builder(ctx).components {
            if (Build.VERSION.SDK_INT >= 28) add(ImageDecoderDecoder.Factory()) else add(GifDecoder.Factory())
        }.build()
    }
    BackHandler(true) { if (!vm.back()) vm.finishApp?.invoke() }
    val land = LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE
    val ins = WindowInsets.displayCutout.asPaddingValues()
    val ld = LocalLayoutDirection.current

    ProvideTextStyle(TextStyle(fontSize = 16.sp, color = TEXT)) {
        Box(
            Modifier
                .fillMaxSize()
                .drawBehind {
                    drawRect(BG)
                    drawRect(Brush.radialGradient(listOf(Color(0xFF1C2030), Color.Transparent), center = Offset(size.width * 0.2f, 0f), radius = size.maxDimension * 0.6f))
                },
        ) {
            // body: padding max(8px, safe-area) max(10px, ...) max(10px, ...) max(10px, ...)
            Box(
                Modifier.fillMaxSize().padding(
                    start = maxOf(10.dp, ins.calculateStartPadding(ld)),
                    end = maxOf(10.dp, ins.calculateEndPadding(ld)),
                    top = maxOf(8.dp, ins.calculateTopPadding()),
                    bottom = maxOf(if (land) 18.dp else 10.dp, ins.calculateBottomPadding()),
                ),
            ) {
                when (vm.mode) {
                    Mode.DECK -> DeckView(vm, loader)
                    Mode.PAIR -> PairView(vm)
                    Mode.CONNECTING -> CenterText("Menghubungkan…", "Mencari PC kamu.")
                    Mode.PAIRING -> CenterText("Menyambungkan…", null)
                    Mode.FAIL -> CenterText("Belum tersambung", vm.failMsg) {
                        PrimaryButton("Coba lagi") { vm.retry() }
                    }
                }
            }
            if (vm.showProfiles) ProfilesSheet(vm)
            if (vm.showVkeys) VKeysSheet(vm, loader)
            if (vm.showSettings) SettingsSheet(vm)
            if (vm.offVisible) OffOverlay(vm)
            vm.dlg?.let { DialogHost(vm, it) }
            if (vm.scanning) ScanOverlay { vm.scanDone(it) }
        }
    }
}

// ---------------------------------------------------------------- layar tengah
@Composable
private fun CenterText(title: String, text: String?, extra: @Composable ColumnScope.() -> Unit = {}) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 320.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(title, color = TEXT, fontSize = 24.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center)
            if (text != null) {
                Spacer(Modifier.height(16.dp))
                Text(text, color = MUTED, textAlign = TextAlign.Center, fontSize = 16.sp, lineHeight = 24.sp)
            }
            extra()
        }
    }
}

@Composable
private fun PairView(vm: DeckVM) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 320.dp).verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
            androidx.compose.foundation.Image(painterResource(R.drawable.logo), null, Modifier.size(84.dp))
            Spacer(Modifier.height(14.dp))
            Text("Sambungkan ke PC", color = TEXT, fontSize = 24.sp, fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(16.dp))
            Text(
                "Di PC, klik tombol ⋮ di pojok kanan atas lalu tampilkan kode QR. Di sini tekan tombol di bawah untuk memindainya.",
                color = MUTED, textAlign = TextAlign.Center, fontSize = 16.sp, lineHeight = 24.sp,
            )
            PrimaryButton("Scan QR", Modifier.padding(top = 4.dp)) { vm.scan() }
            Spacer(Modifier.height(16.dp))
            Text("Tidak bisa scan? Isi manual", color = Color(0xFF8FA2FF), fontSize = 16.sp, modifier = Modifier.clickable { vm.manual() }.padding(6.dp))
        }
    }
}

@Composable
private fun PrimaryButton(text: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Box(
        modifier.padding(top = 12.dp).clip(RoundedCornerShape(12.dp)).background(ACCENT).clickable(onClick = onClick).padding(horizontal = 22.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center,
    ) { Text(text, color = Color.White, fontWeight = FontWeight.Bold) }
}

@Composable
private fun WideButton(text: String, current: Boolean = false, onClick: () -> Unit) {
    Box(
        Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(if (current) ACCENT else BG).clickable(onClick = onClick).padding(14.dp),
        contentAlignment = Alignment.Center,
    ) { Text(text, color = if (current) Color.White else TEXT, fontWeight = if (current) FontWeight.Bold else FontWeight.Normal) }
}

// ---------------------------------------------------------------- deck
@Composable
private fun DeckView(vm: DeckVM, loader: ImageLoader) {
    val deck = vm.deck ?: return
    val land = LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            TopBar(vm, deck, land)
            ObsBar(vm)
            KeyGrid(vm, deck, land, loader, Modifier.weight(1f).fillMaxWidth())
            if (!land) Box(Modifier.fillMaxWidth().padding(top = 10.dp), contentAlignment = Alignment.Center) { Pager(vm) }
        }
        if (land) Box(Modifier.align(Alignment.BottomEnd).padding(end = 6.dp, bottom = 4.dp)) { Pager(vm) }
    }
}

@Composable
private fun TopBar(vm: DeckVM, deck: Deck, land: Boolean) {
    val h = if (land) 34.dp else 40.dp
    val maxW = (LocalConfiguration.current.screenWidthDp * 0.42f).dp
    Row(Modifier.fillMaxWidth().padding(bottom = if (land) 4.dp else 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.widthIn(max = maxW).height(h).clip(RoundedCornerShape(12.dp)).background(PILL).clickable { vm.showProfiles = true }.padding(horizontal = 12.dp),
            contentAlignment = Alignment.Center,
        ) { Text((deck.profiles.getOrNull(deck.cur) ?: "Profil") + " ▾", color = TEXT, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        Text(vm.stat, color = Color(0xFFE0A14A), fontSize = 12.sp, textAlign = TextAlign.Center, modifier = Modifier.weight(1f).padding(horizontal = 8.dp))
        Box(
            Modifier.padding(end = 6.dp).size(h).clip(RoundedCornerShape(12.dp)).background(PILL).clickable { vm.showVkeys = true },
            contentAlignment = Alignment.Center,
        ) { KeyboardIcon() }
        Box(Modifier.size(h).clip(RoundedCornerShape(12.dp)).clickable { vm.showSettings = true }, contentAlignment = Alignment.Center) {
            Text("⋮", color = TEXT, fontSize = 24.sp)
        }
    }
}

@Composable
private fun KeyboardIcon() {
    androidx.compose.foundation.Canvas(Modifier.size(22.dp)) {
        val s = size.width / 24f
        val sw = 2f * s
        drawRoundRect(TEXT, Offset(2 * s, 6 * s), Size(20 * s, 12 * s), CornerRadius(2.5f * s), style = Stroke(sw))
        for (x in listOf(6f, 10f, 14f, 18f)) drawCircle(TEXT, 1f * s, Offset(x * s, 10 * s))
        drawLine(TEXT, Offset(7 * s, 14 * s), Offset(17 * s, 14 * s), sw)
    }
}

@Composable
private fun Pager(vm: DeckVM) {
    Row(Modifier.clip(RoundedCornerShape(14.dp)).background(PILL).padding(horizontal = 6.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).clickable { vm.prevPage() }, contentAlignment = Alignment.Center) { Text("‹", color = TEXT, fontSize = 24.sp) }
        Text("${vm.page + 1}", color = TEXT, fontSize = 18.sp, textAlign = TextAlign.Center, modifier = Modifier.widthIn(min = 22.dp))
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).clickable { vm.nextPage() }, contentAlignment = Alignment.Center) { Text("›", color = TEXT, fontSize = 24.sp) }
    }
}

private fun hms(ms: Long): String {
    val s = ms / 1000
    return "%02d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60)
}

/** Indikator OBS di HP: LIVE dan REC, redup kalau belum jalan */
@Composable
private fun ObsBar(vm: DeckVM) {
    val o = vm.obs ?: return
    if (o.link == "off") return
    val now by produceState(System.currentTimeMillis()) {
        while (true) {
            delay(500)
            value = System.currentTimeMillis()
        }
    }
    Row(Modifier.fillMaxWidth().padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
        if (o.link != "ok") {
            Pill(if (o.link == "auth") "Password salah" else "OBS terputus", PILL, Color(0xFFE0A14A), Color(0xFF555B6A), false, 1f)
        } else {
            val dt = now - o.at
            val live = if (o.live) "LIVE" + (if (o.recon) " putus " else " ") + hms(o.liveMs + dt) else "LIVE"
            val rec = if (o.rec) (if (o.paused) "REC jeda " + hms(o.recMs) else "REC " + hms(o.recMs + dt)) else "REC"
            Pill(
                live,
                if (o.live) (if (o.recon) Color(0xFF3B3220) else Color(0xFF3A1D22)) else PILL,
                if (o.live) (if (o.recon) Color(0xFFF1C15A) else Color(0xFFFF8A80)) else MUTED,
                if (o.live) (if (o.recon) Color(0xFFF1C15A) else Color(0xFFFF4D4D)) else Color(0xFF555B6A),
                o.live && !o.recon, if (o.live) 1f else 0.55f,
            )
            Pill(
                rec,
                if (o.rec) (if (o.paused) Color(0xFF3B3220) else Color(0xFF3A1D22)) else PILL,
                if (o.rec) (if (o.paused) Color(0xFFF1C15A) else Color(0xFFFF8A80)) else MUTED,
                if (o.rec) (if (o.paused) Color(0xFFF1C15A) else Color(0xFFFF4D4D)) else Color(0xFF555B6A),
                o.rec && !o.paused, if (o.rec) 1f else 0.55f,
            )
        }
    }
}

@Composable
private fun Pill(text: String, bg: Color, fg: Color, dot: Color, blink: Boolean, alpha: Float) {
    val t = rememberInfiniteTransition(label = "blink")
    val a by t.animateFloat(1f, 0.3f, infiniteRepeatable(tween(600), RepeatMode.Reverse), label = "a")
    Row(
        Modifier.alpha(alpha).clip(CircleShape).background(bg).padding(horizontal = 12.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        Box(Modifier.size(9.dp).alpha(if (blink) a else 1f).clip(CircleShape).background(dot))
        Text(text, color = fg, fontSize = 13.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun KeyGrid(vm: DeckVM, deck: Deck, land: Boolean, loader: ImageLoader, modifier: Modifier) {
    BoxWithConstraints(modifier, contentAlignment = Alignment.Center) {
        val c = deck.cols
        val r = deck.rows
        val a = if (land) r else c
        val b = if (land) c else r
        val gap = 8.dp
        val fit: Dp = minOf((maxWidth - gap * (b - 1)) / b, (maxHeight - gap * (a - 1)) / a)
        val ks = maxOf(44.dp, minOf(vm.ks.dp, fit - 4.dp))
        val keys = deck.pages.getOrNull(vm.page) ?: emptyMap()
        Column(verticalArrangement = Arrangement.spacedBy(gap), horizontalAlignment = Alignment.CenterHorizontally) {
            for (ai in 0 until a) {
                Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                    for (bi in 0 until b) {
                        val i = if (land) ai * c + bi else bi * c + ai
                        KeyView(keys[i], ks, vm.host, loader) { keys[i]?.let { k -> vm.press(i, k) } }
                    }
                }
            }
        }
    }
}

@Composable
private fun KeyView(k: KeyDef?, ks: Dp, host: String, loader: ImageLoader, onTap: () -> Unit) {
    val shape = RoundedCornerShape(ks * 0.2f)
    if (k == null) {
        Box(Modifier.size(ks).clip(shape).background(Color.Black).border(2.dp, Color(0xFF2B2E38), shape))
        return
    }
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val sc by animateFloatAsState(if (pressed) 0.93f else 1f, label = "press")
    val crt = k.img != null && k.crt
    val lblShadow = Shadow(Color(0x73000000), Offset(0f, 1.5f), 2f)
    Box(
        Modifier
            .size(ks)
            .scale(sc)
            .clip(shape)
            .background(parseColor(k.color))
            .clickable(interactionSource = src, indication = null, onClick = onTap)
            .drawWithContent {
                drawContent()
                // sorot atas dan bayangan bawah, seperti tombol versi web
                drawRect(Color(0x40FFFFFF), Offset.Zero, Size(size.width, 2.dp.toPx()))
                drawRect(Color(0x40000000), Offset(0f, size.height - 5.dp.toPx()), Size(size.width, 5.dp.toPx()))
                if (pressed) drawRect(Color(0x40FFFFFF)) // filter: brightness(1.25)
                if (crt) {
                    val line = 1.dp.toPx()
                    var y = 0f
                    while (y < size.height) {
                        drawRect(Color(0x59000000), Offset(0f, y), Size(size.width, line))
                        y += 3.dp.toPx()
                    }
                    drawRect(
                        Brush.radialGradient(
                            colorStops = arrayOf(0.55f to Color.Transparent, 1f to Color(0x8C000000)),
                            center = center, radius = size.maxDimension * 0.7f,
                        ),
                    )
                }
            },
        contentAlignment = if (k.img != null) Alignment.BottomCenter else Alignment.Center,
    ) {
        if (k.img != null) {
            AsyncImage(
                model = if (k.img.startsWith("/")) "http://$host${k.img}" else k.img,
                contentDescription = null, imageLoader = loader, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize(),
                colorFilter = if (crt) crtFilter else null,
            )
        }
        if (k.label.isNotEmpty()) {
            if (k.img != null) {
                Text(
                    k.label, color = Color.White, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, lineHeight = 1.15.em,
                    fontSize = maxOf(9f, ks.value * 0.13f).sp,
                    modifier = Modifier.fillMaxWidth().background(Color(0x8C000000)).padding(3.dp),
                )
            } else {
                Text(
                    k.label, color = Color.White, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, lineHeight = 1.15.em,
                    fontSize = maxOf(10f, ks.value * 0.17f).sp, modifier = Modifier.padding(4.dp), style = TextStyle(shadow = lblShadow),
                )
            }
        }
    }
}

/** filter: contrast(1.2) saturate(1.35) untuk tombol bergambar dengan filter CRT */
private val crtFilter: ColorFilter = run {
    val c = 1.2f
    val t = (-0.5f * c + 0.5f) * 255f
    val contrast = floatArrayOf(c, 0f, 0f, 0f, t, 0f, c, 0f, 0f, t, 0f, 0f, c, 0f, t, 0f, 0f, 0f, 1f, 0f)
    val s = 1.35f
    val r = 0.2126f * (1 - s)
    val g = 0.7152f * (1 - s)
    val b = 0.0722f * (1 - s)
    val sat = floatArrayOf(r + s, g, b, 0f, 0f, r, g + s, b, 0f, 0f, r, g, b + s, 0f, 0f, 0f, 0f, 0f, 1f, 0f)
    // hasil = sat * contrast (contrast dulu, lalu saturasi)
    val m = FloatArray(20)
    for (row in 0 until 4) for (col in 0 until 5) {
        var v = 0f
        for (k in 0 until 4) v += sat[row * 5 + k] * contrast[k * 5 + col]
        if (col == 4) v += sat[row * 5 + 4]
        m[row * 5 + col] = v
    }
    ColorFilter.colorMatrix(ColorMatrix(m))
}

// ---------------------------------------------------------------- lembar penuh
@Composable
private fun SheetBox(content: @Composable ColumnScope.() -> Unit) {
    val top = maxOf(26.dp, WindowInsets.displayCutout.asPaddingValues().calculateTopPadding())
    Column(
        Modifier.fillMaxSize().background(SHEET).clickable(indication = null, interactionSource = remember { MutableInteractionSource() }) {}
            .verticalScroll(rememberScrollState()).padding(start = 18.dp, end = 18.dp, top = top, bottom = 18.dp),
        content = content,
    )
}

@Composable
private fun SheetTitle(title: String, sub: String) {
    Text(title, color = TEXT, fontSize = 26.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 8.dp, bottom = 2.dp))
    Text(sub, color = MUTED, modifier = Modifier.padding(bottom = 18.dp))
}

@Composable
private fun CardBox(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().padding(bottom = 14.dp).clip(RoundedCornerShape(16.dp)).background(CARD).padding(16.dp), content = content)
}

@Composable
private fun SettingsSheet(vm: DeckVM) {
    SheetBox {
        SheetTitle("Pengaturan", "Gatito: Stream Tool v${vm.version}")
        CardBox {
            Text("Ukuran tombol", color = TEXT, fontWeight = FontWeight.Bold)
            Slider(value = vm.ks.toFloat(), onValueChange = { vm.setKs(it.toInt()) }, valueRange = 56f..130f, modifier = Modifier.padding(top = 4.dp))
        }
        CardBox {
            Text("Layar HP", color = TEXT, fontWeight = FontWeight.Bold)
            Text("Selama aplikasi dibuka, layar tetap menyala.", color = MUTED, fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp))
            SleepPicker(vm)
        }
        CardBox {
            Text("Update aplikasi", color = TEXT, fontWeight = FontWeight.Bold)
            WideButton(vm.upBtn) { vm.upButton() }
        }
        CardBox {
            Text(vm.pcName, color = TEXT, fontWeight = FontWeight.Bold)
            Text(vm.host, color = MUTED, modifier = Modifier.padding(top = 4.dp))
            WideButton("Sambungkan ke PC lain (scan QR)") { vm.reset() }
            WideButton("Putuskan dari PC ini") { vm.reset() }
        }
    }
}

private val SLEEP = listOf(0 to "Tidak pernah mati", 1 to "Mati setelah 1 menit", 2 to "Mati setelah 2 menit", 5 to "Mati setelah 5 menit", 10 to "Mati setelah 10 menit", 30 to "Mati setelah 30 menit")

@Composable
private fun SleepPicker(vm: DeckVM) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.padding(top = 10.dp).fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(BG).border(1.dp, Color(0xFF333A4A), RoundedCornerShape(12.dp)).clickable { open = true }.padding(12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(SLEEP.firstOrNull { it.first == vm.sleepMin }?.second ?: SLEEP[0].second, color = TEXT)
            androidx.compose.foundation.Canvas(Modifier.padding(end = 2.dp).size(width = 12.dp, height = 8.dp).align(Alignment.CenterVertically)) {
                val sw = 2.dp.toPx()
                drawLine(MUTED, Offset(size.width * 1f / 12f, size.height * 1f / 8f), Offset(size.width * 0.5f, size.height * 6f / 8f), sw, androidx.compose.ui.graphics.StrokeCap.Round)
                drawLine(MUTED, Offset(size.width * 0.5f, size.height * 6f / 8f), Offset(size.width * 11f / 12f, size.height * 1f / 8f), sw, androidx.compose.ui.graphics.StrokeCap.Round)
            }
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            SLEEP.forEach { (m, label) -> DropdownMenuItem(text = { Text(label) }, onClick = { vm.setSleep(m); open = false }) }
        }
    }
}

@Composable
private fun ProfilesSheet(vm: DeckVM) {
    val deck = vm.deck
    SheetBox {
        SheetTitle("Profil", "Pilih deck yang mau dipakai.")
        deck?.let { dk -> dk.profiles.forEachIndexed { i, n -> WideButton(n, current = i == dk.cur) { vm.pickProfile(i) } } }
    }
}

@Composable
private fun VKeysSheet(vm: DeckVM, loader: ImageLoader) {
    var m by remember { mutableStateOf("F") }
    var showUsed by remember { mutableStateOf(false) }
    SheetBox {
        SheetTitle("Tombol virtual", "Klik kotak hotkey di OBS (di PC), lalu tekan tombol di sini.")
        Row(Modifier.padding(bottom = 14.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (x in listOf("F", "K")) {
                Box(
                    Modifier.height(40.dp).clip(RoundedCornerShape(12.dp)).background(if (m == x) ACCENT else PILL).clickable { m = x }.padding(horizontal = 12.dp),
                    contentAlignment = Alignment.Center,
                ) { Text(x, color = TEXT, fontSize = 14.sp) }
            }
        }
        val list = if (m == "F") (0 until 12).map { "F${13 + it}" } else (1..36).map { "K$it" }
        list.chunked(4).forEach { row ->
            Row(Modifier.fillMaxWidth().padding(bottom = 10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                row.forEach { v -> VKeyButton(v, Modifier.weight(1f)) { vm.vsend(v) } }
                repeat(4 - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
        Spacer(Modifier.height(6.dp))
        WideButton("Cek tombol yang terpakai") { showUsed = true }
        if (showUsed) {
            val us = (vm.deck?.used ?: emptyList()).sortedBy { if (it.vk.startsWith("F")) it.vk.drop(1).toIntOrNull() ?: 0 else 100 + (it.vk.drop(1).toIntOrNull() ?: 0) }
            if (us.isEmpty()) Text("Belum ada tombol virtual yang dipakai.", color = MUTED, modifier = Modifier.padding(top = 12.dp))
            us.forEach { u ->
                Row(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(u.vk, color = TEXT, fontWeight = FontWeight.Bold, modifier = Modifier.width(42.dp))
                    Box(Modifier.size(44.dp).clip(RoundedCornerShape(10.dp)).background(if (u.img.isEmpty()) parseColor(u.color.ifEmpty { "#4f6df5" }) else Color.Transparent)) {
                        if (u.img.isNotEmpty()) AsyncImage(
                            model = if (u.img.startsWith("/")) "http://${vm.host}${u.img}" else u.img,
                            contentDescription = null, imageLoader = loader, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize(),
                        )
                    }
                    Column(Modifier.weight(1f)) {
                        Text(if (u.label.isEmpty()) "(tanpa nama)" else u.label, color = if (u.label.isEmpty()) MUTED else TEXT)
                        Text("${u.prof}, halaman ${u.page}", color = MUTED, fontSize = 12.sp)
                    }
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0xFF262A33)))
            }
        }
    }
}

@Composable
private fun VKeyButton(text: String, modifier: Modifier, onTap: () -> Unit) {
    var flash by remember { mutableStateOf(false) }
    LaunchedEffect(flash) {
        if (flash) {
            delay(140)
            flash = false
        }
    }
    Box(
        modifier.height(64.dp).clip(RoundedCornerShape(14.dp)).background(if (flash) ACCENT else CARD).clickable(indication = null, interactionSource = remember { MutableInteractionSource() }) {
            flash = true
            onTap()
        },
        contentAlignment = Alignment.Center,
    ) { Text(text, color = TEXT, fontSize = 18.sp, fontWeight = FontWeight.Bold) }
}

// ---------------------------------------------------------------- overlay dan dialog
@Composable
private fun OffOverlay(vm: DeckVM) {
    Box(
        Modifier.fillMaxSize().background(Color(0xB8000000)).clickable(indication = null, interactionSource = remember { MutableInteractionSource() }) {}.padding(20.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier.widthIn(max = 340.dp).clip(RoundedCornerShape(16.dp)).background(CARD).padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("PC tidak terhubung", color = TEXT, fontSize = 18.sp, fontWeight = FontWeight.Bold)
            Text(
                "Buka aplikasi Gatito: Stream Tool di PC kamu, atau putuskan perangkat ini dari PC.",
                color = MUTED, textAlign = TextAlign.Center, fontSize = 16.sp, lineHeight = 24.sp, modifier = Modifier.padding(top = 16.dp),
            )
            PrimaryButton("Coba lagi") { vm.offRetry() }
            WideButton("Putuskan dari perangkat") { vm.reset() }
        }
    }
}

@Composable
private fun DialogHost(vm: DeckVM, d: Dlg) {
    val values = remember(d) { mutableStateListOf(*Array(d.fields.size) { "" }) }
    Dialog(onDismissRequest = { if (!d.hideOk) vm.dlgDone(null) }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.padding(20.dp).widthIn(max = 340.dp).fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(CARD).padding(16.dp)) {
            Text(d.title, color = TEXT, fontSize = 18.sp, fontWeight = FontWeight.Bold)
            d.text?.let { Text(it, color = MUTED, fontSize = 16.sp, lineHeight = 24.sp, modifier = Modifier.padding(top = 8.dp)) }
            d.fields.forEachIndexed { i, f ->
                OutlinedTextField(
                    value = values[i], onValueChange = { values[i] = it }, singleLine = true,
                    placeholder = { Text(f.placeholder, color = MUTED) },
                    keyboardOptions = KeyboardOptions(keyboardType = if (f.numeric) KeyboardType.Number else if (f.uri) KeyboardType.Uri else KeyboardType.Text),
                    modifier = Modifier.fillMaxWidth().padding(top = 10.dp),
                )
            }
            if (!d.hideOk || d.cancel) {
                Row(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (d.cancel) Box(
                        Modifier.weight(1f).clip(RoundedCornerShape(12.dp)).background(BG).clickable { vm.dlgDone(null) }.padding(14.dp),
                        contentAlignment = Alignment.Center,
                    ) { Text("Batal", color = TEXT) }
                    if (!d.hideOk) Box(
                        Modifier.weight(1f).clip(RoundedCornerShape(12.dp)).background(ACCENT).clickable { vm.dlgDone(values.map { it.trim() }) }.padding(14.dp),
                        contentAlignment = Alignment.Center,
                    ) { Text(d.ok, color = Color.White, fontWeight = FontWeight.Bold) }
                }
            }
        }
    }
}
