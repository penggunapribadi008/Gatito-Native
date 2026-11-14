package com.streamdeckhp.app

import android.content.Context
import kotlin.random.Random

/** Pengaturan HP (setara localStorage "sd" di versi web) */
class Prefs(ctx: Context) {
    private val sp = ctx.getSharedPreferences("sd", Context.MODE_PRIVATE)

    var host: String?
        get() = sp.getString("host", null)
        set(v) = sp.edit().apply { if (v == null) remove("host") else putString("host", v) }.apply()
    var token: String?
        get() = sp.getString("token", null)
        set(v) = sp.edit().apply { if (v == null) remove("token") else putString("token", v) }.apply()
    var pc: String?
        get() = sp.getString("pc", null)
        set(v) = sp.edit().apply { if (v == null) remove("pc") else putString("pc", v) }.apply()
    var ks: Int
        get() = sp.getInt("ks", 96)
        set(v) = sp.edit().putInt("ks", v).apply()
    var sleep: Int
        get() = sp.getInt("sleep", 0)
        set(v) = sp.edit().putInt("sleep", v).apply()

    /** Nomor unik HP, dipakai sebagai nama perangkat ("HP 123") */
    val id: Int
        get() {
            var v = sp.getInt("id", 0)
            if (v == 0) {
                v = Random.nextInt(100, 1000)
                sp.edit().putInt("id", v).apply()
            }
            return v
        }

    /** Putuskan dari PC: semuanya dihapus kecuali nomor HP */
    fun reset() {
        val i = id
        sp.edit().clear().putInt("id", i).apply()
    }
}
