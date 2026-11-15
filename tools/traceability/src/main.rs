use proc_macro2::Span;
use quote::ToTokens;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};
use syn::{
    Expr, Item, ItemFn, Lit, Token,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Target {
    package_root: PathBuf,
    package: String,
    kind: String,
    name: String,
    source: PathBuf,
}
impl Target {
    fn value(&self) -> Value {
        if self.kind == "lib" {
            json!({"kind": "lib"})
        } else {
            json!({"kind": self.kind, "name": self.name})
        }
    }
}
#[derive(Clone)]
struct Context {
    target: Target,
    file: PathBuf,
    modules: Vec<String>,
    module_dir: PathBuf,
    path_dir: PathBuf,
    unsupported_cfg: bool,
}
struct Declaration {
    feature: String,
    contract: Option<String>,
    value: Value,
    context: Context,
}
struct Test {
    context: Context,
    function: String,
    line: usize,
    classification: Option<(String, Value)>,
    span: Span,
    outer: Option<Span>,
    body: Span,
    tokens: String,
}
impl Test {
    fn name(&self) -> String {
        self.context
            .modules
            .iter()
            .chain(std::iter::once(&self.function))
            .cloned()
            .collect::<Vec<_>>()
            .join("::")
    }
}
#[derive(Default)]
struct Collector {
    scan: bool,
    root: PathBuf,
    reached_targets: BTreeSet<(Target, PathBuf)>,
    declarations: Vec<Declaration>,
    tests: Vec<Test>,
    reached: BTreeSet<PathBuf>,
    macros: Vec<(Context, String)>,
    contexts: Vec<Context>,
}
fn fail(context: &Context, span: Span, message: impl std::fmt::Display) -> String {
    format!(
        "{}:{}: {message}",
        context.file.display(),
        span.start().line
    )
}
fn literal(expr: &Expr) -> Result<Value> {
    match expr {
        Expr::Lit(v) => match &v.lit {
            Lit::Str(s) => Ok(json!(s.value())),
            _ => Err("only string literals are supported".into()),
        },
        Expr::Reference(v) => literal(&v.expr),
        Expr::Array(v) => v
            .elems
            .iter()
            .map(literal)
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Expr::Struct(v) => {
            if v.rest.is_some() {
                return Err("struct updates are unsupported".into());
            }
            let kind = v.path.segments.last().unwrap().ident.to_string();
            let expected: &[&str] = match kind.as_str() {
                "Feature" => &[
                    "title",
                    "source",
                    "flow",
                    "guarantees",
                    "limitations",
                    "entrypoints",
                    "surfaces",
                ],
                "EntryPoint" => &["name", "source"],
                "Guarantee" => &["id", "statement", "contracts"],
                "Contract" => &["statement", "source", "gap"],
                "Source" => &["path", "needle"],
                "Case" => &["scope", "reason", "relationship", "also"],
                "Evidence" => &["feature", "contract", "scope", "relationship", "reason"],
                "Example" => &["kind", "reason"],
                "OpenApi" => &["package", "example", "operation"],
                "Grpc" => &["path", "service", "operation"],
                _ => return Err(format!("unsupported semantic type {kind}")),
            };
            let mut result = Map::new();
            for field in &v.fields {
                let key = match &field.member {
                    syn::Member::Named(i) => i.to_string(),
                    _ => return Err("named fields required".into()),
                };
                if result.insert(key, literal(&field.expr)?).is_some() {
                    return Err("duplicate field".into());
                }
            }
            if result.keys().map(String::as_str).collect::<BTreeSet<_>>()
                != expected.iter().copied().collect()
            {
                return Err(format!("{kind} requires exactly {expected:?}"));
            }
            if matches!(kind.as_str(), "OpenApi" | "Grpc") {
                result.insert("kind".into(), json!(kind));
            }
            Ok(Value::Object(result))
        }
        Expr::Path(v) => {
            let name = v.path.segments.last().unwrap().ident.to_string();
            match name.as_str() {
                "None" => Ok(Value::Null),
                "Asserts" => Ok(json!("asserts")),
                "Exercises" => Ok(json!("exercises")),
                "Unreviewed" => Ok(json!("unreviewed")),
                "OpenApi" => Ok(json!("OpenApi")),
                "Program" => Ok(json!("Program")),
                _ => Err(format!("unsupported semantic variant {name}")),
            }
        }
        Expr::Call(v) => {
            let Expr::Path(path) = &*v.func else {
                return Err("literal constructor required".into());
            };
            let kind = path.path.segments.last().unwrap().ident.to_string();
            let args = v.args.iter().map(literal).collect::<Result<Vec<_>>>()?;
            match (kind.as_str(), args.len()) {
                ("Some", 1) => Ok(args[0].clone()),
                _ => Err(format!("unsupported semantic constructor {kind}")),
            }
        }
        _ => Err(
            "semantic declarations require literal structs, arrays, strings and enum variants"
                .into(),
        ),
    }
}
struct CaseInput {
    value: Expr,
    function: ItemFn,
}
impl Parse for CaseInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let value = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(Self {
            value,
            function: input.parse()?,
        })
    }
}
struct InfrastructureInput {
    category: syn::LitStr,
    reason: syn::LitStr,
    function: ItemFn,
}
impl Parse for InfrastructureInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let category = input.parse()?;
        input.parse::<Token![,]>()?;
        let reason = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(Self {
            category,
            reason,
            function: input.parse()?,
        })
    }
}
struct HelperInput {
    reason: syn::LitStr,
    function: ItemFn,
}
impl Parse for HelperInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let reason = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(Self {
            reason,
            function: input.parse()?,
        })
    }
}
fn owner(file: &Path, package_root: &Path) -> Option<(String, Option<String>)> {
    let relative = file.strip_prefix(package_root).ok()?;
    let components = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()?;
    if components.len() < 4 || !matches!(components[0], "src" | "tests") || components[1] != "trace"
    {
        return None;
    }
    Some((
        components[2].into(),
        if components.len() >= 5 {
            Some(components[3].into())
        } else {
            None
        },
    ))
}
fn cfg_supported(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().all(|a| {
        if a.path().is_ident("cfg_attr") {
            return false;
        }
        if !a.path().is_ident("cfg") {
            return true;
        }
        matches!(&a.meta, syn::Meta::List(v) if v.tokens.to_string() == "test")
    })
}
fn is_test(function: &ItemFn) -> Result<bool> {
    let mut test = false;
    for attr in &function.attrs {
        if attr
            .path()
            .segments
            .last()
            .is_some_and(|s| s.ident == "test")
        {
            let names = attr
                .path()
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>();
            if names != ["test"] && names != ["tokio", "test"] {
                return Err("unsupported test attribute".into());
            }
            test = true;
        }
    }
    Ok(test)
}
impl Collector {
    fn file(&mut self, context: Context, stack: &mut BTreeSet<PathBuf>) -> Result<()> {
        let canonical = context
            .file
            .canonicalize()
            .map_err(|e| format!("{}: {e}", context.file.display()))?;
        if !stack.insert(canonical.clone()) {
            return Err(format!("cyclic Rust module {}", context.file.display()));
        }
        if !canonical.starts_with(&self.root) {
            return Err(format!(
                "Rust source outside repository: {}",
                canonical.display()
            ));
        }
        if !self
            .reached_targets
            .insert((context.target.clone(), canonical.clone()))
        {
            return Err(format!(
                "duplicate Rust source in target: {}",
                canonical.display()
            ));
        }
        self.reached.insert(canonical.clone());
        let content = fs::read_to_string(&context.file).map_err(|e| e.to_string())?;
        let ast = syn::parse_file(&content).map_err(|e| fail(&context, e.span(), e))?;
        let mut context = context;
        context.file = canonical.clone();
        context.unsupported_cfg |= !cfg_supported(&ast.attrs);
        self.contexts.push(context.clone());
        self.items(&ast.items, &context, stack)?;
        stack.remove(&canonical);
        Ok(())
    }
    fn function(
        &mut self,
        function: &ItemFn,
        context: &Context,
        classification: Option<(String, Value)>,
        outer: Option<Span>,
    ) -> Result<()> {
        if is_test(function).map_err(|e| fail(context, function.span(), e))? {
            let mut context = context.clone();
            context.unsupported_cfg |= !cfg_supported(&function.attrs);
            if context.unsupported_cfg
                && owner(&context.file, &context.target.package_root).is_none()
            {
                return Ok(());
            }
            self.tests.push(Test {
                context,
                function: function.sig.ident.to_string(),
                line: function.sig.fn_token.span.start().line,
                classification,
                span: function.span(),
                outer,
                body: function.block.span(),
                tokens: function.to_token_stream().to_string(),
            });
        } else if classification.is_some() {
            return Err(fail(
                context,
                function.span(),
                "classification wrapper requires a test function",
            ));
        }
        Ok(())
    }
    fn items(
        &mut self,
        items: &[Item],
        context: &Context,
        stack: &mut BTreeSet<PathBuf>,
    ) -> Result<()> {
        for item in items {
            match item {
                Item::Mod(module) => {
                    if module.ident == "trace_support" {
                        continue;
                    }
                    let mut child = context.clone();
                    child.modules.push(module.ident.to_string());
                    child.unsupported_cfg |= !cfg_supported(&module.attrs);
                    if child.unsupported_cfg {
                        if owner(&context.file, &context.target.package_root).is_some() {
                            return Err(fail(
                                context,
                                module.span(),
                                "unsupported cfg inside trace directory",
                            ));
                        }
                        self.contexts.push(child);
                        continue;
                    }
                    let explicit = module
                        .attrs
                        .iter()
                        .find(|a| a.path().is_ident("path"))
                        .map(|a| -> Result<String> {
                            let syn::Meta::NameValue(v) = &a.meta else {
                                return Err("literal module path required".into());
                            };
                            let Value::String(path) = literal(&v.value)? else {
                                return Err("literal module path required".into());
                            };
                            Ok(path)
                        })
                        .transpose()?;
                    if let Some((_, items)) = &module.content {
                        if explicit.is_some() {
                            return Err(fail(
                                context,
                                module.span(),
                                "path on inline module is unsupported",
                            ));
                        }
                        child.module_dir = context.module_dir.join(module.ident.to_string());
                        child.path_dir = child.module_dir.clone();
                        self.contexts.push(child.clone());
                        self.items(items, &child, stack)?;
                    } else {
                        child.file = if let Some(path) = &explicit {
                            context.path_dir.join(path)
                        } else {
                            let flat = context.module_dir.join(format!("{}.rs", module.ident));
                            let nested = context
                                .module_dir
                                .join(module.ident.to_string())
                                .join("mod.rs");
                            match (flat.is_file(), nested.is_file()) {
                                (true, false) => flat,
                                (false, true) => nested,
                                _ => {
                                    return Err(fail(
                                        context,
                                        module.span(),
                                        "module must resolve to exactly one Rust file",
                                    ));
                                }
                            }
                        };
                        child.path_dir = child.file.parent().unwrap().to_owned();
                        child.module_dir = if explicit.is_some() {
                            child.path_dir.clone()
                        } else {
                            module_directory(&child.file)
                        };
                        self.file(child, stack)?;
                    }
                }
                Item::Fn(function) => self.function(function, context, None, None)?,
                Item::Macro(item) => {
                    let name = item.mac.path.segments.last().unwrap().ident.to_string();
                    match name.as_str() {
                        "feature" | "contract" if self.scan => (),
                        "feature" | "contract" => {
                            if context.unsupported_cfg || !cfg_supported(&item.attrs) {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "unsupported cfg on trace declaration",
                                ));
                            }
                            let (feature, contract) = owner(&context.file, &context.target.package_root).ok_or_else(|| fail(context, item.span(), "declaration must live under src/trace/<feature> or tests/trace/<feature>"))?;
                            let expected = if name == "feature" {
                                "feature.rs"
                            } else {
                                "contract.rs"
                            };
                            if context.file.file_name().unwrap() != expected
                                || (name == "contract") != contract.is_some()
                            {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "declaration path does not match ownership",
                                ));
                            }
                            let expr = syn::parse2::<Expr>(item.mac.tokens.clone())
                                .map_err(|e| fail(context, e.span(), e))?;
                            let value =
                                literal(&expr).map_err(|e| fail(context, expr.span(), e))?;
                            self.declarations.push(Declaration {
                                feature,
                                contract,
                                value,
                                context: context.clone(),
                            });
                        }
                        "case" => {
                            let input = syn::parse2::<CaseInput>(item.mac.tokens.clone())
                                .map_err(|e| fail(context, e.span(), e))?;
                            if !item.attrs.is_empty() {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "put test attributes inside the transparent wrapper",
                                ));
                            }
                            let value = if self.scan {
                                Value::Null
                            } else {
                                literal(&input.value)
                                    .map_err(|e| fail(context, input.value.span(), e))?
                            };
                            self.function(
                                &input.function,
                                context,
                                Some((name, value)),
                                Some(item.span()),
                            )?;
                        }
                        "outside" if self.scan => {
                            let input = syn::parse2::<InfrastructureInput>(item.mac.tokens.clone())
                                .map_err(|e| fail(context, e.span(), e))?;
                            self.function(
                                &input.function,
                                context,
                                Some((name, Value::Null)),
                                Some(item.span()),
                            )?;
                        }
                        "infrastructure" => {
                            let input = syn::parse2::<InfrastructureInput>(item.mac.tokens.clone())
                                .map_err(|e| fail(context, e.span(), e))?;
                            if !item.attrs.is_empty() {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "put attributes inside the wrapper",
                                ));
                            }
                            self.function(&input.function, context, Some((name, json!({"category": input.category.value(), "reason": input.reason.value()}))), Some(item.span()))?;
                        }
                        "helper" => {
                            let input = syn::parse2::<HelperInput>(item.mac.tokens.clone())
                                .map_err(|e| fail(context, e.span(), e))?;
                            if !item.attrs.is_empty() {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "put attributes inside the wrapper",
                                ));
                            }
                            self.function(
                                &input.function,
                                context,
                                Some((name, json!({"reason": input.reason.value()}))),
                                Some(item.span()),
                            )?;
                        }
                        "include" => {
                            if context.unsupported_cfg || !cfg_supported(&item.attrs) {
                                if owner(&context.file, &context.target.package_root).is_some() {
                                    return Err(fail(
                                        context,
                                        item.span(),
                                        "unsupported cfg inside trace directory",
                                    ));
                                }
                                continue;
                            }
                            let path = syn::parse2::<syn::LitStr>(item.mac.tokens.clone())
                                .map_err(|_| {
                                    fail(
                                        context,
                                        item.span(),
                                        "include! requires a literal Rust source path",
                                    )
                                })?
                                .value();
                            if Path::new(&path).is_absolute() {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "include! path must be relative",
                                ));
                            }
                            let mut child = context.clone();
                            child.file = context.file.parent().unwrap().join(path);
                            child.module_dir = child.file.parent().unwrap().to_owned();
                            child.path_dir = child.module_dir.clone();
                            self.file(child, stack)?;
                        }
                        _ => self.macros.push((context.clone(), name)),
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}
fn module_directory(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap();
    if path.file_name().unwrap() == "mod.rs" {
        parent.to_path_buf()
    } else {
        parent.join(path.file_stem().unwrap())
    }
}
fn rust_files(path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_symlink() {
            return Err(format!(
                "trace directories cannot contain symlinks: {}",
                path.display()
            ));
        }
        if path.is_dir() {
            rust_files(&path, files)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.insert(path.canonicalize().map_err(|e| e.to_string())?);
        }
    }
    Ok(())
}
fn strings(value: &Value) -> Result<Vec<&str>> {
    value
        .as_array()
        .ok_or("expected array")?
        .iter()
        .map(|v| v.as_str().ok_or_else(|| "expected string".into()))
        .collect()
}
fn nonempty(value: &Value) -> Result<&str> {
    value
        .as_str()
        .filter(|s| !s.trim().is_empty() && !s.contains(['\n', '\r']))
        .ok_or_else(|| "expected nonempty single-line string".into())
}
fn workspace(root: &Path) -> Result<Value> {
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
fn workspace_targets(metadata: &Value) -> Result<(Vec<Target>, Vec<Value>)> {
    let members = strings(&metadata["workspace_members"])?;
    let mut targets = Vec::new();
    let mut excluded = Vec::new();
    for package in metadata["packages"].as_array().ok_or("packages missing")? {
        if !members.contains(&package["id"].as_str().ok_or("package id missing")?) {
            continue;
        }
        for target in package["targets"].as_array().ok_or("targets missing")? {
            let kinds = strings(&target["kind"])?;
            let kind = if kinds.iter().any(|kind| {
                matches!(
                    *kind,
                    "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
                )
            }) {
                "lib"
            } else if kinds.contains(&"bin") {
                "bin"
            } else if kinds.contains(&"test") {
                "test"
            } else {
                continue;
            };
            let target_value = Target {
                package_root: Path::new(nonempty(&package["manifest_path"])?)
                    .parent()
                    .unwrap()
                    .canonicalize()
                    .map_err(|e| e.to_string())?,
                package: nonempty(&package["name"])?.into(),
                kind: kind.into(),
                name: nonempty(&target["name"])?.into(),
                source: nonempty(&target["src_path"])?.into(),
            };
            if target["test"] == false {
                excluded.push(json!({"package":target_value.package,"target":target_value.value(),"reason":"Cargo test = false excludes this target from the default test harness policy."}));
                continue;
            }
            if target["required-features"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
            {
                return Err(format!(
                    "{} {}: required-features targets are unsupported by the default-host/default-Cargo-feature inventory policy",
                    target_value.package, target_value.name
                ));
            }
            targets.push(target_value);
        }
    }
    targets.sort();
    Ok((targets, excluded))
}
fn test_id(test: &Test) -> String {
    format!(
        "test-{}-{}-{}-{}",
        test.context.target.package,
        test.context.target.kind,
        test.context.target.name,
        test.name()
    )
    .replace("::", "-")
    .replace('_', "-")
}
fn test_source(test: &Test, root: &Path) -> Result<Value> {
    let path = test
        .context
        .file
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "test source outside repository")?;
    let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let needle = source
        .lines()
        .nth(test.line - 1)
        .ok_or("test span outside source")?
        .trim();
    Ok(json!({"path":relative,"needle":needle,"line":test.line}))
}
fn example_declaration(root: &Path, package: &Value, target: &Value) -> Result<Value> {
    let source = Path::new(nonempty(&target["src_path"])?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let relative = source
        .strip_prefix(root)
        .map_err(|_| "example source outside repository")?;
    let content = fs::read_to_string(&source).map_err(|e| e.to_string())?;
    let ast = syn::parse_file(&content).map_err(|e| format!("{}: {e}", source.display()))?;
    let declarations = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Macro(item) if item.mac.path.segments.last().unwrap().ident == "example" => {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if declarations.len() != 1
        || !declarations[0].attrs.is_empty()
        || ast.attrs.iter().any(|attribute| {
            attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
        })
    {
        return Err(format!(
            "{}: each Cargo example requires exactly one top-level example! declaration without attributes",
            source.display()
        ));
    }
    let declaration = declarations[0];
    let expr = syn::parse2::<Expr>(declaration.mac.tokens.clone())
        .map_err(|e| format!("{}: {e}", source.display()))?;
    let value = literal(&expr).map_err(|e| format!("{}: {e}", source.display()))?;
    let kind = nonempty(&value["kind"])?;
    if !matches!(kind, "OpenApi" | "Program") {
        return Err(format!(
            "{}: unsupported example kind {kind}",
            source.display()
        ));
    }
    let reason = nonempty(&value["reason"])?;
    Ok(
        json!({"package":package["name"],"example":target["name"],"source":relative,"kind":kind,"reason":reason}),
    )
}
fn collect(root: &Path) -> Result<Value> {
    let metadata = workspace(root)?;
    let (targets, excluded_targets) = workspace_targets(&metadata)?;
    let members = strings(&metadata["workspace_members"])?;
    let mut files = BTreeSet::new();
    let mut packages = Vec::new();
    let mut examples = Vec::new();
    let mut feature_roots = BTreeMap::<String, Vec<PathBuf>>::new();
    for package in metadata["packages"].as_array().ok_or("packages missing")? {
        if !members.contains(&package["id"].as_str().ok_or("package id missing")?) {
            continue;
        }
        let manifest = PathBuf::from(nonempty(&package["manifest_path"])?);
        packages.push(json!({"name":package["name"],"path":manifest.parent().unwrap().strip_prefix(root).map_err(|_|"workspace package outside repository")?}));
        for target in package["targets"].as_array().ok_or("targets missing")? {
            if strings(&target["kind"])?.contains(&"example") {
                if target["required-features"]
                    .as_array()
                    .is_some_and(|features| !features.is_empty())
                {
                    return Err(format!(
                        "{} {}: required-features examples are unsupported by the default-host/default-Cargo-feature inventory policy",
                        package["name"], target["name"]
                    ));
                }
                examples.push(example_declaration(root, package, target)?);
            }
        }
        for convention in ["src/trace", "tests/trace"] {
            let trace = manifest.parent().unwrap().join(convention);
            if !trace.is_dir() {
                continue;
            }
            for entry in fs::read_dir(trace).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if !path.is_dir() || path.is_symlink() {
                    return Err(format!("feature directory expected: {}", path.display()));
                }
                let id = path.file_name().unwrap().to_string_lossy().into_owned();
                feature_roots.entry(id).or_default().push(path.clone());
                rust_files(&path, &mut files)?;
            }
        }
    }
    let mut collector = Collector {
        root: root.to_owned(),
        ..Collector::default()
    };
    for target in &targets {
        collector.file(
            Context {
                file: target.source.clone(),
                module_dir: target.source.parent().unwrap().to_owned(),
                path_dir: target.source.parent().unwrap().to_owned(),
                modules: vec![],
                target: target.clone(),
                unsupported_cfg: false,
            },
            &mut BTreeSet::new(),
        )?;
    }
    if let Some(orphan) = files.difference(&collector.reached).next() {
        return Err(format!("unreachable trace Rust file: {}", orphan.display()));
    }
    for context in &collector.contexts {
        if context.unsupported_cfg && owner(&context.file, &context.target.package_root).is_some() {
            return Err(format!(
                "{}: unsupported cfg inside trace directory",
                context.file.display()
            ));
        }
    }
    for (context, name) in &collector.macros {
        if owner(&context.file, &context.target.package_root).is_some() {
            return Err(format!(
                "{}: unsupported item macro {name} inside trace directory",
                context.file.display()
            ));
        }
    }
    let mut features = BTreeMap::new();
    let mut contracts = BTreeMap::<String, BTreeMap<String, Value>>::new();
    for declaration in &collector.declarations {
        if let Some(contract) = &declaration.contract {
            let mut value = declaration.value.clone();
            value["id"] = json!(contract);
            value["evidence"] = json!([]);
            if value["gap"].is_null() {
                value.as_object_mut().unwrap().remove("gap");
            }
            if contracts
                .entry(declaration.feature.clone())
                .or_default()
                .insert(contract.clone(), value)
                .is_some()
            {
                return Err(format!(
                    "duplicate contract {}/{contract}",
                    declaration.feature
                ));
            }
        } else if features
            .insert(declaration.feature.clone(), declaration.value.clone())
            .is_some()
        {
            return Err(format!("duplicate feature {}", declaration.feature));
        }
    }
    for feature in feature_roots.keys() {
        if !features.contains_key(feature) {
            return Err(format!(
                "feature directory {feature} requires exactly one reachable feature.rs declaration"
            ));
        }
    }
    let mut inventories = targets
        .iter()
        .cloned()
        .map(|target| (target, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut tests_by_feature = BTreeMap::<String, BTreeMap<String, Value>>::new();
    let mut classifications = Vec::new();
    collector
        .tests
        .sort_by_key(|test| (test.context.target.clone(), test.name()));
    let mut used_ids = BTreeSet::new();
    for test in &collector.tests {
        if test.context.unsupported_cfg {
            return Err(format!(
                "{}: unsupported cfg in classified test {}",
                test.context.file.display(),
                test.name()
            ));
        }
        let source = test_source(test, root)?;
        let identity = json!({"package":test.context.target.package,"target":test.context.target.value(),"name":test.name()});
        if !inventories
            .get_mut(&test.context.target)
            .unwrap()
            .insert(test.name())
        {
            return Err(format!("duplicate parsed test {}", test.name()));
        }
        let owned = owner(&test.context.file, &test.context.target.package_root);
        let class = test
            .classification
            .as_ref()
            .map(|(kind, _)| kind.as_str())
            .unwrap_or("plain");
        if matches!(class, "helper" | "infrastructure") {
            let semantics = &test.classification.as_ref().unwrap().1;
            nonempty(&semantics["reason"])?;
            if class == "infrastructure"
                && !matches!(
                    nonempty(&semantics["category"])?,
                    "sql-policy" | "test-support"
                )
            {
                return Err(format!(
                    "{}: unsupported infrastructure category",
                    test.name()
                ));
            }
            classifications.push(json!({"identity":identity,"source":source,"classification":class,"semantics":semantics}));
            continue;
        }
        let Some((feature, Some(contract))) = owned else {
            return Err(format!(
                "{}: unclassified test; move it into a declared contract directory or classify it explicitly",
                test.name()
            ));
        };
        if !features.contains_key(&feature)
            || !contracts
                .get(&feature)
                .is_some_and(|cs| cs.contains_key(&contract))
        {
            return Err(format!(
                "{}: unknown owning contract {feature}/{contract}",
                test.name()
            ));
        }
        let semantics = test.classification.as_ref().map(|(_,v)|v.clone()).unwrap_or_else(||json!({"scope":"Contract-directory membership only; runtime dependencies and assertions have not been reviewed.","reason":"Directory ownership declares this test belongs to the contract; execution of the implementation and strength of assertions require review.","relationship":"unreviewed","also":[]}));
        nonempty(&semantics["scope"])?;
        nonempty(&semantics["reason"])?;
        let base_id = test_id(test);
        let mut id = base_id.clone();
        let mut suffix = 1;
        while !used_ids.insert(id.clone()) {
            suffix += 1;
            id = format!("{base_id}-{suffix}");
        }
        let mut edges = vec![(
            feature.clone(),
            contract,
            semantics["scope"].clone(),
            semantics["relationship"].clone(),
            semantics["reason"].clone(),
        )];
        for edge in semantics["also"].as_array().ok_or("also array required")? {
            edges.push((
                nonempty(&edge["feature"])?.into(),
                nonempty(&edge["contract"])?.into(),
                edge["scope"].clone(),
                edge["relationship"].clone(),
                edge["reason"].clone(),
            ));
        }
        let mut scopes = BTreeMap::new();
        let mut destinations = BTreeSet::new();
        for (feature, contract, scope, relationship, reason) in edges {
            nonempty(&scope)?;
            nonempty(&reason)?;
            if !matches!(
                relationship.as_str(),
                Some("asserts" | "exercises" | "unreviewed")
            ) {
                return Err("invalid relationship".into());
            }
            if !destinations.insert((feature.clone(), contract.clone())) {
                return Err(format!(
                    "duplicate evidence edge {feature}/{contract} for {}",
                    test.name()
                ));
            }
            if scopes
                .insert(feature.clone(), scope.clone())
                .is_some_and(|prior| prior != scope)
            {
                return Err(format!(
                    "conflicting feature scopes for {feature}: {}",
                    test.name()
                ));
            }
            let destination = contracts
                .get_mut(&feature)
                .and_then(|cs| cs.get_mut(&contract))
                .ok_or_else(|| {
                    format!("unknown cross-feature/contract reference {feature}/{contract}")
                })?;
            destination["evidence"]
                .as_array_mut()
                .unwrap()
                .push(json!({"test":id,"relationship":relationship,"reason":reason}));
            tests_by_feature.entry(feature).or_default().entry(id.clone()).or_insert_with(||json!({"id":id,"package":test.context.target.package,"target":test.context.target.value(),"name":test.name(),"source":source,"scope":scope}));
        }
        classifications.push(json!({"identity":identity,"source":source,"classification":"evidence","feature":feature}));
    }
    let mut entrypoints = Vec::new();
    let mut surfaces = Vec::new();
    let mut graphs = Vec::new();
    for (id, value) in features {
        for entry in value["entrypoints"]
            .as_array()
            .ok_or("entrypoints array required")?
        {
            let mut entry = entry.clone();
            entry["feature"] = json!(id);
            entrypoints.push(entry);
        }
        for surface in value["surfaces"]
            .as_array()
            .ok_or("surfaces array required")?
        {
            let mut surface = surface.clone();
            surface["feature"] = json!(id);
            surfaces.push(surface);
        }
        let record = collector
            .declarations
            .iter()
            .find(|d| d.feature == id && d.contract.is_none())
            .unwrap()
            .context
            .file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        graphs.push(json!({"record":record,"graph":{"version":1,"feature":{"id":id,"title":value["title"],"source":value["source"],"flow":value["flow"]},"guarantees":value["guarantees"],"contracts":contracts.remove(&id).unwrap_or_default().into_values().collect::<Vec<_>>(),"tests":tests_by_feature.remove(&id).unwrap_or_default().into_values().collect::<Vec<_>>(),"limitations":value["limitations"]}}));
    }
    let inventories=inventories.into_iter().map(|(target,tests)|json!({"package":target.package,"target":target.value(),"tests":tests})).collect::<Vec<_>>();
    Ok(
        json!({"graphs":graphs,"inventories":inventories,"classifications":classifications,"entrypoints":entrypoints,"surfaces":surfaces,"excluded_targets":excluded_targets,"packages":packages,"examples":examples}),
    )
}
fn source_range(span: Span) -> (usize, usize) {
    let range = span.byte_range();
    (range.start, range.end)
}
fn scan(root: &Path) -> Result<Value> {
    let metadata = workspace(root)?;
    let (workspace_targets, _) = workspace_targets(&metadata)?;
    let mut collector = Collector {
        scan: true,
        root: root.to_owned(),
        ..Collector::default()
    };
    let mut targets = Vec::new();
    for target in workspace_targets {
        targets.push(json!({"package":target.package,"target":target.value(),"source":target.source.strip_prefix(root).unwrap()}));
        collector.file(
            Context {
                file: target.source.clone(),
                module_dir: target.source.parent().unwrap().to_owned(),
                path_dir: target.source.parent().unwrap().to_owned(),
                modules: vec![],
                target,
                unsupported_cfg: false,
            },
            &mut BTreeSet::new(),
        )?;
    }
    let mut tests = Vec::new();
    for test in collector.tests {
        let source = fs::read_to_string(&test.context.file).map_err(|e| e.to_string())?;
        let span = source_range(test.span);
        let body = source_range(test.body);
        let outer = test.outer.map(source_range);
        tests.push(json!({"package":test.context.target.package,"target":test.context.target.value(),"target_name":test.context.target.name,"target_source":test.context.target.source.strip_prefix(root).unwrap(),"file":test.context.file.strip_prefix(root).unwrap(),"name":test.name(),"function":test.function,"modules":test.context.modules,"module_dir":test.context.module_dir.strip_prefix(root).unwrap(),"span":{"start":span.0,"end":span.1},"outer_span":outer.map(|v|json!({"start":v.0,"end":v.1})),"body_span":{"start":body.0,"end":body.1},"function_tokens":test.tokens,"function_source":source[span.0..span.1],"body_source":source[body.0..body.1],"classification":test.classification.map(|v|v.0)}));
    }
    Ok(json!({"targets":targets,"tests":tests}))
}

fn main() {
    let scan_mode = env::args().any(|arg| arg == "--scan");
    let root = env::args_os()
        .skip(1)
        .find(|arg| arg != "--scan")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap());
    let result = root
        .canonicalize()
        .map_err(|e| e.to_string())
        .and_then(|root| {
            if scan_mode {
                scan(&root)
            } else {
                collect(&root)
            }
        });
    match result {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("traceability collector: {error}");
            process::exit(1);
        }
    }
}
