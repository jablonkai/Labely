package org.labely.app.ui.jobs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.labely.app.AppState
import org.labely.app.R
import org.labely.app.editor.Sheet
import org.labely.app.engine.JobInfo
import org.labely.app.ui.Banner
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Name, icon and colours of a job state: queued, printing, done, failed or cancelled. */
@Composable
fun jobState(state: String): Triple<String, Ic, Pair<Color, Color>> = when (state) {
    "queued" -> Triple(stringResource(R.string.job_queued), Ic.Clock, Palette.text2 to Palette.bg)
    "printing" -> Triple(stringResource(R.string.job_printing), Ic.Printer, Palette.primary to Palette.primaryTint)
    "failed" -> Triple(stringResource(R.string.job_failed), Ic.Warn, Palette.red to Palette.amberTint)
    "cancelled" -> Triple(stringResource(R.string.job_cancelled), Ic.Pause, Palette.amber to Palette.amberTint)
    else -> Triple(stringResource(R.string.job_done), Ic.Check, Palette.green to Palette.greenTint)
}

/**
 * The print queue: every job of this session, newest first, with its state and progress. A
 * queued or printing job can be cancelled; the queue can be paused and resumed, and the
 * finished jobs cleared from the list.
 */
@Composable
fun JobsSheet(app: AppState, onDismiss: () -> Unit) {
    LaunchedEffect(Unit) { app.refreshJobs() }
    val jobs = app.jobs.reversed()
    Sheet(stringResource(R.string.jobs_title), onDismiss) {
        if (app.queuePaused) Banner(stringResource(R.string.jobs_paused), ok = false)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            SecondaryButton(
                stringResource(if (app.queuePaused) R.string.jobs_resume else R.string.jobs_pause),
                { app.pauseQueue(!app.queuePaused) },
                Modifier.weight(1f),
                if (app.queuePaused) Ic.Play else Ic.Pause,
            )
            SecondaryButton(stringResource(R.string.jobs_clear), { app.clearJobs() }, Modifier.weight(1f), Ic.Trash, enabled = jobs.any(JobInfo::finished))
        }
        Group {
            if (jobs.isEmpty()) ListRow(stringResource(R.string.jobs_empty))
            jobs.forEachIndexed { i, job ->
                if (i > 0) RowDivider()
                val (name, icon, colours) = jobState(job.state)
                ListRow(
                    job.docName.ifEmpty { job.id },
                    listOfNotNull(name, stringResource(R.string.job_pages, job.pagesDone, job.pagesTotal), job.error).joinToString(" · "),
                    leading = { IconTile(icon, colours.first, colours.second) },
                ) {
                    if (!job.finished) {
                        Text(
                            stringResource(R.string.cancel),
                            style = Type.bodyStrong.copy(color = Palette.red),
                            modifier = Modifier.clickable { app.cancelJob(job.id) },
                        )
                    }
                }
            }
        }
    }
}
