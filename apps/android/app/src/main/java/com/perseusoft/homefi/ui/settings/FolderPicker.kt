package com.perseusoft.homefi.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.CreateNewFolder
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.SdCard
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.Storage
import java.io.File

/**
 * Picks a download folder on any mounted volume (internal storage, SD card).
 * The Rust core writes with plain file paths, so this browses the file system
 * instead of the system document picker, which some players (A&K) don't ship.
 */
@Composable
fun FolderPicker(initial: String, onPick: (File) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val volumes = remember { Storage.volumes(context) }
    var dir by remember {
        mutableStateOf(File(initial).takeIf { d -> initial.isNotBlank() && d.isDirectory })
    }
    var tick by remember { mutableIntStateOf(0) }
    var naming by remember { mutableStateOf(false) }
    var name by remember { mutableStateOf("") }

    val current = dir
    val volume = current?.let { d -> volumes.firstOrNull { d.absolutePath.startsWith(it.root.absolutePath) } }
    val children = remember(current, tick) {
        current?.listFiles()
            ?.filter { it.isDirectory && !it.name.startsWith(".") }
            ?.sortedBy { it.name.lowercase() }
            .orEmpty()
    }
    val writable = remember(current, tick) { current != null && Storage.isWritable(current) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Column {
                Text(stringResource(R.string.folder_pick_title))
                if (current != null) {
                    Text(
                        current.absolutePath,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.StartEllipsis,
                    )
                }
            }
        },
        text = {
            Column {
                if (naming) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it.replace("/", "") },
                        label = { Text(stringResource(R.string.folder_new_name)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (current != null && !writable) {
                    Text(
                        stringResource(R.string.folder_readonly),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(vertical = 4.dp),
                    )
                }
                LazyColumn(Modifier.heightIn(max = 360.dp)) {
                    if (current == null) {
                        items(volumes) { v ->
                            PickerRow(
                                v.label,
                                if (v.removable) Icons.Rounded.SdCard else Icons.Rounded.PhoneAndroid,
                            ) { dir = v.root }
                        }
                    } else {
                        item {
                            // at a volume's root, "up" goes back to the list of volumes
                            PickerRow(stringResource(R.string.folder_up), Icons.Rounded.ArrowUpward) {
                                dir = if (current == volume?.root || volume == null) null else current.parentFile
                            }
                        }
                        items(children, key = { it.absolutePath }) { child ->
                            PickerRow(child.name, Icons.Rounded.Folder) { dir = child }
                        }
                        if (children.isEmpty()) {
                            item {
                                Text(
                                    stringResource(R.string.folder_empty),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.padding(16.dp),
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            when {
                naming -> TextButton(
                    enabled = name.isNotBlank(),
                    onClick = {
                        val made = File(current, name.trim())
                        if (made.mkdirs() || made.isDirectory) dir = made
                        naming = false
                        name = ""
                        tick++
                    },
                ) { Text(stringResource(R.string.folder_create)) }
                current != null -> TextButton(enabled = writable, onClick = { onPick(current) }) {
                    Text(stringResource(R.string.folder_use))
                }
            }
        },
        dismissButton = {
            if (current != null && writable && !naming) {
                TextButton(onClick = { naming = true }) {
                    Icon(Icons.Rounded.CreateNewFolder, contentDescription = null)
                    Text(stringResource(R.string.folder_new), Modifier.padding(start = 6.dp))
                }
            }
            TextButton(onClick = { if (naming) naming = false else onDismiss() }) {
                Text(stringResource(R.string.cancel))
            }
        },
    )
}

@Composable
private fun PickerRow(text: String, icon: ImageVector, onClick: () -> Unit) {
    ListItem(
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        leadingContent = { Icon(icon, contentDescription = null) },
        headlineContent = { Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        modifier = Modifier.clickable(onClick = onClick),
    )
}
