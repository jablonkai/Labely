package org.labely.app.editor

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.NumberField
import org.labely.app.ui.RowDivider
import org.labely.app.ui.Segmented
import org.labely.app.ui.lengthText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Nudge steps of the arrow buttons, in millimetres. */
val NUDGE_STEPS = listOf(0.5, 5.0)

/**
 * Everything about the selection that does not fit the inspector: its properties (one element)
 * and how it is arranged (one or several): name, nudging, rotation, flipping, aligning,
 * stacking, grouping, locking, mirror copies and the clipboard.
 */
@Composable
fun ElementSheet(ed: EditorState, onBinding: () -> Unit, onPickIcon: () -> Unit, onAddFont: () -> Unit, onDismiss: () -> Unit) {
    val selected = ed.selectedElements
    if (selected.isEmpty()) {
        LaunchedEffect(Unit) { onDismiss() }
        return
    }
    val one = ed.selected
    var tab by rememberSaveable { mutableIntStateOf(0) }
    Sheet(if (one != null) elementTitle(one) else pluralStringResource(R.plurals.selected_count, selected.size, selected.size), onDismiss) {
        if (one != null) {
            Segmented(listOf(stringResource(R.string.tab_properties), stringResource(R.string.tab_arrange)), tab, Modifier.fillMaxWidth()) { tab = it }
        }
        if (one != null && tab == 0) PropertiesContent(ed, one, onBinding, onPickIcon, onAddFont) else ArrangeContent(ed)
    }
}

@Composable
private fun ArrangeContent(ed: EditorState) {
    val app = ed.app
    val selected = ed.selectedElements
    val one = ed.selected
    val n = selected.size
    val movable = selected.any { !it.locked }

    if (one != null) {
        Heading(stringResource(R.string.element_name))
        ContentField(one.name.orEmpty(), kindName(one.type)) { ed.setRaw(one.id, JSONObject().put("name", if (it.isBlank()) JSONObject.NULL else it.trim())) }
    }

    Heading(stringResource(R.string.arrange_move))
    var step by rememberSaveable { mutableIntStateOf(0) }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Segmented(NUDGE_STEPS.map { lengthText(it) }, step, Modifier.weight(1f)) { step = it }
        val d = NUDGE_STEPS[step]
        Toggle(Ic.Left, false, movable) { ed.moveBy(-d, 0.0) }
        Toggle(Ic.Up, false, movable) { ed.moveBy(0.0, -d) }
        Toggle(Ic.Down, false, movable) { ed.moveBy(0.0, d) }
        Toggle(Ic.Right, false, movable) { ed.moveBy(d, 0.0) }
    }

    if (one != null) {
        Heading(stringResource(R.string.arrange_rotation))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            NumberField(stringResource(R.string.arrange_angle), one.rotationDeg, { ed.rotate(one.id, it) }, Modifier.weight(1f), "°")
            Toggle(Ic.Rotate, false, !one.locked) { ed.rotate(one.id, one.rotationDeg + 90) }
        }
        Group { SwitchRow(stringResource(R.string.rotate_snap), stringResource(R.string.rotate_snap_sub), app.rotateSnap) { app.rotateSnap = it } }
    }
    Actions {
        Action(Ic.FlipH, stringResource(R.string.flip_h), movable, on = one?.flipH == true) { ed.flip(horizontal = true) }
        Action(Ic.FlipV, stringResource(R.string.flip_v), movable, on = one?.flipV == true) { ed.flip(horizontal = false) }
        if (one != null) Action(Ic.Rotate, stringResource(R.string.turn_90), !one.locked) { ed.rotate(one.id, one.rotationDeg + 90) }
    }

    if (ed.redPrinter || selected.any { it.red }) {
        Heading(stringResource(R.string.ink_color))
        val red = selected.isNotEmpty() && selected.all { it.red }
        Segmented(listOf(stringResource(R.string.ink_black), stringResource(R.string.ink_red)), if (red) 1 else 0) { ed.setRed(it == 1) }
    }

    Heading(stringResource(if (n > 1) R.string.arrange_align else R.string.arrange_align_label))
    Actions {
        Action(Ic.AlignLeft, stringResource(R.string.align_left), movable) { ed.align("left") }
        Action(Ic.AlignCenter, stringResource(R.string.align_center), movable) { ed.align("center") }
        Action(Ic.AlignRight, stringResource(R.string.align_right), movable) { ed.align("right") }
    }
    Actions {
        Action(Ic.AlignTop, stringResource(R.string.align_top), movable) { ed.align("top") }
        Action(Ic.AlignMiddle, stringResource(R.string.align_middle), movable) { ed.align("middle") }
        Action(Ic.AlignBottom, stringResource(R.string.align_bottom), movable) { ed.align("bottom") }
    }
    if (n > 1) {
        Actions {
            Action(Ic.DistH, stringResource(R.string.distribute_h), n >= 3) { ed.distribute("horizontal") }
            Action(Ic.DistV, stringResource(R.string.distribute_v), n >= 3) { ed.distribute("vertical") }
        }
    }

    Heading(stringResource(R.string.arrange_order))
    Actions {
        Action(Ic.Front, stringResource(R.string.order_front)) { ed.reorder("front") }
        Action(Ic.Up, stringResource(R.string.order_forward)) { ed.reorder("forward") }
        Action(Ic.Down, stringResource(R.string.order_backward)) { ed.reorder("backward") }
        Action(Ic.Back, stringResource(R.string.order_back)) { ed.reorder("back") }
    }

    Heading(stringResource(R.string.arrange_group))
    val ids = selected.map { it.id }
    val allLocked = selected.all { it.locked }
    val allHidden = selected.all { it.hidden }
    Actions {
        Action(Ic.Group, stringResource(R.string.group), n >= 2) { ed.group() }
        Action(Ic.Ungroup, stringResource(R.string.ungroup), hasGroup(ids, selected)) { ed.ungroup() }
        Action(if (allLocked) Ic.Lock else Ic.Unlock, stringResource(if (allLocked) R.string.unlock else R.string.lock), on = allLocked) { ed.lock(ids, !allLocked) }
        Action(if (allHidden) Ic.EyeOff else Ic.Eye, stringResource(if (allHidden) R.string.show else R.string.hide), on = allHidden) { ed.setHidden(ids, !allHidden) }
    }

    if (one != null) {
        Heading(stringResource(R.string.mirror_copy))
        Text(stringResource(R.string.mirror_copy_sub), style = Type.caption)
        Actions {
            Action(Ic.FlipH, stringResource(R.string.mirror_h), one.mirrorOf == null) { ed.mirror(one.id, "horizontal") }
            Action(Ic.FlipV, stringResource(R.string.mirror_v), one.mirrorOf == null) { ed.mirror(one.id, "vertical") }
            Action(Ic.Link, stringResource(R.string.mirror_unlink), one.mirrorOf != null, on = one.mirrorOf != null) {
                ed.setRaw(one.id, JSONObject().put("mirror", JSONObject.NULL))
            }
        }
    }

    Heading(stringResource(R.string.clipboard))
    Actions {
        Action(Ic.Copy, stringResource(R.string.copy)) { ed.copy() }
        Action(Ic.Cut, stringResource(R.string.cut)) { ed.cut() }
        Action(Ic.Paste, stringResource(R.string.paste), app.clip != null || app.systemClipboard() != null) { ed.pasteAny() }
        Action(Ic.Layers, stringResource(R.string.duplicate)) { ed.duplicateSelection() }
        Action(Ic.Trash, stringResource(R.string.delete), tint = Palette.red) { ed.deleteSelection() }
    }
    RowDivider()
}
