import React from "react";
import { useTranslation } from "react-i18next";
import { Replace, Search, X } from "lucide-react";
import { Button } from "@/components/ui/Button";

type SearchMode = "exact" | "fuzzy" | "phonetic";

const SEARCH_MODE_OPTIONS = [
  { value: "exact", labelKey: "editor.searchExact" },
  { value: "fuzzy", labelKey: "editor.searchFuzzy" },
  { value: "phonetic", labelKey: "editor.searchPhonetic" },
] as const satisfies ReadonlyArray<{
  value: SearchMode;
  labelKey: string;
}>;

interface FindReplaceBarProps {
  findQuery: string;
  replaceText: string;
  searchMode: SearchMode;
  findMatchIndex: number;
  findMatchCount: number;
  findInputRef: React.RefObject<HTMLInputElement>;
  onQueryChange: (query: string) => void;
  onReplaceTextChange: (text: string) => void;
  onSearchModeChange: (mode: SearchMode) => void;
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
    searchMode,
    findMatchIndex,
    findMatchCount,
    findInputRef,
    onQueryChange,
    onReplaceTextChange,
    onSearchModeChange,
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
            <Button
              variant="danger-ghost"
              size="sm"
              onClick={onDeleteAll}
              className="!py-0.5 !text-[11px]"
            >
              {t("editor.deleteAll")}
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={onClose}
            className="!px-1 !py-1 text-mid-gray/60 hover:!text-mid-gray"
          >
            <X size={14} />
          </Button>
        </div>
        <div className="flex items-center gap-1 pl-5">
          <span className="mr-1 text-[11px] text-mid-gray/60">
            {t("editor.searchMode")}:
          </span>
          {SEARCH_MODE_OPTIONS.map(({ value, labelKey }) => (
            <Button
              key={value}
              variant={searchMode === value ? "primary-soft" : "ghost"}
              size="sm"
              onClick={() => onSearchModeChange(value)}
              className="!px-1.5 !py-0 !text-[10px]"
            >
              {t(labelKey)}
            </Button>
          ))}
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
