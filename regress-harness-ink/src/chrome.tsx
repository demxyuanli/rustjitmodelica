import React, { type ReactNode } from "react";
import { Box, Text } from "ink";

export function termWidth(): number {
  const c = process.stdout.columns;
  if (typeof c === "number" && c > 0) {
    return Math.min(Math.max(c - 4, 48), 100);
  }
  return 72;
}

export function Divider(): React.ReactElement {
  const w = termWidth();
  return <Text dimColor>{"\u2500".repeat(w)}</Text>;
}

export function DoubleDivider(): React.ReactElement {
  const w = termWidth();
  return <Text dimColor>{"\u2550".repeat(w)}</Text>;
}

type ValueTone = "default" | "green" | "red" | "yellow" | "cyan" | "gray";

export function TreeKV(props: {
  label: string;
  value: string;
  last?: boolean;
  valueTone?: ValueTone;
}): React.ReactElement {
  const br = props.last ? "\u2514\u2500 " : "\u251c\u2500 ";
  const tone = props.valueTone ?? "default";
  const color =
    tone === "green"
      ? "green"
      : tone === "red"
        ? "red"
        : tone === "yellow"
          ? "yellow"
          : tone === "cyan"
            ? "cyan"
            : tone === "gray"
              ? "gray"
              : undefined;
  return (
    <Box flexDirection="row" flexWrap="wrap">
      <Text dimColor>{br}</Text>
      <Text bold color="white">
        {props.label}
      </Text>
      <Text dimColor>:</Text>
      <Text color={color} wrap="wrap">
        {" "}
        {props.value}
      </Text>
    </Box>
  );
}

export function Panel(props: {
  title: string;
  titleColor?: string;
  borderColor?: string;
  children: ReactNode;
}): React.ReactElement {
  const tc = props.titleColor ?? "cyan";
  const bc = props.borderColor ?? "cyan";
  return (
    <Box
      flexDirection="column"
      borderStyle="single"
      borderColor={bc}
      paddingX={1}
      paddingY={0}
      marginBottom={1}
    >
      <Text bold color={tc}>
        {props.title}
      </Text>
      <Divider />
      <Box flexDirection="column" marginTop={0}>
        {props.children}
      </Box>
    </Box>
  );
}

export function SubPanel(props: {
  title: string;
  accent?: string;
  children: ReactNode;
}): React.ReactElement {
  const a = props.accent ?? "gray";
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text bold color={a}>
        {"\u251c\u2500 "}
        {props.title}
      </Text>
      <Box marginLeft={2} flexDirection="column">
        {props.children}
      </Box>
    </Box>
  );
}

export function TipLine(props: { text: string }): React.ReactElement {
  return (
    <Box flexDirection="row">
      <Text color="yellow" bold>
        {"\u2514\u2500 "}
      </Text>
      <Text dimColor italic>
        {props.text}
      </Text>
    </Box>
  );
}

export function TipsBlock(props: { lines: string[] }): React.ReactElement {
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text bold color="yellow">
        Tips
      </Text>
      <Divider />
      {props.lines.map((line, i) => (
        <TipLine key={i} text={line} />
      ))}
    </Box>
  );
}
