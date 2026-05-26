package com.ariadne.sample

import kotlin.math.sqrt

class Counter(var value: Int = 0) {
    fun increment(): Int {
        value += 1
        return value
    }
}

object Singleton {
    fun greet(name: String): String {
        return "hi $name"
    }
}

fun distance(a: Pair<Double, Double>, b: Pair<Double, Double>): Double {
    val dx = a.first - b.first
    val dy = a.second - b.second
    return sqrt(dx * dx + dy * dy)
}

annotation class Marker

@Marker
fun annotated(): Int = 7
