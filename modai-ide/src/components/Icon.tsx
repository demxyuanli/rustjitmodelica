import type { ComponentProps } from "react";
import {
  FolderTree,
  GitBranch,
  Search,
  Bot,
  FileDiff,
  Play,
  CheckCircle2,
  Settings,
  SlidersHorizontal,
  ListChecks,
  Table as TableIcon,
  BarChart3,
  Columns3,
  ChevronLeft,
  ChevronRight,
  X,
  RefreshCw,
  Plus,
  Minus,
  Languages,
  GitCommit,
  AlertTriangle,
  AlertCircle,
  Link,
  BookOpenText,
  ScrollText,
  Loader2,
  User,
  Copy,
  Check,
  SendHorizontal,
  MessageSquarePlus,
  ArrowDown,
  History,
} from "lucide-react";

export type AppIconName =
  | "explorer"
  | "sourceControl"
  | "search"
  | "ai"
  | "diff"
  | "run"
  | "validate"
  | "settings"
  | "simSettings"
  | "variables"
  | "table"
  | "chart"
  | "columns"
  | "prev"
  | "next"
  | "close"
  | "refresh"
  | "stage"
  | "unstage"
  | "language"
  | "gitCommit"
  | "warning"
  | "error"
  | "index"
  | "iterate"
  | "tests"
  | "link"
  | "library"
  | "output"
  | "spinner"
  | "user"
  | "copy"
  | "check"
  | "send"
  | "newChat"
  | "arrowDown"
  | "history";

type IconProps = Omit<ComponentProps<"svg">, "ref">;

export function AppIcon({ name, className, ...rest }: { name: AppIconName } & IconProps) {
  const sizeClass = "w-4 h-4";
  const mergedClassName = className ? `${sizeClass} ${className}` : sizeClass;

  switch (name) {
    case "explorer":
      return <FolderTree className={mergedClassName} {...rest} />;
    case "sourceControl":
      return <GitBranch className={mergedClassName} {...rest} />;
    case "search":
      return <Search className={mergedClassName} {...rest} />;
    case "ai":
      return <Bot className={mergedClassName} {...rest} />;
    case "diff":
      return <FileDiff className={mergedClassName} {...rest} />;
    case "run":
      return <Play className={mergedClassName} {...rest} />;
    case "validate":
      return <CheckCircle2 className={mergedClassName} {...rest} />;
    case "settings":
    case "simSettings":
      return <Settings className={mergedClassName} {...rest} />;
    case "variables":
      return <SlidersHorizontal className={mergedClassName} {...rest} />;
    case "table":
      return <TableIcon className={mergedClassName} {...rest} />;
    case "chart":
      return <BarChart3 className={mergedClassName} {...rest} />;
    case "columns":
      return <Columns3 className={mergedClassName} {...rest} />;
    case "prev":
      return <ChevronLeft className={mergedClassName} {...rest} />;
    case "next":
      return <ChevronRight className={mergedClassName} {...rest} />;
    case "close":
      return <X className={mergedClassName} {...rest} />;
    case "refresh":
      return <RefreshCw className={mergedClassName} {...rest} />;
    case "stage":
      return <Plus className={mergedClassName} {...rest} />;
    case "unstage":
      return <Minus className={mergedClassName} {...rest} />;
    case "language":
      return <Languages className={mergedClassName} {...rest} />;
    case "gitCommit":
      return <GitCommit className={mergedClassName} {...rest} />;
    case "warning":
      return <AlertTriangle className={mergedClassName} {...rest} />;
    case "error":
      return <AlertCircle className={mergedClassName} {...rest} />;
    case "index":
      return <ListChecks className={mergedClassName} {...rest} />;
    case "iterate":
      return <RefreshCw className={mergedClassName} {...rest} />;
    case "tests":
      return <ListChecks className={mergedClassName} {...rest} />;
    case "link":
      return <Link className={mergedClassName} {...rest} />;
    case "library":
      return <BookOpenText className={mergedClassName} {...rest} />;
    case "output":
      return <ScrollText className={mergedClassName} {...rest} />;
    case "spinner":
      return <Loader2 className={mergedClassName} {...rest} />;
    case "user":
      return <User className={mergedClassName} {...rest} />;
    case "copy":
      return <Copy className={mergedClassName} {...rest} />;
    case "check":
      return <Check className={mergedClassName} {...rest} />;
    case "send":
      return <SendHorizontal className={mergedClassName} {...rest} />;
    case "newChat":
      return <MessageSquarePlus className={mergedClassName} {...rest} />;
    case "arrowDown":
      return <ArrowDown className={mergedClassName} {...rest} />;
    case "history":
      return <History className={mergedClassName} {...rest} />;
    default:
      return null;
  }
}

