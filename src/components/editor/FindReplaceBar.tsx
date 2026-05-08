import React from "react";
import { useTranslation } from "react-i18next";
import { Replace, Search, X } from "lucide-react";
import { Button } from "@/components/ui/Button";

interface FindReplaceBarProps {
  findQuery: string;
  replaceText: string;
  findMatchIndex: number;
  findMatchCount: number;
  findInputRef: React.RefObject<HTMLInputElement>;
  onQueryChange: (query: string) => void;
  onReplaceTextChange: (text: string) => void;
  onMatchIndexReset: () => void;
  onNavigate: (direction: 1 | -1) => void;
  onReplaceOne: () => void;
  onReplaceAll: () => void;
  onDeleteAll: () => void;
  onClose: () => void;
}

const FindReplaceBar: React.FC<FindReplaceBarProps> = React.memo(
  ({
    findQuery,
    replaceText,
    findMatchIndex,
    findMatchCount,
    findInputRef,
    onQueryChange,
    onReplaceTextChange,
    onMatchIndexReset,
    onNavigate,
    onReplaceOne,
    onReplaceAll,
    onDeleteAll,
    onClose,
  }) => {
    const { t } = useTranslation();

    return (
      <div className="flex flex-col gap-2 mb-3 p-2 rounded-lg bg-background border border-mid-gray/20">
        {/* Find row */}
        <div className="flex items-center gap-2">
          <Search size={14} className="text-mid-gray/60 shrink-0" />
          <input
            ref={findInputRef}
            type="text"
            value={findQuery}
            onChange={(e) => {
              onQueryChange(e.target.value);
              onMatchIndexReset();
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") onNavigate(e.shiftKey ? -1 : 1);
              if (e.key === "Escape") onClose();
            }}
            placeholder={t("editor.findPlaceholder")}
            className="flex-1 bg-transparent text-sm text-text outline-none placeholder:text-mid-gray/40"
          />
          {findMatchCount > 0 && (
            <span className="text-[11px] text-mid-gray/60 shrink-0">
              {findMatchIndex + 1}/{findMatchCount}
            </span>
          )}
          {findMatchCount > 0 && (
            <button
              onClick={onDeleteAll}
              className="px-2 py-0.5 text-[11px] text-red-400 bg-red-900/20 rounded hover:bg-red-900/40 transition-colors"
            >
              {t("editor.deleteAll")}
            </button>
          )}
          <button
            onClick={onClose}
            className="text-mid-gray/60 hover:text-mid-gray transition-colors"
          >
            <X size={14} />
          </button>
        </div>
        {/* Replace row */}
        <div className="flex items-center gap-2">
          <Replace size={14} className="text-mid-gray/60 shrink-0" />
          <input
            type="text"
            value={replaceText}
            onChange={(e) => onReplaceTextChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onReplaceOne();
              if (e.key === "Escape") onClose();
            }}
            placeholder={t("editor.replacePlaceholder")}
            className="flex-1 bg-transparent text-sm text-text outline-none placeholder:text-mid-gray/40"
          />
          {findMatchCount > 0 && (
            <>
              <Button
                variant="primary-soft"
                size="sm"
                onClick={onReplaceOne}
                className="!py-0.5 !text-[11px] !text-logo-primary"
              >
                {t("editor.replaceOne")}
              </Button>
              <Button
                variant="primary-soft"
                size="sm"
                onClick={onReplaceAll}
                className="!py-0.5 !text-[11px] !text-logo-primary"
              >
                {t("editor.replaceAll")}
              </Button>
            </>
          )}
        </div>
      </div>
    );
  },
);

FindReplaceBar.displayName = "FindReplaceBar";

export default FindReplaceBar;
