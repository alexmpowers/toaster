import React from "react";
import { useTranslation } from "react-i18next";
import type { Word } from "@/stores/editorStore";

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  wordIndex: number;
}

interface TranscriptContextMenuProps {
  contextMenu: ContextMenuState;
  contextWord: Word | null;
  selectionRange: [number, number] | null;
  onDelete: () => void;
  onRestore: () => void;
  onSilence: () => void;
  onSplit: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onRestoreAll: () => void;
  onClose: () => void;
}

const TranscriptContextMenu: React.FC<TranscriptContextMenuProps> = React.memo(({
  contextMenu,
  contextWord,
  selectionRange,
  onDelete,
  onRestore,
  onSilence,
  onSplit,
  onUndo,
  onRedo,
  onRestoreAll,
  onClose,
}) => {
  const { t } = useTranslation();

  if (!contextMenu.visible) return null;

  return (
    <div
      className="fixed z-50 min-w-[160px] rounded-md border border-[rgba(128,128,128,0.2)] bg-[#252525] py-1 shadow-lg"
      style={{ left: contextMenu.x, top: contextMenu.y }}
    >
      {contextWord && !contextWord.deleted && (
        <button
          className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
          onClick={onDelete}
        >
          {selectionRange ? t("editor.deleteRange") : t("editor.deleteWord")}
        </button>
      )}
      {contextWord && contextWord.deleted && (
        <button
          className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
          onClick={onRestore}
        >
          {t("editor.restoreWord")}
        </button>
      )}
      {contextWord && !contextWord.deleted && (
        <button
          className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
          onClick={onSilence}
        >
          {t("editor.silenceWord")}
        </button>
      )}
      {contextWord && !contextWord.deleted && contextWord.text.length > 1 && (
        <button
          className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
          onClick={onSplit}
        >
          {t("editor.splitWord")}
        </button>
      )}
      <div className="my-1 border-t border-[rgba(128,128,128,0.2)]" />
      <button
        className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
        onClick={async () => {
          await onUndo();
          onClose();
        }}
      >
        {t("editor.undo")}
      </button>
      <button
        className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
        onClick={async () => {
          await onRedo();
          onClose();
        }}
      >
        {t("editor.redo")}
      </button>
      <div className="my-1 border-t border-[rgba(128,128,128,0.2)]" />
      <button
        className="w-full px-3 py-1.5 text-left text-sm text-[#F0F0F0] hover:bg-[rgba(128,128,128,0.2)]"
        onClick={async () => {
          await onRestoreAll();
          onClose();
        }}
      >
        {t("editor.restoreAll")}
      </button>
    </div>
  );
});

TranscriptContextMenu.displayName = "TranscriptContextMenu";

export default TranscriptContextMenu;
export type { ContextMenuState };
