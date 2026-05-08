import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  onModelSelect: (modelId: string) => void;
}

function hasWordLevelTimestamps(model: ModelInfo): boolean {
  return model.engine_type === "Parakeet";
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  onModelSelect,
}) => {
  const { t } = useTranslation();
  const downloadedModels = models.filter(
    (m) => m.is_downloaded && m.category === "Transcription",
  );

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  return (
    <div className="absolute bottom-full start-0 mb-2 w-64 max-h-[60vh] overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50">
      {downloadedModels.length > 0 ? (
        <div>
          {downloadedModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className={`w-full px-3 py-2 text-start hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                currentModelId === model.id
                  ? "bg-logo-primary/10 text-logo-primary"
                  : ""
              }`}
            >
              <div className="flex items-center justify-between">
                <div className="min-w-0 flex-1">
                  <div className="text-sm text-text/80 flex items-center gap-1.5">
                    {getTranslatedModelName(model, t)}
                    {model.is_custom && (
                      <span className="text-[10px] font-medium text-text/40 uppercase">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                    {model.is_recommended && (
                      <span className="text-[10px] font-medium text-logo-primary/80 uppercase">
                        {t("modelSelector.recommended")}
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-text/40 italic pe-4">
                    {getTranslatedModelDescription(model, t)}
                  </div>
                  <div className="mt-0.5">
                    {hasWordLevelTimestamps(model) ? (
                      <span
                        className="inline-flex items-center gap-0.5 text-[10px] font-medium text-green-600 dark:text-green-400"
                        title={t("modelSelector.wordLevelTooltip")}
                      >
                        ⚡ {t("modelSelector.wordLevelTimestamps")}
                      </span>
                    ) : (
                      <span
                        className="inline-flex items-center gap-0.5 text-[10px] font-medium text-text/30"
                        title={t("modelSelector.approximateTooltip")}
                      >
                        ⚠ {t("modelSelector.approximateTimestamps")}
                      </span>
                    )}
                  </div>
                </div>
                {currentModelId === model.id && (
                  <div className="text-xs text-logo-primary shrink-0">
                    {t("modelSelector.active")}
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="px-3 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
