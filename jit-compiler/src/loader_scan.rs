use crate::ast::Model;
use crate::diag::ParseErrorInfo;
use crate::loader_compat::{early_compat, late_compat, EarlyCompat, LateCompat};
use crate::parser;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use super::{LoadError, ModelLoader};

impl ModelLoader {
    pub(super) fn load_model_impl(&mut self, name: &str, silent: bool) -> Result<Arc<Model>, LoadError> {
        fn trace_enabled() -> bool {
            static ENABLED: OnceLock<bool> = OnceLock::new();
            *ENABLED.get_or_init(|| {
                std::env::var("RUSTMODLICA_LOAD_TRACE")
                    .ok()
                    .map(|v| {
                        let v = v.trim();
                        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
                    })
                    .unwrap_or(false)
            })
        }
        if trace_enabled() && !silent {
            eprintln!("[load-trace] {}", name);
        }
        if let Some(r) = self.try_example_templates_alias(name) {
            return r;
        }
        match early_compat(name) {
            EarlyCompat::None => {}
            EarlyCompat::Hard(targets) => {
                let mut last_err: Option<LoadError> = None;
                for t in targets {
                    match self.load_model_impl(&t, true) {
                        Ok(arc) => {
                            self.cache_compat_alias(name, &t, &arc);
                            return Ok(arc);
                        }
                        Err(e) => last_err = Some(e),
                    }
                }
                if let Some(e) = last_err {
                    return Err(e);
                }
            }
            EarlyCompat::Soft(targets) => {
                for t in targets {
                    if let Ok(arc) = self.load_model_impl(&t, true) {
                        self.cache_compat_alias(name, &t, &arc);
                        return Ok(arc);
                    }
                }
            }
        }
        if let Some(arc) = self.loaded_models.get(name) {
            return Ok(Arc::clone(arc));
        }
        if let Some(ep) = self.absent_name_epoch.get(name) {
            if *ep == self.registration_epoch() {
                if !silent {
                    // Keep negative cache-dependency recording identical to the
                    // non-memoized path for non-silent (real) references.
                    let rel = name.replace('.', "/");
                    let libs: Vec<PathBuf> = self.library_paths.clone();
                    for lib in &libs {
                        self.probed_absent
                            .insert(lib.join(format!("{}/package.mo", rel)));
                        self.probed_absent.insert(lib.join(format!("{}.mo", rel)));
                    }
                }
                return Err(LoadError::NotFound(name.to_string()));
            }
        }
        crate::query_db::perf_record_add("loader_cache_miss_calls", 1);
        if self.currently_loading.contains(name) {
            return Err(LoadError::RecursiveLoad(name.to_string()));
        }
        let name_key = name.to_string();
        self.currently_loading.insert(name_key.clone());

        let result = (|| {
            let relative_path = name.replace('.', "/");
        // Modelica libraries commonly represent packages as directories containing `package.mo`,
        // even when the qualified name contains '.'.
        // Always try both forms to support both layouts:
        // - <rel>.mo
        // - <rel>/package.mo
        let filenames: Vec<std::path::PathBuf> = vec![
            PathBuf::from(format!("{}/package.mo", relative_path)),
            PathBuf::from(format!("{}.mo", relative_path)),
        ];

        for lib_path in &self.library_paths {
            for filename in &filenames {
                let full_path = lib_path.join(filename);
                if full_path.exists() {
                    if !self.quiet && !silent {
                        println!(
                            "{}",
                            crate::i18n::msg(
                                "loading_dependency",
                                &[&full_path.display().to_string() as &dyn std::fmt::Display]
                            )
                        );
                    }
                    let content = fs::read_to_string(&full_path)
                        .map_err(|e| LoadError::Io(name.to_string(), e))?;
                    match parser::parse_all(&content) {
                        Ok(items) => {
                            if items.is_empty() {
                                return Err(LoadError::NotFound(name.to_string()));
                            }
                            let mut selected_idx = 0usize;
                            let short_name = name.rsplit('.').next().unwrap_or(name);
                            if let Some((idx, _)) = items
                                .iter()
                                .enumerate()
                                .find(|(_, it)| Self::item_name(it) == short_name)
                            {
                                selected_idx = idx;
                            }
                            let mut model = Self::item_to_model(items[selected_idx].clone());
                            // Inherit imports from parent package if available.
                            if let Some((prefix, _)) = name.rsplit_once('.') {
                                if let Ok(parent) = self.load_model_impl(prefix, true) {
                                    if !parent.imports.is_empty() {
                                        for (a, q) in &parent.imports {
                                            if !model.imports.iter().any(|(aa, qq)| aa == a && qq == q) {
                                                model.imports.push((a.clone(), q.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                            let arc = Arc::new(model);
                            self.loaded_models
                                .insert(name.to_string(), Arc::clone(&arc));
                            self.loaded_paths
                                .insert(name.to_string(), full_path.clone());
                            self.register_inner_classes(name, arc.as_ref());
                            // Also register sibling top-level definitions from same file.
                            if let Some((prefix, _)) = name.rsplit_once('.') {
                                for item in items {
                                    let sibling_name = Self::item_name(&item).to_string();
                                    if sibling_name == short_name {
                                        continue;
                                    }
                                    let sibling_model = Self::item_to_model(item);
                                    let fq = format!("{}.{}", prefix, sibling_name);
                                    self.loaded_models
                                        .entry(fq.clone())
                                        .or_insert_with(|| Arc::new(sibling_model.clone()));
                                    self.loaded_paths
                                        .entry(fq)
                                        .or_insert_with(|| full_path.clone());
                                    self.loaded_models
                                        .entry(sibling_name.clone())
                                        .or_insert_with(|| Arc::new(sibling_model));
                                    self.loaded_paths
                                        .entry(sibling_name)
                                        .or_insert_with(|| full_path.clone());
                                }
                            } else {
                                for item in items {
                                    let sibling_name = Self::item_name(&item).to_string();
                                    if sibling_name == short_name {
                                        continue;
                                    }
                                    self.loaded_models
                                        .entry(sibling_name.clone())
                                        .or_insert_with(|| Arc::new(Self::item_to_model(item)));
                                    self.loaded_paths
                                        .entry(sibling_name)
                                        .or_insert_with(|| full_path.clone());
                                }
                            }
                            return Ok(arc);
                        }
                        Err(e) => {
                            let (line, column) = crate::diag::line_col_from_pest(&e.line_col);
                            let path_str = full_path.display().to_string();
                            let message =
                                crate::diag::short_message_from_pest_string(&e.to_string());
                            let info = ParseErrorInfo {
                                path: path_str,
                                source: content.clone(),
                                line,
                                column,
                                message,
                            };
                            return Err(LoadError::ParseFailedAt(info));
                        }
                    }
                }
            }
        }

        // If direct file lookup failed, try resolving as an inner class of the nearest parent.
        // This matches common Modelica library layouts where a package is defined in a single
        // `<Package>.mo` (e.g. `Modelica/Blocks/Sources.mo`) and contains many inner classes.
        if let Some((prefix, suffix)) = name.rsplit_once('.') {
            // Do not `?` here: e.g. `TestLib.sumArrayExternal` may resolve via directory scan below
            // when there is no loadable `TestLib.mo` package file (flat library folder layout).
            if let Ok(base) = self.load_model_impl(prefix, silent) {
                // `load_model_impl(prefix, ..)` registers all inner classes under `prefix.*` into
                // `loaded_models`. Prefer returning the already-registered Arc to avoid deep cloning
                // large inner-class trees (which can cause stack overflow).
                if let Some(arc) = self.loaded_models.get(name) {
                    return Ok(Arc::clone(arc));
                }
                let inner = base.find_inner_class(suffix).cloned();
                if let Some(m) = inner {
                    if !self.quiet && !silent {
                        eprintln!("Resolved inner class: {} via {}", name, prefix);
                    }
                    let mut m = m;
                    if !base.imports.is_empty() {
                        // Inherit imports from parent package/class so that short names like
                        // `Interfaces.SISO` work after `import Modelica.Blocks.Interfaces;`.
                        // Keep child's own imports as well.
                        for (a, q) in &base.imports {
                            if !m.imports.iter().any(|(aa, qq)| aa == a && qq == q) {
                                m.imports.push((a.clone(), q.clone()));
                            }
                        }
                    }
                    let arc = Arc::new(m);
                    self.loaded_models
                        .insert(name.to_string(), Arc::clone(&arc));
                    self.loaded_paths.insert(
                        name.to_string(),
                        self.loaded_paths
                            .get(prefix)
                            .cloned()
                            .unwrap_or_else(|| PathBuf::from(prefix)),
                    );
                    self.register_inner_classes(name, arc.as_ref());
                    return Ok(arc);
                }
            }
        }

        // Multi-top-level file fallback:
        // If requested class/function is not found by path, scan already-loaded source files
        // for a sibling top-level definition with matching short name.
        {
            let fallback_scan_t0 = std::time::Instant::now();
            crate::query_db::perf_record_add("loader_multi_top_scan_count", 1);
            let want_short = name.rsplit_once('.').map(|(_, s)| s).unwrap_or(name);
            let mut seen = std::collections::HashSet::new();
            // For a qualified request `Pkg.Sub.Short`, only scan files that are already
            // registered under the same parent scope (`Pkg.Sub` itself or a direct child
            // `Pkg.Sub.X`). Scanning every loaded file matches unrelated classes that merely
            // share the short name (e.g. `FluxTubes.SI.Reluctance` must not resolve to the
            // partial icon `FluxTubes.Icons.Reluctance`).
            let paths: Vec<PathBuf> = match name.rsplit_once('.') {
                Some((parent, _)) => {
                    let dot_parent = format!("{}.", parent);
                    self.loaded_paths
                        .iter()
                        .filter(|(k, _)| {
                            k.as_str() == parent
                                || (k.starts_with(&dot_parent)
                                    && !k[dot_parent.len()..].contains('.'))
                        })
                        .map(|(_, v)| v.clone())
                        .collect()
                }
                None => self.loaded_paths.values().cloned().collect(),
            };
            let mut scan_result = None;
            for p in paths {
                if !seen.insert(p.clone()) {
                    continue;
                }
                crate::query_db::perf_record_add("loader_multi_top_scan_files", 1);
                let Some(items) = self.parse_file_items_cached(&p) else {
                    continue;
                };
                if let Some(item) = items
                    .iter()
                    .find(|it| Self::item_name(it) == want_short)
                    .cloned()
                {
                    let arc = Arc::new(Self::item_to_model(item));
                    self.loaded_models
                        .insert(name.to_string(), Arc::clone(&arc));
                    self.loaded_paths.insert(name.to_string(), p.clone());
                    self.register_inner_classes(name, arc.as_ref());
                    scan_result = Some(arc);
                    break;
                }
            }
            crate::query_db::perf_record_us(
                "loader_multi_top_scan_us",
                fallback_scan_t0.elapsed().as_micros() as u64,
            );
            if let Some(arc) = scan_result {
                return Ok(arc);
            }
        }

        // Package directory scan: e.g. `TestLib.sumArrayExternal` has no `TestLib/sumArrayExternal.mo`,
        // but `TestLib/ExtFuncArrayArg.mo` defines `function sumArrayExternal`.
        if let Some((pkg_rel, short)) = name.rsplit_once('.') {
            struct DirScanPerfGuard(std::time::Instant);
            impl Drop for DirScanPerfGuard {
                fn drop(&mut self) {
                    crate::query_db::perf_record_us(
                        "loader_dir_scan_us",
                        self.0.elapsed().as_micros() as u64,
                    );
                }
            }
            let _dir_scan_guard = DirScanPerfGuard(std::time::Instant::now());
            crate::query_db::perf_record_add("loader_dir_scan_count", 1);
            let rel_dir: PathBuf = pkg_rel.replace('.', std::path::MAIN_SEPARATOR_STR).into();
            let libs: Vec<PathBuf> = self.library_paths.clone();
            for lib_path in &libs {
                let nested = lib_path.join(&rel_dir);
                // If `lib_path` already points at a package directory (e.g. CLI `--lib-path=.../TestLib`),
                // `nested` may be `.../TestLib/TestLib` and miss; fall back to scanning `lib_path`.
                let scan_dir = if nested.is_dir() {
                    nested
                } else {
                    lib_path.clone()
                };
                if scan_dir.is_dir() {
                    let dir = scan_dir;
                    let entries = match fs::read_dir(&dir) {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    for ent in entries.filter_map(|e| e.ok()) {
                        let path = ent.path();
                        if !path.extension().map(|e| e == "mo").unwrap_or(false) {
                            continue;
                        }
                        let Some(items) = self.parse_file_items_cached(&path) else {
                            continue;
                        };
                        let Some((idx, _)) = items
                            .iter()
                            .enumerate()
                            .find(|(_, it)| Self::item_name(it) == short)
                        else {
                            continue;
                        };
                        let matched = items[idx].clone();
                        let arc = Arc::new(Self::item_to_model(matched));
                        self.loaded_models
                            .insert(name.to_string(), Arc::clone(&arc));
                        self.loaded_paths.insert(name.to_string(), path.clone());
                        self.register_inner_classes(name, arc.as_ref());
                        for item in items.iter() {
                            let sibling_name = Self::item_name(item).to_string();
                            if sibling_name == short {
                                continue;
                            }
                            let sibling_model = Self::item_to_model(item.clone());
                            let fq = format!("{}.{}", pkg_rel, sibling_name);
                            self.loaded_models
                                .entry(fq.clone())
                                .or_insert_with(|| Arc::new(sibling_model.clone()));
                            self.loaded_paths
                                .entry(fq)
                                .or_insert_with(|| path.clone());
                            self.loaded_models
                                .entry(sibling_name.clone())
                                .or_insert_with(|| Arc::new(sibling_model.clone()));
                            self.loaded_paths
                                .entry(sibling_name)
                                .or_insert_with(|| path.clone());
                        }
                        return Ok(arc);
                    }
                }
            }
        }

        match late_compat(name) {
            LateCompat::None => {}
            LateCompat::Soft(targets) => {
                for t in targets {
                    if let Ok(arc) = self.load_model_impl(&t, true) {
                        self.cache_compat_alias(name, &t, &arc);
                        return Ok(arc);
                    }
                }
            }
        }

        if !self.quiet && !silent {
            eprintln!(
                "{}",
                crate::i18n::msg("could_not_find_model", &[&name as &dyn std::fmt::Display])
            );
        }
        Err(LoadError::NotFound(name.to_string()))
        })();

        self.currently_loading.remove(&name_key);
        if matches!(result, Err(LoadError::NotFound(_))) {
            // Negative name cache: valid until any new registration changes the
            // epoch (new file/model registered could change resolution).
            let epoch = self.registration_epoch();
            self.absent_name_epoch.insert(name_key.clone(), epoch);
        }
        if !silent && matches!(result, Err(LoadError::NotFound(_))) {
            // Record the direct-lookup candidate paths for this unresolved name as
            // negative dependencies: creating any of them would change resolution.
            // Only for non-silent (real) references — silent probes are the many
            // speculative short-name lookups and would bloat the dep set.
            let rel = name.replace('.', "/");
            let libs: Vec<PathBuf> = self.library_paths.clone();
            for lib in &libs {
                self.probed_absent
                    .insert(lib.join(format!("{}/package.mo", rel)));
                self.probed_absent.insert(lib.join(format!("{}.mo", rel)));
            }
        }
        result
    }

    /// Load a model from source code in memory (for IDE / single-file compile).
    /// Registers the parsed model under `model_name` and uses a virtual path for diagnostics.
    pub fn load_model_from_source(
        &mut self,
        model_name: &str,
        code: &str,
    ) -> Result<Arc<Model>, LoadError> {
        if let Some(arc) = self.loaded_models.get(model_name) {
            return Ok(Arc::clone(arc));
        }
        let items = parser::parse_all(code).map_err(|e| {
            let (line, column) = crate::diag::line_col_from_pest(&e.line_col);
            let message = crate::diag::short_message_from_pest_string(&e.to_string());
            LoadError::ParseFailedAt(ParseErrorInfo {
                path: format!("<{}>", model_name),
                source: code.to_string(),
                line,
                column,
                message,
            })
        })?;
        if items.is_empty() {
            return Err(LoadError::NotFound(model_name.to_string()));
        }
        let mut selected_idx = 0usize;
        let short_name = model_name.rsplit('.').next().unwrap_or(model_name);
        if let Some((idx, _)) = items
            .iter()
            .enumerate()
            .find(|(_, it)| Self::item_name(it) == short_name)
        {
            selected_idx = idx;
        }
        let model = Self::item_to_model(items[selected_idx].clone());
        let arc = Arc::new(model);
        self.loaded_models
            .insert(model_name.to_string(), Arc::clone(&arc));
        self.loaded_paths.insert(
            model_name.to_string(),
            PathBuf::from(format!("<{}>", model_name)),
        );
        self.register_inner_classes(model_name, arc.as_ref());
        if let Some((prefix, _)) = model_name.rsplit_once('.') {
            for item in items {
                let sibling = Self::item_name(&item).to_string();
                if sibling == short_name {
                    continue;
                }
                let sibling_model = Self::item_to_model(item);
                let fq = format!("{}.{}", prefix, sibling);
                self.loaded_models
                    .entry(fq.clone())
                    .or_insert_with(|| Arc::new(sibling_model.clone()));
                self.loaded_paths
                    .entry(fq)
                    .or_insert_with(|| PathBuf::from(format!("<{}>", model_name)));
                self.loaded_models
                    .entry(sibling.clone())
                    .or_insert_with(|| Arc::new(sibling_model));
                self.loaded_paths
                    .entry(sibling)
                    .or_insert_with(|| PathBuf::from(format!("<{}>", model_name)));
            }
        } else {
            for item in items {
                let sibling = Self::item_name(&item).to_string();
                if sibling == short_name {
                    continue;
                }
                self.loaded_models
                    .entry(sibling.clone())
                    .or_insert_with(|| Arc::new(Self::item_to_model(item)));
                self.loaded_paths
                    .entry(sibling)
                    .or_insert_with(|| PathBuf::from(format!("<{}>", model_name)));
            }
        }
        Ok(arc)
    }

    fn register_inner_classes(&mut self, prefix: &str, model: &Model) {
        // Iterative to avoid stack overflow on large package trees (e.g. Modelica.Media.*).
        // Also avoid deep cloning `inner_classes` trees: we register each inner class as a shallow
        // model (without its own `inner_classes`) and traverse children by reference.
        let mut stack: Vec<(String, &Model)> = model
            .inner_classes
            .iter()
            .map(|m| (prefix.to_string(), m))
            .collect();

        while let Some((parent_prefix, inner)) = stack.pop() {
            let full_name = format!("{}.{}", parent_prefix, inner.name);
            if self.loaded_models.contains_key(&full_name) {
                continue;
            }
            let arc = Arc::new(Self::clone_model_shallow(inner));
            self.loaded_models
                .insert(full_name.clone(), Arc::clone(&arc));
            let path = self
                .loaded_paths
                .get(&parent_prefix)
                .cloned()
                .unwrap_or_else(|| PathBuf::from(&parent_prefix));
            self.loaded_paths.insert(full_name.clone(), path);

            for child in &inner.inner_classes {
                stack.push((full_name.clone(), child));
            }
        }
    }
}
