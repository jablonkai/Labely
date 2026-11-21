package org.labely.app.editor

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.core.content.FileProvider
import java.io.ByteArrayOutputStream
import java.io.File
import kotlin.math.max
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.labely.app.R

/** Files in and out of the editor through the system pickers and the share sheet. */

/** MIME types offered when picking a data file: delimited text and spreadsheets. */
val DATA_MIME_TYPES = arrayOf(
    "text/*",
    "application/csv",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-excel",
    "application/vnd.oasis.opendocument.spreadsheet",
)

/** Largest data file the engine reads. */
private const val MAX_DATA_BYTES = 32 * 1024 * 1024

/** Name and content of a picked document. */
suspend fun readPicked(context: Context, uri: Uri): Pair<String, ByteArray> = withContext(Dispatchers.IO) {
    val resolver = context.contentResolver
    val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { c -> if (c.moveToFirst()) c.getString(0) else null }
        ?: uri.lastPathSegment?.substringAfterLast('/')
        ?: "data"
    val bytes = resolver.openInputStream(uri)?.use { input ->
        val out = ByteArrayOutputStream()
        val buffer = ByteArray(64 * 1024)
        while (true) {
            val n = input.read(buffer)
            if (n < 0) break
            out.write(buffer, 0, n)
            if (out.size() > MAX_DATA_BYTES) error(context.getString(R.string.file_too_large))
        }
        out.toByteArray()
    } ?: error("cannot read $uri")
    name to bytes
}

/** A picked picture, scaled down and encoded for `element.add_image`. */
class PickedImage(val bytes: ByteArray, val mime: String, val width: Int, val height: Int)

/**
 * Decode a picked picture upright, scale it so the longer side is at most [MAX_IMAGE_PX], and
 * encode it as PNG when it has transparency, else as JPEG (a photo stays small in the document).
 */
suspend fun loadImage(context: Context, uri: Uri): PickedImage = withContext(Dispatchers.IO) {
    val resolver = context.contentResolver
    val unreadable = context.getString(R.string.image_unreadable)
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, bounds) }
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) error(unreadable)
    var sample = 1
    while (max(bounds.outWidth, bounds.outHeight) / (sample * 2) >= MAX_IMAGE_PX) sample *= 2
    val decoded = resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample }) }
        ?: error(unreadable)
    val degrees = runCatching {
        resolver.openInputStream(uri)?.use {
            when (ExifInterface(it).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)) {
                ExifInterface.ORIENTATION_ROTATE_90 -> 90f
                ExifInterface.ORIENTATION_ROTATE_180 -> 180f
                ExifInterface.ORIENTATION_ROTATE_270 -> 270f
                else -> 0f
            }
        }
    }.getOrNull() ?: 0f
    val (w, h) = scaledSize(decoded.width, decoded.height)
    val matrix = Matrix().apply {
        postScale(w.toFloat() / decoded.width, h.toFloat() / decoded.height)
        postRotate(degrees)
    }
    val bitmap = Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, matrix, true)
    val alpha = bitmap.hasAlpha()
    val out = ByteArrayOutputStream()
    bitmap.compress(if (alpha) Bitmap.CompressFormat.PNG else Bitmap.CompressFormat.JPEG, 90, out)
    PickedImage(out.toByteArray(), if (alpha) "image/png" else "image/jpeg", bitmap.width, bitmap.height)
}

/** Hand exported [files] (inside the app's `exports/` folder) to the system share sheet. */
fun shareFiles(context: Context, files: List<File>, mime: String) {
    if (files.isEmpty()) return
    val uris = files.map { FileProvider.getUriForFile(context, "${context.packageName}.files", it) }
    val intent = if (uris.size == 1) {
        Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, uris[0])
    } else {
        Intent(Intent.ACTION_SEND_MULTIPLE).putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
    }
    intent.type = mime
    // The receiving app reads the files through the clip data's grant.
    intent.clipData = ClipData.newUri(context.contentResolver, files[0].name, uris[0]).apply {
        uris.drop(1).forEach { addItem(ClipData.Item(it)) }
    }
    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    context.startActivity(Intent.createChooser(intent, null))
}

/** Copy an exported [file] into the document the user created with the system picker. */
suspend fun saveTo(context: Context, file: File, target: Uri) = withContext(Dispatchers.IO) {
    context.contentResolver.openOutputStream(target)?.use { out -> file.inputStream().use { it.copyTo(out) } }
        ?: error("cannot write $target")
}

/** Copy exported [files] into the folder ([tree]) the user picked, one document each. */
suspend fun saveInto(context: Context, files: List<File>, tree: Uri, mime: String) = withContext(Dispatchers.IO) {
    val folder = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
    for (f in files) {
        val doc = DocumentsContract.createDocument(context.contentResolver, folder, mime, f.name) ?: error("cannot create ${f.name}")
        saveTo(context, f, doc)
    }
}
