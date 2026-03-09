import { useState, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface SearchMatch {
  file: string;
  line: number;
  column: number;
  line_content: string;
}

export interface FileGroup {
  file: string;
  matches: SearchMatch[];
}

export interface SearchOptions {
  caseSensitive: boolean;
  filePattern: string;
}

export function useSearch(projectDir: string | null) {
  const [query, setQuery] = useState("");
  const [options, setOptions] = useState<SearchOptions>({
    caseSensitive: false,
    filePattern: "",
  });
  const [results, setResults] = useState<SearchMatch[]>([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const setOption = useCallback(<K extends keyof SearchOptions>(key: K, value: SearchOptions[K]) => {
    setOptions((prev) => ({ ...prev, [key]: value }));
  }, []);

  const doSearch = useCallback(
    async (q: string) => {
      if (!projectDir || !q.trim()) {
        setResults([]);
        setSearched(false);
        return;
      }
      setLoading(true);
      setSearched(true);
      try {
        const matches = (await invoke("search_in_project", {
          projectDir,
          query: q.trim(),
          caseSensitive: options.caseSensitive,
          filePattern: options.filePattern.trim() || null,
          maxResults: 500,
        })) as SearchMatch[];
        setResults(matches);
      } catch {
        setResults([]);
      } finally {
        setLoading(false);
      }
    },
    [projectDir, options.caseSensitive, options.filePattern]
  );

  const updateQuery = useCallback(
    (val: string) => {
      setQuery(val);
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (val.trim().length >= 2) {
        debounceRef.current = setTimeout(() => doSearch(val), 400);
      } else {
        setResults([]);
        setSearched(false);
      }
    },
    [doSearch]
  );

  const searchNow = useCallback(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    doSearch(query);
  }, [query, doSearch]);

  const grouped = useMemo<FileGroup[]>(() => {
    const map = new Map<string, SearchMatch[]>();
    for (const m of results) {
      const arr = map.get(m.file) ?? [];
      arr.push(m);
      map.set(m.file, arr);
    }
    return Array.from(map.entries()).map(([file, matches]) => ({ file, matches }));
  }, [results]);

  return {
    query,
    updateQuery,
    searchNow,
    options,
    setOption,
    results,
    grouped,
    loading,
    searched,
    matchCount: results.length,
    fileCount: grouped.length,
  };
}
