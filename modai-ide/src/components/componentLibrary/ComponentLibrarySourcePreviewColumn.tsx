import Editor from "@monaco-editor/react";
import { t } from "../../i18n";
import type { ComponentTypeSource, InstantiableClass } from "../../types";
import type { DependencyGraphBehavior } from "../../utils/dependencyGraphBehavior";
import { LibraryRelationGraphPane } from "../LibraryRelationGraphPane";
import { SectionIcon } from "./ComponentLibraryTypeTreePane";

export interface ComponentLibrarySourcePreviewColumnProps {
  theme: "dark" | "light";
  projectDir: string | null;
  source: ComponentTypeSource | null;
  selectedClass: InstantiableClass | null;
  onOpenDependencyGraphSettings?: () => void;
  dependencyGraphBehavior: DependencyGraphBehavior;
}

export function ComponentLibrarySourcePreviewColumn({
  theme,
  projectDir,
  source,
  selectedClass,
  onOpenDependencyGraphSettings,
  dependencyGraphBehavior,
}: ComponentLibrarySourcePreviewColumnProps) {
  return (
    <section className="min-w-0 flex-1 flex-[1.1] w-full bg-[var(--bg-elevated)]">
      <div className="flex h-full min-h-0 w-full flex-col">
        <div className="min-h-0 flex-[0.58] flex flex-col border-b border-border">
          <div className="panel-header-bar shrink-0 flex items-center gap-2 border-b border-border">
            <SectionIcon letter="S" bg="bg-violet-500/85" title={t("librarySourcePreview")} />
            <span className="text-sm font-medium">{t("librarySourcePreview")}</span>
          </div>
          <div className="min-h-0 flex-1">
            <Editor
              height="100%"
              defaultLanguage="modelica"
              language="modelica"
              theme={theme === "light" ? "vs-light" : "vs-dark"}
              value={source?.content ?? ""}
              options={{
                readOnly: true,
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                wordWrap: "on",
                lineNumbersMinChars: 3,
              }}
              beforeMount={(monaco) => {
                if (monaco.languages.getLanguages().some((lang: { id: string }) => lang.id === "modelica")) return;
                monaco.languages.register({ id: "modelica" });
                monaco.languages.setMonarchTokensProvider("modelica", {
                  defaultToken: "",
                  tokenPostfix: ".mo",
                  keywords: [
                    "model", "end", "equation", "algorithm", "initial", "extends",
                    "parameter", "flow", "connect", "if", "then", "else", "elseif",
                    "for", "loop", "in", "while", "when", "elsewhen", "partial",
                    "input", "output", "package", "constant", "terminal", "function",
                    "each", "redeclare", "annotation", "assert", "terminate",
                    "operator", "type", "external", "replaceable", "record", "block",
                    "class", "connector", "reinit",
                  ],
                  typeKeywords: ["Real", "Integer", "Boolean", "String"],
                  operators: ["=", ":=", "+", "-", "*", "/", "^", "and", "or", "not"],
                  tokenizer: {
                    root: [
                      [/\b(parameter|constant|flow|discrete|input|output)\b/, "keyword"],
                      [/\b(model|block|class|connector|record|package|function|operator)\b/, "keyword"],
                      [/\b(equation|algorithm|initial|extends|each|redeclare)\b/, "keyword"],
                      [/\b(if|then|else|elseif|for|loop|in|while|when|elsewhen)\b/, "keyword"],
                      [/\b(connect|reinit|assert|terminate|annotation|external)\b/, "keyword"],
                      [/\b(end|partial|replaceable|type)\b/, "keyword"],
                      [/\b(der|pre)\s*\(/, "keyword"],
                      [/\b(Real|Integer|Boolean|String)\b/, "type"],
                      [/"[^"]*"/, "string"],
                      [/\/\/.*$/, "comment"],
                      [/\/\*/, "comment", "@comment"],
                      [/\d+\.?\d*([eE][+-]?\d+)?/, "number"],
                      [/[{}()\[\];,]/, "delimiter"],
                      [/[=:]/, "operator"],
                      [/[+\-*\/^]/, "operator"],
                      [/\b(and|or|not)\b/, "operator"],
                    ],
                    comment: [
                      [/[^\/*]+/, "comment"],
                      [/\*\//, "comment", "@pop"],
                      [/[\/*]/, "comment"],
                    ],
                  },
                });
              }}
            />
          </div>
        </div>
        <div className="min-h-0 flex-[0.42]">
          <LibraryRelationGraphPane
            code={source?.content ?? null}
            modelName={selectedClass?.qualifiedName ?? null}
            projectDir={projectDir}
            onOpenDependencyGraphSettings={onOpenDependencyGraphSettings}
            dependencyGraphBehavior={dependencyGraphBehavior}
          />
        </div>
      </div>
    </section>
  );
}
