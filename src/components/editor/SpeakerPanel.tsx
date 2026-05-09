import React, { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GitMerge, Sparkles, Trash2, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/Button";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { type SpeakerInfo, useEditorStore } from "@/stores/editorStore";
import SpeakerPanelRow from "./SpeakerPanelRow";
import { toSpeakerNameMap } from "./speakerPanelUtils";

const SpeakerPanel: React.FC = () => {
  const { t } = useTranslation();
  const { words, selectedIndex, selectionRange, refreshFromBackend, setSpeakerNames } =
    useEditorStore(
      useShallow((state) => ({
        words: state.words,
        selectedIndex: state.selectedIndex,
        selectionRange: state.selectionRange,
        refreshFromBackend: state.refreshFromBackend,
        setSpeakerNames: state.setSpeakerNames,
      })),
    );
  const [speakers, setSpeakers] = useState<SpeakerInfo[]>([]);
  const [assignSpeakerId, setAssignSpeakerId] = useState<string | null>(null);
  const [mergeFromId, setMergeFromId] = useState<string | null>(null);
  const [mergeToId, setMergeToId] = useState<string | null>(null);
  const [editingSpeakerId, setEditingSpeakerId] = useState<number | null>(null);
  const [draftName, setDraftName] = useState("");

  const selectedRange: [number, number] | null =
    selectionRange ??
    (selectedIndex === null ? null : [selectedIndex, selectedIndex]);

  const nextSpeakerId =
    speakers.length === 0
      ? 0
      : Math.max(...speakers.map((speaker) => speaker.id)) + 1;

  const applySpeakers = useCallback(
    (nextSpeakers: SpeakerInfo[]) => {
      setSpeakers(nextSpeakers);
      setSpeakerNames(toSpeakerNameMap(nextSpeakers));
    },
    [setSpeakerNames],
  );

  const loadSpeakers = useCallback(async () => {
    try {
      const nextSpeakers = await invoke<SpeakerInfo[]>("get_speakers");
      applySpeakers(nextSpeakers);
    } catch (error) {
      console.error("Failed to load speakers", error);
      toast.error(t("editor.noSpeakers"));
    }
  }, [applySpeakers, t]);

  useEffect(() => {
    void loadSpeakers();
  }, [loadSpeakers, words]);

  useEffect(() => {
    if (speakers.length === 0) {
      setAssignSpeakerId(String(nextSpeakerId));
      setMergeFromId(null);
      setMergeToId(null);
      return;
    }

    setAssignSpeakerId((current) => current ?? String(speakers[0].id));
    setMergeFromId((current) =>
      current && speakers.some((speaker) => String(speaker.id) === current)
        ? current
        : String(speakers[0].id),
    );
    setMergeToId((current) =>
      current && speakers.some((speaker) => String(speaker.id) === current)
        ? current
        : String(speakers[0].id),
    );
  }, [nextSpeakerId, speakers]);

  const speakerOptions = useMemo<DropdownOption[]>(() => {
    const options = speakers.map((speaker) => ({
      value: String(speaker.id),
      label: speaker.name || t("editor.speaker", { id: speaker.id + 1 }),
    }));

    if (!options.some((option) => option.value === String(nextSpeakerId))) {
      options.push({
        value: String(nextSpeakerId),
        label: t("editor.speaker", { id: nextSpeakerId + 1 }),
      });
    }

    return options;
  }, [nextSpeakerId, speakers, t]);

  const existingSpeakerOptions = useMemo<DropdownOption[]>(() => {
    return speakers.map((speaker) => ({
      value: String(speaker.id),
      label: speaker.name || t("editor.speaker", { id: speaker.id + 1 }),
    }));
  }, [speakers, t]);

  const handleAutoAssign = useCallback(async () => {
    try {
      const nextSpeakers = await invoke<SpeakerInfo[]>("auto_assign_speakers", {
        minGapUs: null,
        maxSpeakers: null,
      });
      await refreshFromBackend();
      applySpeakers(nextSpeakers);
    } catch (error) {
      console.error("Failed to auto-assign speakers", error);
      toast.error(t("editor.autoAssignSpeakers"));
    }
  }, [applySpeakers, refreshFromBackend, t]);

  const handleClear = useCallback(async () => {
    try {
      await invoke<void>("clear_speakers");
      await refreshFromBackend();
      setSpeakers([]);
      setSpeakerNames({});
      setAssignSpeakerId(String(nextSpeakerId));
      setMergeFromId(null);
      setMergeToId(null);
    } catch (error) {
      console.error("Failed to clear speakers", error);
      toast.error(t("editor.clearSpeakers"));
    }
  }, [nextSpeakerId, refreshFromBackend, setSpeakerNames, t]);

  const handleAssign = useCallback(async () => {
    if (!selectedRange || !assignSpeakerId) {
      return;
    }

    try {
      await invoke<void>("assign_speaker_to_range", {
        startIndex: selectedRange[0],
        endIndex: selectedRange[1],
        speakerId: Number(assignSpeakerId),
      });
      await refreshFromBackend();
      await loadSpeakers();
    } catch (error) {
      console.error("Failed to assign speaker range", error);
      toast.error(t("editor.assignSpeaker"));
    }
  }, [assignSpeakerId, loadSpeakers, refreshFromBackend, selectedRange, t]);

  const handleMerge = useCallback(async () => {
    if (!mergeFromId || !mergeToId || mergeFromId === mergeToId) {
      return;
    }

    try {
      const nextSpeakers = await invoke<SpeakerInfo[]>("merge_speakers", {
        fromId: Number(mergeFromId),
        toId: Number(mergeToId),
      });
      await refreshFromBackend();
      applySpeakers(nextSpeakers);
      setMergeFromId(null);
    } catch (error) {
      console.error("Failed to merge speakers", error);
      toast.error(t("editor.mergeSpeakers"));
    }
  }, [applySpeakers, mergeFromId, mergeToId, refreshFromBackend, t]);

  const beginRename = useCallback(
    (speaker: SpeakerInfo) => {
      setEditingSpeakerId(speaker.id);
      setDraftName(speaker.name || t("editor.speaker", { id: speaker.id + 1 }));
    },
    [t],
  );

  const handleRename = useCallback(async () => {
    if (editingSpeakerId === null) {
      return;
    }

    try {
      await invoke<void>("rename_speaker", {
        speakerId: editingSpeakerId,
        name: draftName,
      });
      await loadSpeakers();
    } catch (error) {
      console.error("Failed to rename speaker", error);
      toast.error(t("editor.renameSpeaker"));
    } finally {
      setEditingSpeakerId(null);
      setDraftName("");
    }
  }, [draftName, editingSpeakerId, loadSpeakers, t]);

  return (
    <SettingsGroup title={t("editor.speakerPanel")}>
      <div className="space-y-3 px-4 py-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            type="button"
            variant="primary-soft"
            size="sm"
            onClick={handleAutoAssign}
            className="inline-flex items-center gap-1.5"
          >
            <Sparkles size={14} />
            {t("editor.autoAssignSpeakers")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={handleClear}
            className="inline-flex items-center gap-1.5"
          >
            <Trash2 size={14} />
            {t("editor.clearSpeakers")}
          </Button>
        </div>

        <div className="space-y-2 rounded-lg border border-mid-gray/20 bg-background p-3">
          <p className="text-xs font-medium uppercase tracking-wide text-mid-gray">
            {t("editor.assignSpeaker")}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Dropdown
              options={speakerOptions}
              selectedValue={assignSpeakerId}
              onSelect={setAssignSpeakerId}
            />
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={handleAssign}
              disabled={!selectedRange}
            >
              {selectedRange
                ? `${selectedRange[0] + 1}-${selectedRange[1] + 1}`
                : t("editor.assignSpeaker")}
            </Button>
          </div>
        </div>

        <div className="space-y-2 rounded-lg border border-mid-gray/20 bg-background p-3">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-mid-gray">
            <GitMerge size={14} />
            {t("editor.mergeSpeakers")}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Dropdown
              options={existingSpeakerOptions}
              selectedValue={mergeFromId}
              onSelect={setMergeFromId}
              disabled={existingSpeakerOptions.length < 2}
            />
            <span className="text-xs text-mid-gray">{t("editor.mergeInto")}</span>
            <Dropdown
              options={existingSpeakerOptions}
              selectedValue={mergeToId}
              onSelect={setMergeToId}
              disabled={existingSpeakerOptions.length < 2}
            />
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={handleMerge}
              disabled={
                existingSpeakerOptions.length < 2 ||
                !mergeFromId ||
                !mergeToId ||
                mergeFromId === mergeToId
              }
            >
              {t("editor.mergeSpeakers")}
            </Button>
          </div>
        </div>

        <div className="space-y-2">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-mid-gray">
            <Users size={14} />
            {t("editor.speakers")}
          </div>
          {speakers.length === 0 ? (
            <div className="rounded-lg border border-dashed border-mid-gray/30 px-3 py-4 text-sm text-mid-gray">
              {t("editor.noSpeakers")}
            </div>
          ) : (
            speakers.map((speaker) => (
              <SpeakerPanelRow
                key={speaker.id}
                speaker={speaker}
                editingSpeakerId={editingSpeakerId}
                draftName={draftName}
                onDraftNameChange={setDraftName}
                onBeginRename={beginRename}
                onCommitRename={handleRename}
                onCancelRename={() => {
                  setEditingSpeakerId(null);
                  setDraftName("");
                }}
              />
            ))
          )}
        </div>
      </div>
    </SettingsGroup>
  );
};

export default SpeakerPanel;
