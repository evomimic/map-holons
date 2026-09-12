package org.evomimic.map.tdl

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspIntegrationProvider
import com.intellij.platform.lsp.api.ProjectWideLspClientDescriptor
import java.nio.file.Files
import java.nio.file.Path

internal object TdlExecutable {
    fun resolve(project: Project): String {
        System.getProperty("map.tdl.lsp.executable")?.let { return it }

        val localExecutable = project.basePath
            ?.let { Path.of(it, "tools", "map-schema", "target", "debug", "map-schema") }
            ?.takeIf(Files::isExecutable)

        return localExecutable?.toString() ?: "map-schema"
    }
}

/**
 * Thin RustRover adapter for the editor-neutral `map-schema lsp` service.
 *
 * All TDL parsing, lowering, provenance, and source indexing remain in the Rust
 * source tool. This adapter only associates `.tdl` files and starts that service.
 */
class TdlLspIntegrationProvider : LspIntegrationProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        clientStarter: LspIntegrationProvider.LspClientStarter,
    ) {
        if (file.extension == "tdl") {
            clientStarter.ensureClientStarted(TdlLspClientDescriptor(project))
        }
    }
}

private class TdlLspClientDescriptor(project: Project) :
    ProjectWideLspClientDescriptor(project, "MAP TDL") {

    override fun isSupportedFile(file: VirtualFile): Boolean =
        file.extension == "tdl"

    override fun createCommandLine(): GeneralCommandLine =
        GeneralCommandLine(TdlExecutable.resolve(project), "lsp")
}
