package org.evomimic.map.tdl

import com.intellij.ide.structureView.StructureViewBuilder
import com.intellij.ide.structureView.StructureViewBuilderProvider
import com.intellij.ide.structureView.StructureViewModel
import com.intellij.ide.structureView.StructureViewTreeElement
import com.intellij.ide.structureView.TreeBasedStructureViewBuilder
import com.intellij.ide.structureView.StructureViewModelBase
import com.intellij.ide.projectView.PresentationData
import com.intellij.ide.util.treeView.smartTree.SortableTreeElement
import com.intellij.ide.util.treeView.smartTree.TreeElement
import com.intellij.ide.util.treeView.smartTree.Sorter
import com.intellij.lang.PsiStructureViewFactory
import com.intellij.navigation.ItemPresentation
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.fileEditor.OpenFileDescriptor
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.PsiManager
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Native Structure View bridge for TDL.
 *
 * RustRover does not project generic LSP document symbols into the Structure
 * tool window. This adapter asks map-schema for its source-only outline and
 * turns the returned locations into navigable IntelliJ tree elements.
 */
class TdlStructureViewFactory : PsiStructureViewFactory {
    override fun getStructureViewBuilder(psiFile: PsiFile): StructureViewBuilder? {
        if (psiFile.fileType != TdlFileType) return null
        return object : TreeBasedStructureViewBuilder() {
            override fun createStructureViewModel(editor: Editor?): StructureViewModel =
                TdlStructureViewModel(editor, psiFile)
        }
    }
}

/** Selects the Structure bridge by file type when TDL has no native PSI parser. */
class TdlStructureViewBuilderProvider : StructureViewBuilderProvider {
    override fun getStructureViewBuilder(
        fileType: com.intellij.openapi.fileTypes.FileType,
        file: com.intellij.openapi.vfs.VirtualFile,
        project: com.intellij.openapi.project.Project,
    ): StructureViewBuilder? {
        if (fileType != TdlFileType) return null
        val psiFile = PsiManager.getInstance(project).findFile(file) ?: return null
        return TdlStructureViewFactory().getStructureViewBuilder(psiFile)
    }
}

private class TdlStructureViewModel(editor: Editor?, psiFile: PsiFile) :
    StructureViewModelBase(psiFile, editor, TdlStructureElement.root(psiFile)) {

    override fun getSuitableClasses(): Array<Class<out PsiElement>> = arrayOf(PsiFile::class.java)

    override fun getSorters(): Array<Sorter> = arrayOf(Sorter.ALPHA_SORTER)
}

private data class TdlOutlineSymbol(
    val line: Int,
    val column: Int,
    val kind: String,
    val key: String,
)

private sealed interface TdlStructureNode

private data object TdlFileRoot : TdlStructureNode

private data class TdlSchemaNode(
    val schema: TdlOutlineSymbol,
    val declarations: List<TdlOutlineSymbol>,
) : TdlStructureNode

private data class TdlDeclarationGroup(
    val name: String,
    val declarations: List<TdlOutlineSymbol>,
) : TdlStructureNode

private data class TdlDeclarationNode(val declaration: TdlOutlineSymbol) : TdlStructureNode

private class TdlStructureElement private constructor(
    private val file: PsiFile,
    private val node: TdlStructureNode,
) : StructureViewTreeElement, SortableTreeElement {

    companion object {
        fun root(file: PsiFile) = TdlStructureElement(file, TdlFileRoot)
    }

    override fun getValue(): Any = when (node) {
        TdlFileRoot -> file
        is TdlSchemaNode -> node.schema
        is TdlDeclarationGroup -> node
        is TdlDeclarationNode -> node.declaration
    }

    override fun getChildren(): Array<TreeElement> = when (node) {
        TdlFileRoot -> rootChildren(file).map { TdlStructureElement(file, it) }.toTypedArray()
        is TdlSchemaNode -> declarationGroups(node.declarations)
            .map { TdlStructureElement(file, it) }
            .toTypedArray()
        is TdlDeclarationGroup -> node.declarations
            .map { TdlStructureElement(file, TdlDeclarationNode(it)) }
            .toTypedArray()
        is TdlDeclarationNode -> emptyArray()
    }

    override fun getPresentation(): ItemPresentation = when (node) {
        TdlFileRoot -> PresentationData(file.name, null, null, null)
        is TdlSchemaNode -> PresentationData(node.schema.key, "schema", null, null)
        is TdlDeclarationGroup -> PresentationData(
            node.name,
            "${node.declarations.size} declarations",
            null,
            null,
        )
        is TdlDeclarationNode -> PresentationData(
            node.declaration.key,
            node.declaration.kind,
            null,
            null,
        )
    }

    override fun getAlphaSortKey(): String = when (node) {
        TdlFileRoot -> file.name
        is TdlSchemaNode -> node.schema.key
        is TdlDeclarationGroup -> node.name
        is TdlDeclarationNode -> node.declaration.key
    }

    override fun navigate(requestFocus: Boolean) {
        val target = when (node) {
            is TdlSchemaNode -> node.schema
            is TdlDeclarationNode -> node.declaration
            else -> return
        }
        OpenFileDescriptor(file.project, file.virtualFile, target.line, target.column).navigate(requestFocus)
    }

    override fun canNavigate(): Boolean = node is TdlSchemaNode || node is TdlDeclarationNode

    override fun canNavigateToSource(): Boolean = canNavigate()
}

private fun rootChildren(file: PsiFile): List<TdlStructureNode> {
    val declarations = sourceOutline(file)
    val schema = declarations.firstOrNull { it.kind == "schema" }
    val nonSchemaDeclarations = declarations.filter { it.kind != "schema" }
    return if (schema != null) {
        listOf(TdlSchemaNode(schema, nonSchemaDeclarations))
    } else {
        declarationGroups(nonSchemaDeclarations)
    }
}

private fun declarationGroups(declarations: List<TdlOutlineSymbol>): List<TdlDeclarationGroup> {
    val grouped = declarations.groupBy(::groupName)
    return GROUP_ORDER.mapNotNull { name ->
        grouped[name]?.let { TdlDeclarationGroup(name, it) }
    } + grouped
        .filterKeys { it !in GROUP_ORDER }
        .toSortedMap()
        .map { (name, members) -> TdlDeclarationGroup(name, members) }
}

private fun groupName(declaration: TdlOutlineSymbol): String = when (declaration.kind) {
    "holon" -> "Holons"
    "property" -> "Properties"
    "value" -> "Values"
    "relationship", "inverseRelationship" -> "Relationships"
    "instance" -> "Instances"
    "enum", "variant" -> "Enumerations"
    else -> declaration.kind.replaceFirstChar(Char::titlecase)
}

private val GROUP_ORDER = listOf("Holons", "Properties", "Relationships", "Values", "Instances", "Enumerations")

private fun sourceOutline(file: PsiFile): List<TdlOutlineSymbol> {
    val virtualFile = file.virtualFile ?: return emptyList()
    return try {
        val process = ProcessBuilder(
            TdlExecutable.resolve(file.project),
            "editor-outline",
            "--source-name",
            virtualFile.url,
        )
            .directory(file.project.basePath?.let(::File))
            .redirectErrorStream(true)
            .start()
        process.outputStream.bufferedWriter().use { it.write(file.text) }
        if (!process.waitFor(2, TimeUnit.SECONDS) || process.exitValue() != 0) return emptyList()
        process.inputStream.bufferedReader().readLines().mapNotNull(::parseOutlineLine)
    } catch (_: Exception) {
        emptyList()
    }
}

private fun parseOutlineLine(line: String): TdlOutlineSymbol? {
    val fields = line.split('\t', limit = 4)
    if (fields.size != 4) return null
    val lineNumber = fields[0].toIntOrNull() ?: return null
    val column = fields[1].toIntOrNull() ?: return null
    return TdlOutlineSymbol(lineNumber, column, fields[2], fields[3])
}
