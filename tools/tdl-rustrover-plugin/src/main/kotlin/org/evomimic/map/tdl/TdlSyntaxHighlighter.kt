package org.evomimic.map.tdl

import com.intellij.lexer.LexerBase
import com.intellij.lexer.Lexer
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.HighlighterColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighter
import com.intellij.openapi.fileTypes.SyntaxHighlighterBase
import com.intellij.openapi.fileTypes.SyntaxHighlighterFactory
import com.intellij.psi.TokenType
import com.intellij.psi.tree.IElementType

/** Lexical presentation for TDL. Source semantics remain owned by map-schema. */
class TdlSyntaxHighlighter : SyntaxHighlighterBase() {
    override fun getHighlightingLexer(): Lexer = TdlLexer()

    override fun getTokenHighlights(tokenType: IElementType): Array<TextAttributesKey> = when (tokenType) {
        TdlTokenTypes.KEYWORD -> pack(KEYWORD)
        TdlTokenTypes.STRING -> pack(STRING)
        TdlTokenTypes.COMMENT -> pack(COMMENT)
        TdlTokenTypes.KEY -> pack(KEY)
        TdlTokenTypes.BRACE -> pack(BRACE)
        TokenType.BAD_CHARACTER -> pack(BAD_CHARACTER)
        else -> emptyArray()
    }

    companion object {
        private val KEYWORD = TextAttributesKey.createTextAttributesKey(
            "MAP_TDL_KEYWORD", DefaultLanguageHighlighterColors.KEYWORD,
        )
        private val STRING = TextAttributesKey.createTextAttributesKey(
            "MAP_TDL_STRING", DefaultLanguageHighlighterColors.STRING,
        )
        private val COMMENT = TextAttributesKey.createTextAttributesKey(
            "MAP_TDL_COMMENT", DefaultLanguageHighlighterColors.LINE_COMMENT,
        )
        private val KEY = TextAttributesKey.createTextAttributesKey(
            "MAP_TDL_KEY", DefaultLanguageHighlighterColors.IDENTIFIER,
        )
        private val BRACE = TextAttributesKey.createTextAttributesKey(
            "MAP_TDL_BRACE", DefaultLanguageHighlighterColors.BRACES,
        )
        private val BAD_CHARACTER = TextAttributesKey.createTextAttributesKey(
            "MAP_TDL_BAD_CHARACTER", HighlighterColors.BAD_CHARACTER,
        )
    }
}

class TdlSyntaxHighlighterFactory : SyntaxHighlighterFactory() {
    override fun getSyntaxHighlighter(
        project: com.intellij.openapi.project.Project?,
        file: com.intellij.openapi.vfs.VirtualFile?,
    ): SyntaxHighlighter = TdlSyntaxHighlighter()
}

internal object TdlTokenTypes {
    val KEYWORD = TdlToken("KEYWORD")
    val STRING = TdlToken("STRING")
    val COMMENT = TdlToken("COMMENT")
    val KEY = TdlToken("KEY")
    val BRACE = TdlToken("BRACE")
}

internal class TdlToken(debugName: String) : IElementType(debugName, TdlLanguage)

internal class TdlLexer : LexerBase() {
    private var buffer: CharSequence = ""
    private var endOffset = 0
    private var tokenStart = 0
    private var tokenEnd = 0
    private var tokenType: IElementType? = null

    override fun start(buffer: CharSequence, startOffset: Int, endOffset: Int, initialState: Int) {
        this.buffer = buffer
        this.endOffset = endOffset
        tokenStart = startOffset
        tokenEnd = startOffset
        advance()
    }

    override fun getState() = 0
    override fun getTokenType(): IElementType? = tokenType
    override fun getTokenStart() = tokenStart
    override fun getTokenEnd() = tokenEnd
    override fun getBufferSequence(): CharSequence = buffer
    override fun getBufferEnd() = endOffset

    override fun advance() {
        tokenStart = tokenEnd
        if (tokenStart >= endOffset) {
            tokenType = null
            return
        }
        val first = buffer[tokenStart]
        when {
            first.isWhitespace() -> consumeWhile { it.isWhitespace() }.also { tokenType = TokenType.WHITE_SPACE }
            first == '/' && tokenStart + 1 < endOffset && buffer[tokenStart + 1] == '/' -> {
                consumeUntil { it == '\n' }.also { tokenType = TdlTokenTypes.COMMENT }
            }
            first == '"' -> consumeString()
            first in "{}[]()" -> {
                tokenEnd++
                tokenType = TdlTokenTypes.BRACE
            }
            first.isLetter() || first == '_' -> consumeIdentifier()
            first.isDigit() || first in ".-$" -> {
                consumeWhile { it.isLetterOrDigit() || it in ".-_$/" }
                tokenType = TdlTokenTypes.KEY
            }
            else -> {
                tokenEnd++
                tokenType = TokenType.BAD_CHARACTER
            }
        }
    }

    private fun consumeString() {
        tokenEnd++
        while (tokenEnd < endOffset) {
            val current = buffer[tokenEnd++]
            if (current == '"' && buffer[tokenEnd - 2] != '\\') break
        }
        tokenType = TdlTokenTypes.STRING
    }

    private fun consumeIdentifier() {
        consumeWhile { it.isLetterOrDigit() || it == '_' }
        val word = buffer.subSequence(tokenStart, tokenEnd).toString()
        tokenType = if (word in KEYWORDS) TdlTokenTypes.KEYWORD else TdlTokenTypes.KEY
    }

    private fun consumeWhile(predicate: (Char) -> Boolean) {
        while (tokenEnd < endOffset && predicate(buffer[tokenEnd])) tokenEnd++
    }

    private fun consumeUntil(predicate: (Char) -> Boolean) {
        while (tokenEnd < endOffset && !predicate(buffer[tokenEnd])) tokenEnd++
    }

    companion object {
        private val KEYWORDS = setOf(
            "abstract", "def", "deletion_semantic", "depends_on", "extends", "header", "holon",
            "instance", "inverse", "meta", "properties", "relationships", "rule_of", "schema",
            "source", "target", "type", "value",
        )
    }
}
