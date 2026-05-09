import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";

export interface SpeakerContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  speakerId: number;
}

interface TranscriptSpeakerContextMenuProps {
  menu: SpeakerContextMenuState;
  onRenamed: () => Promise<void>;
  onClose: () => void;
}

const menuButtonClass =
  "block w-full rounded-none border-transparent px-3 py-1.5 text-left text-sm font-normal text-text hover:border-transparent hover:bg-mid-gray/20";

const TranscriptSpeakerContextMenu: React.FC<
  TranscriptSpeakerContextMenuProps
> = ({ menu, onRenamed, onClose }) => {
  const { t } = useTranslation();

  if (!menu.visible) return null;

  const handleRename = async () => {
    const name = prompt(t("editor.renameSpeaker"));
    if (name !== null) {
      try {
        await invoke("rename_speaker", {
          speakerId: menu.speakerId,
          name,
        });
        await onRenamed();
      } catch (error) {
        console.error("Rename speaker failed:", error);
      }
    }
    onClose();
  };

  return (
    <div
      className="fixed z-50 min-w-[160px] rounded-md border border-mid-gray/20 bg-background py-1 shadow-lg"
      style={{ left: menu.x, top: menu.y }}
    >
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={menuButtonClass}
        onClick={() => void handleRename()}
      >
        {t("editor.renameSpeaker")}
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={menuButtonClass}
        onClick={onClose}
      >
        {t("editor.mergeSpeakers")}
      </Button>
    </div>
  );
};

export default TranscriptSpeakerContextMenu;
