package org.evomimic.map.tdl

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.extapi.psi.PsiFileBase
import com.intellij.lang.ASTNode
import com.intellij.lang.ParserDefinition
import com.intellij.lang.PsiBuilder
import com.intellij.lang.PsiParser
import com.intellij.lexer.Lexer
import com.intellij.openapi.project.Project
import com.intellij.psi.FileViewProvider
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IFileElementType
import com.intellij.psi.tree.IElementType
import com.intellij.psi.tree.TokenSet

/**
 * Flat PSI bridge required by RustRover's native Structure View lifecycle.
 *
 * It deliberately supplies no TDL semantics: map-schema remains the only
 * declaration/source analysis implementation.
 */
class TdlParserDefinition : ParserDefinition {
    override fun createLexer(project: Project?): Lexer = TdlLexer()

    override fun createParser(project: Project?): PsiParser = object : PsiParser {
        override fun parse(root: IElementType, builder: PsiBuilder): ASTNode {
            val marker = builder.mark()
            while (!builder.eof()) builder.advanceLexer()
            marker.done(root)
            return builder.treeBuilt
        }
    }

    override fun getFileNodeType(): IFileElementType = TdlElementTypes.FILE

    override fun getCommentTokens(): TokenSet = TokenSet.create(TdlTokenTypes.COMMENT)

    override fun getStringLiteralElements(): TokenSet = TokenSet.create(TdlTokenTypes.STRING)

    override fun createElement(node: ASTNode): PsiElement = ASTWrapperPsiElement(node)

    override fun createFile(viewProvider: FileViewProvider): PsiFile = TdlPsiFile(viewProvider)
}

private object TdlElementTypes {
    val FILE = IFileElementType(TdlLanguage)
}

private class TdlPsiFile(viewProvider: FileViewProvider) : PsiFileBase(viewProvider, TdlLanguage) {
    override fun getFileType() = TdlFileType

    override fun toString() = "TDL File"
}
