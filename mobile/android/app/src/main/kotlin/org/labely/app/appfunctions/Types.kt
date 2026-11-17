package org.labely.app.appfunctions

import androidx.appfunctions.AppFunctionSerializable

// The shapes agents see (ADR-0028). KSP reads only inline KDoc on each property.

/** A label saved in Labely, ready to print. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class SavedLabel(
    /** Id of the saved label. Pass it to "printLabel". It stays valid after the app restarts. */
    val labelId: String,
    /** Name of the label, as the app shows it in its recent labels. */
    val name: String,
    /** Label width in millimetres. */
    val widthMm: Double,
    /** Label height in millimetres. */
    val heightMm: Double,
    /**
     * Problems that would spoil the print, such as text that does not fit or content outside the
     * printable area. Empty when the label prints cleanly.
     */
    val warnings: List<String>,
)

/** A text or code on a template that "createLabelFromTemplate" can fill in. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class TemplateField(
    /** Field name, to use as "FieldValue.name". It is the element's name, else its sample text. */
    val name: String,
    /** "text" for printed text, "barcode" or "qr" for a code whose data is replaced. */
    val kind: String,
    /** The sample value the template holds now. */
    val value: String,
)

/** A label template: a starter that ships with the app, or one the user saved. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class LabelTemplate(
    /** Template id such as "starter:price-tag" or "my:Kitchen/Jar". Pass it to "createLabelFromTemplate". */
    val templateId: String,
    /** Template name in the user's language. */
    val name: String,
    /** Folder in the user's template library; empty for starters and the default folder. */
    val folder: String,
    /** Label width in millimetres. */
    val widthMm: Double,
    /** Label height in millimetres. */
    val heightMm: Double,
    /** The fields that can be filled in. */
    val fields: List<TemplateField>,
)

/** A value for one template field. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class FieldValue(
    /** Field name from "LabelTemplate.fields". */
    val name: String,
    /** New text, or the new data of a barcode or QR code. */
    val value: String,
)

/** A label printer the app knows. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class Printer(
    /** Printer id, such as "ble:AA:BB:CC:DD:EE:FF". Pass it to "getPrinterStatus" and "printLabel". */
    val printerId: String,
    /** Printer name as it advertises itself, such as "B21 Pro-1234". */
    val name: String,
    /** Whether the app is connected to the printer now. */
    val connected: Boolean,
    /** Printer model such as "NIIMBOT B21 Pro"; empty until the app has connected once. */
    val model: String,
)

/** The state of a connected printer. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class PrinterStatus(
    /** Printer id. */
    val printerId: String,
    /** Printer name. */
    val name: String,
    /** Printer model, such as "NIIMBOT B21 Pro". */
    val model: String,
    /** Battery charge from 0 to 100, or -1 when the printer does not report it. */
    val batteryPercent: Int,
    /** "loaded", "empty" or "unknown" when the printer does not report paper. */
    val paper: String,
)

/** The outcome of a print. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class PrintResult(
    /** Id of the print job in the app's print history. */
    val jobId: String,
    /** "done" when every page printed. */
    val state: String,
    /** Pages printed. */
    val pagesPrinted: Int,
    /** Pages the job had. */
    val pagesTotal: Int,
)
