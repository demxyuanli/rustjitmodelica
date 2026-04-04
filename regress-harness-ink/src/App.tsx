import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Box, Text, useApp, useInput } from "ink";
import TextInput from "ink-text-input";
import Spinner from "ink-spinner";
import type { ChildProcess } from "node:child_process";
import { resolveHarnessExe } from "./harness.js";
import {
  dispatchLine,
  formatPrompt,
  type SessionContext,
} from "./replDispatch.js";
import { startHarnessJob } from "./spawnHarness.js";
import {
  loadSessionFromDisk,
  resolveWorkspaceRoot,
  saveSessionToDisk,
} from "./sessionStore.js";
import { QuickMenu } from "./QuickMenu.js";
import {
  Divider,
  DoubleDivider,
  Panel,
  SubPanel,
  TipsBlock,
  TreeKV,
} from "./chrome.js";

const MAX_LOG_OUT = 128;
const MAX_LOG_ERR = 48;

function tailLines(text: string, maxLines: number): string {
  const lines = text.split(/\r?\n/);
  if (lines.length <= maxLines) {
    return text;
  }
  return lines.slice(-maxLines).join("\n");
}

const TIP_LINES = [
  "Ctrl+G or type menu + Enter opens the quick menu; Esc closes it.",
  "Session context is saved under workspace root: .regress-harness-ink-session.json",
  "While a job runs: stdout vs stderr split; spinner shows elapsed seconds.",
  "Ctrl+C kills the child process first; press again to exit the UI.",
];

export function App() {
  const { exit } = useApp();
  const cwd = process.cwd();
  const workspaceRoot = useMemo(() => resolveWorkspaceRoot(cwd), [cwd]);
  const exe = useMemo(() => resolveHarnessExe(cwd), [cwd]);

  const [session, setSession] = useState<SessionContext>(() =>
    loadSessionFromDisk(workspaceRoot)
  );
  const [line, setLine] = useState("");
  const [logOut, setLogOut] = useState("");
  const [logErr, setLogErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [elapsedSec, setElapsedSec] = useState(0);
  const childRef = useRef<ChildProcess | null>(null);
  const sessionRef = useRef(session);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  sessionRef.current = session;

  const appendOut = useCallback((chunk: string) => {
    setLogOut((prev) => tailLines(prev + chunk, MAX_LOG_OUT));
  }, []);

  const appendErr = useCallback((chunk: string) => {
    setLogErr((prev) => tailLines(prev + chunk, MAX_LOG_ERR));
  }, []);

  useEffect(() => {
    if (!busy) {
      setElapsedSec(0);
      return;
    }
    setElapsedSec(0);
    const id = setInterval(() => {
      setElapsedSec((n) => n + 1);
    }, 1000);
    return () => clearInterval(id);
  }, [busy]);

  useEffect(() => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(() => {
      saveSessionToDisk(workspaceRoot, session);
      saveTimerRef.current = null;
    }, 300);
    return () => {
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
    };
  }, [session, workspaceRoot]);

  useEffect(() => {
    const flush = () => {
      saveSessionToDisk(workspaceRoot, sessionRef.current);
    };
    process.once("beforeExit", flush);
    return () => {
      process.removeListener("beforeExit", flush);
    };
  }, [workspaceRoot]);

  useInput(
    (input, key) => {
      if (key.ctrl && input === "c") {
        if (childRef.current) {
          childRef.current.kill();
          childRef.current = null;
          setBusy(false);
          appendOut("\n[interrupted]\n");
          return;
        }
        exit();
      }
    },
    { isActive: true }
  );

  useInput(
    (_input, key) => {
      if (key.escape) {
        setMenuOpen(false);
      }
    },
    { isActive: menuOpen }
  );

  useInput(
    (input, key) => {
      if (key.ctrl && (input === "g" || input === "G")) {
        setMenuOpen(true);
      }
    },
    { isActive: !menuOpen && !busy }
  );

  const runDispatch = useCallback(
    (value: string) => {
      if (!exe) {
        appendOut(
          "[error] regress-harness not found. Build: cargo build -p regress-harness --release\n" +
            "Or set REGRESS_HARNESS_EXE.\n"
        );
        return;
      }
      const result = dispatchLine(value, session, cwd);
      if (result.kind === "exit") {
        exit();
        return;
      }
      if (result.kind === "noop") {
        return;
      }
      if (result.kind === "clear") {
        setLogOut("");
        setLogErr("");
        return;
      }
      if (result.kind === "log") {
        if (result.lines.length) {
          appendOut(result.lines.join("\n") + "\n");
        }
        if (result.nextCtx) {
          setSession(result.nextCtx);
        }
        return;
      }
      const sinks = {
        onStdout: appendOut,
        onStderr: appendErr,
      };
      if (result.kind === "spawn-cli") {
        setBusy(true);
        const child = startHarnessJob(
          exe,
          cwd,
          { kind: "cli", argv: result.argv },
          sinks,
          (code, signal) => {
            childRef.current = null;
            const sig = signal ? ` signal=${signal}` : "";
            appendOut(`\n[done exit=${code ?? "?"}${sig}]\n`);
            setBusy(false);
          }
        );
        childRef.current = child;
        return;
      }
      if (result.kind === "spawn-repl-exec") {
        setBusy(true);
        const child = startHarnessJob(
          exe,
          cwd,
          { kind: "repl-exec", command: result.command },
          sinks,
          (code, signal) => {
            childRef.current = null;
            const sig = signal ? ` signal=${signal}` : "";
            appendOut(`\n[done exit=${code ?? "?"}${sig}]\n`);
            setBusy(false);
          }
        );
        childRef.current = child;
      }
    },
    [appendErr, appendOut, cwd, exe, exit, session]
  );

  const onSubmit = useCallback(
    (value: string) => {
      const v = value.trim();
      if (!v) {
        return;
      }
      setLine("");
      if (v === "menu") {
        setMenuOpen(true);
        return;
      }
      runDispatch(v);
    },
    [runDispatch]
  );

  const onMenuPick = useCallback(
    (cmdLine: string) => {
      setMenuOpen(false);
      runDispatch(cmdLine);
    },
    [runDispatch]
  );

  const prompt = formatPrompt(session.cmdPrefix);
  const prefixLabel =
    session.cmdPrefix.length > 0
      ? session.cmdPrefix.join("/")
      : "(root)";

  const outputBody = busy ? (
    <Box flexDirection="row" columnGap={1}>
      <Box
        width="49%"
        flexDirection="column"
        borderStyle="single"
        borderColor="gray"
        paddingX={1}
      >
        <Text bold color="cyan">
          stdout
        </Text>
        <Text dimColor>tail {MAX_LOG_OUT} lines</Text>
        <Divider />
        <Text wrap="wrap">{logOut || " "}</Text>
      </Box>
      <Box
        width="49%"
        flexDirection="column"
        borderStyle="single"
        borderColor="red"
        paddingX={1}
      >
        <Text bold color="red">
          stderr
        </Text>
        <Text dimColor>tail {MAX_LOG_ERR} lines</Text>
        <Divider />
        <Text wrap="wrap" color="red">
          {logErr || " "}
        </Text>
      </Box>
    </Box>
  ) : (
    <>
      <SubPanel title={`stdout (last ${MAX_LOG_OUT})`} accent="cyan">
        <Text wrap="wrap">{logOut || " "}</Text>
      </SubPanel>
      {logErr.trim() ? (
        <SubPanel title={`stderr (last ${MAX_LOG_ERR})`} accent="red">
          <Text wrap="wrap" color="red">
            {logErr}
          </Text>
        </SubPanel>
      ) : null}
    </>
  );

  return (
    <Box flexDirection="column" padding={1}>
      <Panel title="regress-harness · Ink (Rust-backed)" borderColor="cyan">
        <TreeKV
          label="harness"
          value={exe ? "binary resolved" : "binary missing"}
          valueTone={exe ? "green" : "red"}
        />
        <TreeKV
          label="path"
          value={exe ?? "cargo build -p regress-harness --release"}
          valueTone={exe ? "gray" : "yellow"}
        />
        <TreeKV label="workspace" value={workspaceRoot} valueTone="gray" />
        <TreeKV label="cwd" value={cwd} valueTone="gray" />
        <TreeKV
          label="cmd_prefix"
          value={prefixLabel}
          valueTone="cyan"
          last
        />
      </Panel>

      <DoubleDivider />

      <Panel title="Session output" borderColor="gray">
        {outputBody}
      </Panel>

      <Box flexDirection="column" marginTop={1}>
        {busy ? (
          <Box
            flexDirection="row"
            borderStyle="round"
            borderColor="yellow"
            paddingX={1}
            marginBottom={1}
          >
            <Spinner type="dots" />
            <Text bold color="yellow">
              {" "}
              job running · {elapsedSec}s elapsed · Ctrl+C to interrupt
            </Text>
          </Box>
        ) : null}
        {menuOpen ? (
          <QuickMenu
            session={session}
            onPick={onMenuPick}
            onClose={() => setMenuOpen(false)}
          />
        ) : !busy ? (
          <Box
            flexDirection="column"
            borderStyle="single"
            borderColor="blue"
            paddingX={1}
            paddingY={0}
          >
            <Text dimColor>Command line</Text>
            <Divider />
            <Box flexDirection="row">
              <Text bold color="green">
                {prompt}
              </Text>
              <TextInput
                value={line}
                onChange={setLine}
                onSubmit={onSubmit}
                placeholder="plan | run | menu | help"
              />
            </Box>
          </Box>
        ) : null}
      </Box>

      <TipsBlock lines={TIP_LINES} />
    </Box>
  );
}
