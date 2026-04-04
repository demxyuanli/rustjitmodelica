import React from "react";
import { Box, Text } from "ink";
import SelectInput from "ink-select-input";
import type { SessionContext } from "./replDispatch.js";
import { Panel, TipLine, TreeKV } from "./chrome.js";

const CLOSE = "__close__";

type Props = {
  session: SessionContext;
  onPick: (line: string) => void;
  onClose: () => void;
};

export function QuickMenu({ session, onPick, onClose }: Props) {
  const runLine = session.progress ? "run --progress" : "run";
  const items = [
    { label: "plan", value: "plan" },
    {
      label: session.progress ? "run (--progress)" : "run",
      value: runLine,
    },
    { label: "scope list (repl-exec)", value: "scope list" },
    { label: "status", value: "status" },
    { label: "validate", value: "validate" },
    { label: "ctx", value: "ctx" },
    { label: "help", value: "help" },
    { label: "Close menu", value: CLOSE },
  ];

  const cfgHint = session.config
    ? session.config
    : "(set config for plan/run/validate)";

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Panel title="Quick menu" titleColor="magenta" borderColor="magenta">
        <TreeKV label="config" value={cfgHint} valueTone="cyan" />
        <TreeKV
          label="keys"
          value="j/k or arrows, Enter, Esc to close"
          last
          valueTone="gray"
        />
        <TipLine text="Choosing an item runs the same command as typing it at the prompt." />
        <Box marginTop={1} flexDirection="column">
          <Text bold color="white">
            Actions
          </Text>
          <SelectInput
            items={items}
            isFocused
            onSelect={(item) => {
              if (item.value === CLOSE) {
                onClose();
              } else {
                onPick(String(item.value));
              }
            }}
          />
        </Box>
      </Panel>
    </Box>
  );
}
