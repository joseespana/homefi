package com.perseusoft.homefi.data

import org.junit.Assert.assertEquals
import org.junit.Test

class DeviceFormTest {
    @Test
    fun autoFollowsTheDevice() {
        assertEquals(DeviceForm.PHONE, DeviceForm.of(tvMode = false, leanback = false, choice = UiForm.AUTO))
        assertEquals(DeviceForm.TV, DeviceForm.of(tvMode = true, leanback = false, choice = UiForm.AUTO))
        assertEquals(DeviceForm.TV, DeviceForm.of(tvMode = false, leanback = true, choice = UiForm.AUTO))
        assertEquals(DeviceForm.TV, DeviceForm.of(tvMode = true, leanback = true, choice = UiForm.AUTO))
    }

    @Test
    fun theUsersChoiceWinsOverTheDevice() {
        assertEquals(DeviceForm.PHONE, DeviceForm.of(tvMode = true, leanback = true, choice = UiForm.PHONE))
        assertEquals(DeviceForm.TV, DeviceForm.of(tvMode = false, leanback = false, choice = UiForm.TV))
    }

    @Test
    fun `a phone never stays in the TV activity and a TV never in the touch one`() {
        assert(!DeviceForm.PHONE.mustHandOver(activityIsTv = false))
        assert(DeviceForm.PHONE.mustHandOver(activityIsTv = true))
        assert(DeviceForm.TV.mustHandOver(activityIsTv = false))
        assert(!DeviceForm.TV.mustHandOver(activityIsTv = true))
    }
}
