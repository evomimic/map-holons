package org.evomimic.map.tdl

import com.intellij.lang.Language
import com.intellij.openapi.fileTypes.LanguageFileType
import javax.swing.Icon

/** Registers `.tdl` as a MAP Type Definition Language file. */
object TdlLanguage : Language("TDL")

object TdlFileType : LanguageFileType(TdlLanguage) {

    override fun getName(): String = "TDL"

    override fun getDescription(): String = "MAP Type Definition Language"

    override fun getDefaultExtension(): String = "tdl"

    override fun getIcon(): Icon? = null
}
