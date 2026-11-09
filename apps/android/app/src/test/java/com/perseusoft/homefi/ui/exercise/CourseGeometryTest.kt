package com.perseusoft.homefi.ui.exercise

import com.perseusoft.homefi.core.SegmentKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CourseGeometryTest {
    private val model = CourseModel(
        profile = listOf(CoursePoint(0f, 0.1f), CoursePoint(300f, 0.1f), CoursePoint(360f, 0.9f), CoursePoint(480f, 0.9f), CoursePoint(600f, 0.2f)),
        parts = listOf(
            CoursePart(SegmentKind.WARM_UP, 0f, 300f),
            CoursePart(SegmentKind.SEATED_CLIMB, 300f, 60f),
            CoursePart(SegmentKind.SEATED_CLIMB, 360f, 120f),
            CoursePart(SegmentKind.SPRINT, 480f, 120f),
        ),
    )

    @Test
    fun tickStepKeepsLabelsFewOnNarrowCards() {
        assertEquals(300, tickStep(600f, 360f)) // at most 4 labels
        assertEquals(120, tickStep(600f, 700f)) // at most 9
    }

    @Test
    fun nextDifferentSkipsSplitPartsOfOneKind() {
        assertEquals(3, nextDifferent(model.parts, 1))
        assertEquals(-1, nextDifferent(model.parts, 3))
    }

    @Test
    fun layoutKeepsTheRidgeInsideTheCardAndRiderOnIt() {
        val g = layoutCourse(model, 360f, 96f)
        assertTrue(g.ys.all { it in 0f..g.floor })
        assertEquals(g.padX, g.xOfT(0f, model.totalS), 0.001f)
        assertEquals(g.w - g.padX, g.xOfT(model.totalS, model.totalS), 0.001f)
        // the highest part of the course is the highest ridge point
        val peakY = g.ridgeY(g.xOfT(420f, model.totalS))
        assertTrue(peakY < g.ridgeY(g.xOfT(100f, model.totalS)))
    }

    @Test
    fun smallCardsAreMinis() {
        val g = layoutCourse(model, 88f, 44f)
        assertTrue(g.mini)
        assertEquals(0f, g.tick, 0f)
    }

    @Test
    fun dimmedColoursLoseSaturationButKeepAlpha() {
        val c = dimmed(ZONE_COLOR.getValue(SegmentKind.SPRINT))
        assertTrue(c.red < ZONE_COLOR.getValue(SegmentKind.SPRINT).red)
    }
}
