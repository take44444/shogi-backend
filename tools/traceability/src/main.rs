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
    kind: String,
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
                "Component" => &[
                    "description",
                    "notes",
                    "external",
                    "operations",
                    "entrypoints",
                ],
                "External" => &["id", "title"],
                "Operation" => &["contract", "audience", "surface", "reason"],
                "Feature" => &["id", "title", "source", "flow", "guarantees", "limitations"],
                "EntryPoint" => &["name", "source"],
                "Guarantee" => &["id", "statement", "roots"],
                "Contract" => &[
                    "id",
                    "consumer",
                    "provider",
                    "interface",
                    "statement",
                    "assumptions",
                    "limitations",
                    "source",
                    "call",
                    "implementation",
                    "binding",
                    "requires",
                    "gap",
                ],
                "Dependency" => &["contract", "reason"],
                "Source" => &["path", "needle"],
                "BoundaryCase" => &["evidence"],
                "Evidence" => &[
                    "contract",
                    "scope",
                    "relationship",
                    "reason",
                    "observation",
                    "real",
                    "fake",
                ],
                "Example" => &["kind", "reason"],
                "OpenApi" => &["package", "example", "operation"],
                "Grpc" => &["path", "service", "operation"],
                "WebSocket" => &["path", "command", "source"],
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
            if matches!(kind.as_str(), "OpenApi" | "Grpc" | "WebSocket") {
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
                "Frontend"
                | "Internal"
                | "Operator"
                | "RustApi"
                | "Port"
                | "Http"
                | "Grpc"
                | "WebSocket"
                | "External"
                | "ProviderBoundary"
                | "CompleteInteraction" => Ok(json!(name)),
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
fn classified_directory(file: &Path, package_root: &Path) -> bool {
    file.strip_prefix(package_root).is_ok_and(|relative| {
        [
            "src/trace",
            "src/tests",
            "tests/contracts",
            "tests/internal",
        ]
        .iter()
        .any(|path| relative.starts_with(path))
            || (relative.starts_with("src")
                && relative.file_name().is_some_and(|name| name == "tests.rs"))
    })
}
fn declaration_path(context: &Context, kind: &str, value: &Value) -> Result<()> {
    if !matches!(context.target.kind.as_str(), "lib" | "bin") {
        return Err("metadata declarations require a lib or bin target".into());
    }
    let expected = match kind {
        "component" => PathBuf::from("src/trace/component.rs"),
        "feature" | "contract" => {
            let id = nonempty(&value["id"])?;
            if id.contains(['/', '\\']) || id == "." || id == ".." {
                return Err("declaration ID must be a local file name".into());
            }
            PathBuf::from("src/trace")
                .join(if kind == "feature" {
                    "features"
                } else {
                    "contracts"
                })
                .join(format!("{id}.rs"))
        }
        _ => unreachable!(),
    };
    if context.file.strip_prefix(&context.target.package_root).ok() != Some(expected.as_path()) {
        return Err(format!(
            "{kind} declaration must live in {}",
            expected.display()
        ));
    }
    Ok(())
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
                && !classified_directory(&context.file, &context.target.package_root)
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
                        if classified_directory(&context.file, &context.target.package_root) {
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
                        "component" | "feature" | "contract" if self.scan => (),
                        "component" | "feature" | "contract" => {
                            if context.unsupported_cfg || !cfg_supported(&item.attrs) {
                                return Err(fail(
                                    context,
                                    item.span(),
                                    "unsupported cfg on trace declaration",
                                ));
                            }
                            let expr = syn::parse2::<Expr>(item.mac.tokens.clone())
                                .map_err(|e| fail(context, e.span(), e))?;
                            let value =
                                literal(&expr).map_err(|e| fail(context, expr.span(), e))?;
                            declaration_path(context, &name, &value)
                                .map_err(|e| fail(context, item.span(), e))?;
                            self.declarations.push(Declaration {
                                kind: name,
                                value,
                                context: context.clone(),
                            });
                        }
                        "boundary_case" | "case" if self.scan || name == "boundary_case" => {
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
                                if classified_directory(&context.file, &context.target.package_root)
                                {
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
// Internal source tests remain part of the inventory even when a module link is lost.
fn source_test_files(path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_symlink() {
            return Err(format!(
                "source test inventory cannot traverse symlinks: {}",
                path.display()
            ));
        }
        if path.is_dir() {
            source_test_files(&path, files)?;
        } else if path.file_name().is_some_and(|name| name == "tests.rs") {
            files.insert(path.canonicalize().map_err(|e| e.to_string())?);
        }
    }
    Ok(())
}
fn websocket_commands(item: &syn::ItemEnum) -> Result<BTreeSet<String>> {
    let mut tag = None;
    let mut rename = None;
    if !cfg_supported(&item.attrs) || item.attrs.iter().any(|a| a.path().is_ident("cfg")) {
        return Err("unsupported cfg on WebSocket command enum".into());
    }
    for attr in item.attrs.iter().filter(|a| a.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") || meta.path.is_ident("rename_all") {
                let value = meta.value()?.parse::<syn::LitStr>()?.value();
                let slot = if meta.path.is_ident("tag") {
                    &mut tag
                } else {
                    &mut rename
                };
                if slot.replace(value).is_some() {
                    return Err(meta.error("duplicate WebSocket serde rule"));
                }
                Ok(())
            } else if meta.path.is_ident("deny_unknown_fields") {
                Ok(())
            } else {
                Err(meta.error("unsupported WebSocket serde rule"))
            }
        })
        .map_err(|e| format!("unsupported WebSocket serde rule: {e}"))?;
    }
    if tag.as_deref() != Some("type") || rename.as_deref() != Some("snake_case") {
        return Err("WebSocket command enum requires serde(tag = \"type\", rename_all = \"snake_case\"); unsupported serde rule".into());
    }
    let mut commands = BTreeSet::new();
    for variant in &item.variants {
        if variant.attrs.iter().any(|attr| {
            matches!(
                attr.path().get_ident().map(|i| i.to_string()).as_deref(),
                Some("serde" | "cfg" | "cfg_attr")
            )
        }) {
            return Err("unsupported WebSocket command variant serde/cfg rule".into());
        }
        let name = variant.ident.to_string();
        if !name.is_ascii() || name.starts_with("r#") {
            return Err("unsupported WebSocket command variant identifier".into());
        }
        // Match serde's snake_case conversion for enum variants, including acronyms.
        let mut command = String::new();
        for (index, character) in name.chars().enumerate() {
            if character.is_ascii_uppercase() && index != 0 {
                command.push('_');
            }
            command.push(character.to_ascii_lowercase());
        }
        if !commands.insert(command) {
            return Err("duplicate WebSocket wire command".into());
        }
    }
    Ok(commands)
}
fn websocket_inventory(root: &Path, components: &BTreeMap<String, Value>) -> Result<()> {
    // Each anchored enum is reconciled separately for each endpoint and component.
    for component in components.values() {
        let mut enums =
            BTreeMap::<(String, PathBuf, usize), (BTreeSet<String>, BTreeSet<String>)>::new();
        for operation in component["operations"]
            .as_array()
            .ok_or("operations missing")?
        {
            let surface = &operation["surface"];
            if surface["kind"] != "WebSocket" {
                continue;
            }
            let endpoint = nonempty(&surface["path"])?;
            let command = nonempty(&surface["command"])?;
            let source = nonempty(&surface["source"]["path"])?;
            let needle = nonempty(&surface["source"]["needle"])?;
            let path = root
                .join(source)
                .canonicalize()
                .map_err(|e| format!("{source}: {e}"))?;
            if !path.starts_with(root) {
                return Err("WebSocket source outside repository".into());
            }
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let mut matches = content.match_indices(needle);
            let offset = matches
                .next()
                .ok_or_else(|| format!("{source}: WebSocket source anchor not found"))?
                .0;
            if matches.next().is_some() {
                return Err(format!("{source}: WebSocket source anchor is ambiguous"));
            }
            let ast = syn::parse_file(&content).map_err(|e| format!("{source}: {e}"))?;
            if command == "upgrade" {
                struct Routes {
                    anchor: usize,
                    end: usize,
                    paths: Vec<String>,
                }
                impl<'ast> syn::visit::Visit<'ast> for Routes {
                    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
                        if node.method == "route"
                            && node.method.span().byte_range().start.saturating_sub(1)
                                <= self.anchor
                            && self.end <= node.span().byte_range().end
                            && node.args.len() == 2
                            && let (Expr::Lit(path), Expr::Call(handler)) =
                                (&node.args[0], &node.args[1])
                            && let (Lit::Str(path), Expr::Path(get)) = (&path.lit, &*handler.func)
                            && get.path.is_ident("get")
                            && handler.args.len() == 1
                            && matches!(&handler.args[0], Expr::Path(upgrade) if upgrade.path.is_ident("upgrade"))
                        {
                            self.paths.push(path.value());
                        }
                        syn::visit::visit_expr_method_call(self, node);
                    }
                }
                let mut routes = Routes {
                    anchor: offset,
                    end: offset + needle.len(),
                    paths: Vec::new(),
                };
                syn::visit::Visit::visit_file(&mut routes, &ast);
                if routes.paths != [endpoint] {
                    return Err(format!(
                        "{source}: WebSocket upgrade must anchor the matching .route(path, get(upgrade)); declared path {endpoint}"
                    ));
                }
            } else {
                let item = ast.items.iter().find_map(|item| match item {
                    Item::Enum(item) if item.span().byte_range().contains(&offset) && offset + needle.len() <= item.span().byte_range().end => Some(item),
                    _ => None,
                }).ok_or_else(|| format!("{source}: WebSocket command source must anchor a top-level serde command enum"))?;
                let expected = websocket_commands(item).map_err(|e| format!("{source}: {e}"))?;
                let (_, declared) = enums
                    .entry((endpoint.into(), path, item.span().byte_range().start))
                    .or_insert_with(|| (expected, BTreeSet::new()));
                if !declared.insert(command.to_owned()) {
                    return Err(format!(
                        "{source}: duplicate WebSocket command {command} for {endpoint}"
                    ));
                }
            }
        }
        for ((endpoint, source, _), (expected, declared)) in enums {
            if expected != declared {
                return Err(format!(
                    "{}: WebSocket command inventory mismatch for {endpoint}: missing {:?}; extra {:?}",
                    source.display(),
                    expected.difference(&declared).collect::<Vec<_>>(),
                    declared.difference(&expected).collect::<Vec<_>>()
                ));
            }
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
                excluded.push(json!({"package":target_value.package,"target":target_value.value(),"reason":"Cargo の test = false 指定に従い、このターゲットを既定のテスト実行対象から除外する。"}));
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
    let mut components = BTreeMap::new();
    for package in metadata["packages"].as_array().ok_or("packages missing")? {
        if !members.contains(&package["id"].as_str().ok_or("package id missing")?) {
            continue;
        }
        let manifest = PathBuf::from(nonempty(&package["manifest_path"])?);
        let package_root = manifest.parent().unwrap();
        let path = package_root
            .strip_prefix(root)
            .map_err(|_| "workspace package outside repository")?;
        let name = nonempty(&package["name"])?;
        packages.push(json!({"name":name,"path":path}));
        components.insert(name.to_owned(), json!({"id":name,"package":name,"path":path,"description":name,"notes":[],"external":[],"operations":[],"entrypoints":[],"record":null}));
        for target in package["targets"].as_array().ok_or("targets missing")? {
            if strings(&target["kind"])?.contains(&"example") {
                if target["required-features"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty())
                {
                    return Err(format!(
                        "{} {}: required-features examples are unsupported by the default-host/default-Cargo-feature inventory policy",
                        name, target["name"]
                    ));
                }
                examples.push(example_declaration(root, package, target)?);
            }
        }
        let trace = package_root.join("src/trace");
        if trace.exists() {
            if trace.is_symlink() || !trace.is_dir() {
                return Err(format!("metadata directory expected: {}", trace.display()));
            }
            for entry in fs::read_dir(&trace).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                let valid = if path.is_dir() {
                    matches!(
                        path.file_name().and_then(|n| n.to_str()),
                        Some("features" | "contracts")
                    )
                } else {
                    path.file_name().is_some_and(|n| n == "component.rs")
                };
                if !valid || path.is_symlink() {
                    return Err(format!("unsupported metadata path: {}", path.display()));
                }
                if path.is_dir() {
                    for child in fs::read_dir(&path).map_err(|e| e.to_string())? {
                        let child = child.map_err(|e| e.to_string())?.path();
                        if !child.is_file()
                            || child.is_symlink()
                            || child.extension().is_none_or(|e| e != "rs")
                        {
                            return Err(format!(
                                "metadata declarations require direct Rust files: {}",
                                child.display()
                            ));
                        }
                    }
                }
            }
            rust_files(&trace, &mut files)?;
        }
        let legacy = package_root.join("tests/trace");
        if legacy.exists() {
            return Err(format!(
                "legacy trace directory is unsupported: {}",
                legacy.display()
            ));
        }
        let source = package_root.join("src");
        if source.exists() {
            source_test_files(&source, &mut files)?;
        }
        for convention in ["src/tests", "tests/contracts", "tests/internal"] {
            let directory = package_root.join(convention);
            if directory.exists() {
                if directory.is_symlink() || !directory.is_dir() {
                    return Err(format!("test directory expected: {}", directory.display()));
                }
                rust_files(&directory, &mut files)?;
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
        if context.unsupported_cfg
            && classified_directory(&context.file, &context.target.package_root)
        {
            return Err(format!(
                "{}: unsupported cfg inside trace directory",
                context.file.display()
            ));
        }
    }
    for (context, name) in &collector.macros {
        if matches!(name.as_str(), "case" | "outside")
            || classified_directory(&context.file, &context.target.package_root)
        {
            return Err(format!(
                "{}: unsupported item macro {name} inside classified scope",
                context.file.display()
            ));
        }
    }
    for file in &files {
        let metadata_file = components.values().any(|component| {
            let package_root = root.join(component["path"].as_str().unwrap());
            file.strip_prefix(package_root)
                .is_ok_and(|relative| relative.starts_with("src/trace"))
        });
        if metadata_file
            && collector
                .declarations
                .iter()
                .filter(|d| &d.context.file == file)
                .count()
                != 1
        {
            return Err(format!(
                "{}: metadata file requires exactly one declaration",
                file.display()
            ));
        }
    }
    let mut features = BTreeMap::new();
    let mut contracts = BTreeMap::new();
    let mut declared_components = BTreeSet::new();
    for declaration in &collector.declarations {
        let owner = &declaration.context.target.package;
        let record = declaration.context.file.strip_prefix(root).unwrap();
        let mut value = declaration.value.clone();
        value["record"] = json!(record);
        if declaration.kind == "component" {
            if !declared_components.insert(owner.clone()) {
                return Err(format!("duplicate component {owner}"));
            }
            let component = components.get_mut(owner).unwrap();
            for (key, value) in value.as_object().unwrap() {
                component[key] = value.clone();
            }
            continue;
        }
        value["owner"] = json!(owner);
        let local = nonempty(&value["id"])?;
        let id = if declaration.kind == "contract" {
            format!("{owner}/{local}")
        } else {
            local.to_owned()
        };
        value["id"] = json!(id);
        let map = if declaration.kind == "contract" {
            &mut contracts
        } else {
            &mut features
        };
        if map.insert(id.clone(), value).is_some() {
            return Err(format!("duplicate {} {id}", declaration.kind));
        }
    }
    websocket_inventory(root, &components)?;
    let mut inventories = targets
        .iter()
        .cloned()
        .map(|target| (target, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut tests = Vec::new();
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
        if !inventories
            .get_mut(&test.context.target)
            .unwrap()
            .insert(test.name())
        {
            return Err(format!("duplicate parsed test {}", test.name()));
        }
        let relative = test
            .context
            .file
            .strip_prefix(&test.context.target.package_root)
            .map_err(|_| format!("{}: test source belongs to another package", test.name()))?;
        let (class, semantics) = match &test.classification {
            Some((kind, semantics)) if kind == "boundary_case" => {
                if test.context.target.kind != "test" || !relative.starts_with("tests/contracts") {
                    return Err(format!(
                        "{}: boundary_case requires tests/contracts/ in an integration target",
                        test.name()
                    ));
                }
                ("boundary", semantics.clone())
            }
            Some((kind, semantics)) if matches!(kind.as_str(), "helper" | "infrastructure") => {
                nonempty(&semantics["reason"])?;
                if kind == "infrastructure"
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
                (kind.as_str(), semantics.clone())
            }
            None if relative.starts_with("src/trace") => {
                return Err(format!(
                    "{}: metadata directory cannot contain tests",
                    test.name()
                ));
            }
            None if relative.starts_with("src") || relative.starts_with("tests/internal") => {
                ("internal", json!({}))
            }
            _ => {
                return Err(format!(
                    "{}: unclassified test; boundary tests require explicit boundary_case! under tests/contracts/; internal integration tests require tests/internal/",
                    test.name()
                ));
            }
        };
        let base_id = test_id(test);
        let mut id = base_id.clone();
        let mut suffix = 1;
        while !used_ids.insert(id.clone()) {
            suffix += 1;
            id = format!("{base_id}-{suffix}");
        }
        tests.push(json!({"id":id,"package":test.context.target.package,"target":test.context.target.value(),"name":test.name(),"source":test_source(test, root)?,"classification":class,"semantics":semantics}));
    }
    let inventories = inventories.into_iter().map(|(target,tests)| json!({"package":target.package,"target":target.value(),"tests":tests})).collect::<Vec<_>>();
    Ok(
        json!({"version":2,"components":components.into_values().collect::<Vec<_>>(),"features":features.into_values().collect::<Vec<_>>(),"contracts":contracts.into_values().collect::<Vec<_>>(),"tests":tests,"inventories":inventories,"packages":packages,"examples":examples,"excluded_targets":excluded_targets}),
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
