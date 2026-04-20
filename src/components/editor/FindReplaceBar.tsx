import React from "react";
import { useTranslation } from "react-i18next";
import { Search, X } from "lucide-react";

interface FindReplaceBarProps {
  findQuery: string;
  findMatchIndex: number;
  findMatchCount: number;
  findInputRef: React.RefObject<HTMLInputElement>;
  onQueryChange: (query: string) => void;
  onMatchIndexReset: () => void;
  onNavigate: (direction: 1 | -1) => void;
  onDeleteAll: () => void;
  onClose: () => void;
}

const FindReplaceBar: React.FC<FindReplaceBarProps> = React.memo(
  ({
    findQuery,
    findMatchIndex,
    findMatchCount,
    findInputRef,
    onQueryChange,
    onMatchIndexReset,
    onNavigate,
    onDeleteAll,
    onClose,
  }) => {
    const { t } = useTranslation();

    return (
      <div className="flex items-center gap-2 mb-3 p-2 rounded-lg bg-background border border-mid-gray/20">
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
    );
  },
);

FindReplaceBar.displayName = "FindReplaceBar";

export default FindReplaceBar;
