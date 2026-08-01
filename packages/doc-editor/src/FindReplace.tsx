import { Show } from "solid-js";
import { FindBar } from "@redoc/editor-common";

export interface FindReplaceProps {
  open: boolean;
  query: string;
  replaceWith: string;
  matchCount: number;
  matchIndex: number;
  matchCase: boolean;
  onQueryChange: (q: string) => void;
  onReplaceChange: (r: string) => void;
  onFind: () => void;
  onFindNext: () => void;
  onFindPrev: () => void;
  onReplace: () => void;
  onReplaceAll: () => void;
  onMatchCaseChange: (mc: boolean) => void;
  onClose: () => void;
}

export function FindReplace(props: FindReplaceProps) {
  return (
    <Show when={props.open}>
      <FindBar
        query={props.query}
        replaceWith={props.replaceWith}
        matchCount={props.matchCount}
        matchIndex={props.matchIndex}
        matchCase={props.matchCase}
        showReplace={true}
        onQueryChange={props.onQueryChange}
        onReplaceChange={props.onReplaceChange}
        onFind={props.onFind}
        onFindNext={props.onFindNext}
        onFindPrev={props.onFindPrev}
        onReplace={props.onReplace}
        onReplaceAll={props.onReplaceAll}
        onMatchCaseChange={props.onMatchCaseChange}
        onClose={props.onClose}
      />
    </Show>
  );
}
