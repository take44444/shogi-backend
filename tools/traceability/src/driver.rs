use crate::model::{Api, Call, Coverage, Location, Shard, Test, Unit, Unresolved, Workspace};
use rustc_attr_ir::{Attribute, AttributeKind, LangItem};
use rustc_data_structures::steal::Steal;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::{
    self as hir,
    def::DefKind,
    def_id::{CRATE_DEF_INDEX, DefId, LocalDefId},
    intravisit::{self, Visitor},
};
use rustc_interface::interface;
use rustc_middle::{
    mir::{self, visit::Visitor as MirVisitor},
    ty::{self, TyCtxt},
    util::Providers,
};
use rustc_span::{FileName, Span};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env, fs,
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

const TARGET_MARKER: &str = "__traceability_targets_";
type MirProvider = for<'tcx> fn(TyCtxt<'tcx>, LocalDefId) -> &'tcx Steal<mir::Body<'tcx>>;
static UNIT_TARGET: OnceLock<String> = OnceLock::new();
static MIR_PROVIDER: OnceLock<MirProvider> = OnceLock::new();
static WORKSPACE: OnceLock<Workspace> = OnceLock::new();
static GENERATED_SOURCES: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
static CAPTURE: OnceLock<Mutex<Capture>> = OnceLock::new();
static DEFINITION_SIBLINGS: OnceLock<Mutex<DefinitionSiblings>> = OnceLock::new();

type SiblingKey = (
    hir::def_id::CrateNum,
    Option<hir::def_id::DefIndex>,
    hir::definitions::DefPathData,
);

#[derive(Default)]
struct DefinitionSiblings {
    indexed: BTreeMap<hir::def_id::CrateNum, usize>,
    groups: HashMap<SiblingKey, Vec<DefId>>,
}
impl DefinitionSiblings {
    fn get(&mut self, tcx: TyCtxt<'_>, definition: DefId) -> Vec<DefId> {
        let count = if definition.is_local() {
            tcx.definitions_untracked().num_definitions()
        } else {
            tcx.num_extern_def_ids(definition.krate)
        };
        let first = self.indexed.get(&definition.krate).copied().unwrap_or(0);
        for index in first..count {
            let sibling = DefId {
                krate: definition.krate,
                index: hir::def_id::DefIndex::from_usize(index),
            };
            let key = tcx.def_key(sibling);
            self.groups
                .entry((definition.krate, key.parent, key.disambiguated_data.data))
                .or_default()
                .push(sibling);
        }
        self.indexed.insert(definition.krate, count);
        let key = tcx.def_key(definition);
        self.groups[&(definition.krate, key.parent, key.disambiguated_data.data)].clone()
    }
}

#[derive(Default)]
struct Capture {
    error: Option<String>,
    apis: BTreeMap<String, (DefId, Api)>,
    calls: Vec<Call>,
    references: Vec<Call>,
    unresolved: Vec<Unresolved>,
    coverage: Coverage,
}
impl Capture {
    fn insert_api(&mut self, definition: DefId, api: Api) {
        if self.error.is_some() {
            return;
        }
        if let Some((previous_definition, previous)) = self.apis.get(&api.id) {
            if !(*previous_definition == definition
                && previous.package == api.package
                && previous.name == api.name
                && previous.kind == api.kind
                && previous.path == api.path
                && previous.line == api.line)
            {
                self.error = Some(format!(
                    "compiler API identity collision for {} between DefId({}:{}) and DefId({}:{}); fix semantic definition identity before capture",
                    api.id.chars().take(240).collect::<String>(),
                    previous_definition.krate.as_u32(),
                    previous_definition.index.as_u32(),
                    definition.krate.as_u32(),
                    definition.index.as_u32(),
                ));
            }
        } else {
            self.apis.insert(api.id.clone(), (definition, api));
        }
    }
    fn merge(&mut self, other: Self) {
        if self.error.is_some() {
            return;
        }
        self.error = other.error;
        for (definition, api) in other.apis.into_values() {
            self.insert_api(definition, api);
        }
        self.calls.extend(other.calls);
        self.references.extend(other.references);
        self.unresolved.extend(other.unresolved);
        self.coverage.bodies += other.coverage.bodies;
        self.coverage.calls += other.coverage.calls;
        self.coverage.workspace_calls += other.coverage.workspace_calls;
        self.coverage.external_calls += other.coverage.external_calls;
        self.coverage.unresolved_calls += other.coverage.unresolved_calls;
        self.coverage.function_references += other.coverage.function_references;
        self.coverage.implicit_drops += other.coverage.implicit_drops;
        self.coverage.assertions += other.coverage.assertions;
        self.coverage.inline_assembly += other.coverage.inline_assembly;
    }
}

pub fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1).collect::<Vec<_>>();
    let compiler = arguments
        .first()
        .ok_or("expected rustc executable followed by compiler arguments")?
        .clone();
    let Some(root) = env::var_os("TRACEABILITY_ROOT") else {
        let status = Command::new(compiler)
            .args(&arguments[1..])
            .status()
            .map_err(|error| error.to_string())?;
        std::process::exit(status.code().unwrap_or(1));
    };
    let source = arguments
        .iter()
        .skip(1)
        .find(|argument| argument.ends_with(".rs"))
        .map(PathBuf::from);
    let Some(source) = source else {
        let status = Command::new(compiler)
            .args(&arguments[1..])
            .status()
            .map_err(|error| error.to_string())?;
        std::process::exit(status.code().unwrap_or(1));
    };
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let workspace = Workspace::load(root)?;
    let source = if source.is_absolute() {
        source
    } else {
        env::current_dir()
            .map_err(|error| error.to_string())?
            .join(source)
    };
    let package = env::var("CARGO_PKG_NAME").map_err(|_| "CARGO_PKG_NAME is required")?;
    let package_metadata = workspace
        .packages
        .iter()
        .find(|item| item.name == package)
        .ok_or("compiler package is not a workspace member")?;
    let crate_name = arguments
        .windows(2)
        .find(|pair| pair[0] == "--crate-name")
        .map(|pair| pair[1].clone())
        .ok_or("compiler arguments have no crate name")?;
    let mut targets = package_metadata
        .targets
        .iter()
        .filter(|target| target.source == source && target.name.replace('-', "_") == crate_name);
    let target = targets.next().ok_or_else(|| {
        format!(
            "no Cargo target matches {crate_name} at {}",
            source.display()
        )
    })?;
    if targets.next().is_some() {
        return Err(format!(
            "multiple Cargo targets match {crate_name} at {}",
            source.display()
        ));
    }
    let mut features = BTreeSet::new();
    for (index, argument) in arguments.iter().enumerate() {
        let cfg = if argument == "--cfg" {
            arguments.get(index + 1).map(String::as_str)
        } else {
            argument.strip_prefix("--cfg=")
        };
        if let Some(feature) = cfg.and_then(|cfg| cfg.strip_prefix("feature=")) {
            features.insert(
                serde_json::from_str::<String>(feature)
                    .map_err(|error| format!("invalid compiler feature cfg {feature}: {error}"))?,
            );
        }
    }
    let unit = Unit {
        package,
        crate_name,
        target_kind: target.kind.clone(),
        target_name: target.name.clone(),
        is_test: arguments.iter().any(|argument| argument == "--test"),
        features: features.into_iter().collect(),
        source: workspace.relative(&source),
    };
    UNIT_TARGET
        .set(format!(
            "{}:{}:{}",
            unit.target_kind, unit.target_name, unit.source
        ))
        .map_err(|_| "compiler supports one compilation per process")?;
    WORKSPACE
        .set(workspace)
        .map_err(|_| "compiler supports one compilation per process")?;
    GENERATED_SOURCES
        .set(Mutex::new(BTreeMap::new()))
        .map_err(|_| "compiler supports one compilation per process")?;
    CAPTURE
        .set(Mutex::new(Capture::default()))
        .map_err(|_| "compiler supports one compilation per process")?;
    if !arguments.iter().any(|argument| argument == "--sysroot") {
        let output = Command::new(&compiler)
            .args(["--print", "sysroot"])
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err("failed to discover compiler sysroot".into());
        }
        arguments.extend([
            "--sysroot".into(),
            String::from_utf8_lossy(&output.stdout).trim().into(),
        ]);
    }
    let mut callbacks = Collector {
        unit: Some(unit),
        error: None,
    };
    rustc_driver::catch_fatal_errors(|| {
        rustc_driver::compiler_entrypoint(&arguments, &mut callbacks)
    })
    .map_err(|_| "compilation failed".to_owned())?;
    if let Some(error) = callbacks.error {
        return Err(error);
    }
    Ok(())
}

struct Collector {
    unit: Option<Unit>,
    error: Option<String>,
}
impl Callbacks for Collector {
    fn config(&mut self, config: &mut interface::Config) {
        config.override_queries = Some(override_queries);
    }
    fn after_analysis<'tcx>(&mut self, _: &interface::Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let unit = self.unit.take().unwrap();
        let mut tests = BTreeMap::new();
        for definition in tcx.hir_crate_items(()).definitions() {
            for attribute in tcx.hir_attrs(tcx.local_def_id_to_hir_id(definition)) {
                if let Attribute::Parsed(AttributeKind::RustcTestMarker(name)) = attribute {
                    let location = source_location(tcx, tcx.def_span(definition).source_callsite());
                    tests.insert(
                        name.to_string(),
                        Test {
                            package: unit.package.clone(),
                            target_kind: unit.target_kind.clone(),
                            target_name: unit.target_name.clone(),
                            name: name.to_string(),
                            path: location.path,
                            line: location.line,
                            targets: vec![],
                        },
                    );
                }
            }
        }
        let mut reference_capture = Capture::default();
        for definition in tcx.hir_body_owners() {
            let body = tcx.hir_body_owned_by(definition);
            let name = tcx
                .opt_item_name(definition.to_def_id())
                .map(|name| name.to_string())
                .unwrap_or_default();
            let marker = name.strip_prefix(TARGET_MARKER);
            let mut visitor = References {
                tcx,
                typeck: tcx.typeck(definition),
                caller: definition.to_def_id(),
                marker: marker.is_some(),
                targets: BTreeSet::new(),
                invalid_targets: Vec::new(),
                capture: &mut reference_capture,
            };
            visitor.visit_body(body);
            if let Some(test_name) = marker {
                if let Some(error) = visitor.invalid_targets.first() {
                    self.error = Some(error.clone());
                }
                if visitor.targets.is_empty() {
                    self.error = Some(format!(
                        "test target marker {name} contains no workspace callable target"
                    ));
                }
                let name = tcx
                    .def_path(tcx.parent(definition.to_def_id()))
                    .data
                    .iter()
                    .filter_map(|part| part.data.get_opt_name())
                    .map(|name| name.to_string())
                    .chain(std::iter::once(test_name.to_owned()))
                    .collect::<Vec<_>>()
                    .join("::");
                if let Some(test) = tests.get_mut(&name) {
                    test.targets.extend(visitor.targets);
                } else {
                    self.error = Some(format!("targets marker has no compiler test named {name}"));
                }
            }
        }
        for &definition in tcx.mir_keys(()) {
            if tcx.is_constructor(definition.to_def_id()) {
                // Constructor MIR is synthesized without a type-checkable source body.
                if let Some(api) = api(tcx, definition.to_def_id()) {
                    reference_capture.insert_api(definition.to_def_id(), api);
                }
            } else {
                tcx.ensure_done().mir_built(definition);
            }
        }
        let mut capture = CAPTURE.get().unwrap().lock().unwrap();
        capture.merge(reference_capture);
        if let Some(error) = capture.error.take() {
            self.error = Some(error);
        }
        if self.error.is_some() {
            return Compilation::Stop;
        }
        let capture = std::mem::take(&mut *capture);
        let mut shard = Shard {
            generated_sources: std::mem::take(
                &mut *GENERATED_SOURCES.get().unwrap().lock().unwrap(),
            ),
            version: 3,
            unit,
            apis: capture.apis.into_values().map(|(_, api)| api).collect(),
            calls: capture.calls,
            references: capture.references,
            tests: tests.into_values().collect(),
            unresolved: capture.unresolved,
            coverage: capture.coverage,
        };
        shard.calls.sort_by(|a, b| {
            (&a.caller, &a.callee, &a.path, a.line, &a.dispatch).cmp(&(
                &b.caller,
                &b.callee,
                &b.path,
                b.line,
                &b.dispatch,
            ))
        });
        shard.references.sort_by(|a, b| {
            (&a.caller, &a.callee, &a.path, a.line).cmp(&(&b.caller, &b.callee, &b.path, b.line))
        });
        let result = (|| {
            let directory = PathBuf::from(
                env::var_os("TRACEABILITY_SHARDS").ok_or("TRACEABILITY_SHARDS is required")?,
            );
            fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let filename = format!(
                "{}-{}-{}-{}.json",
                shard.unit.package,
                shard.unit.target_kind,
                shard.unit.target_name,
                std::process::id()
            );
            let encoded = serde_json::to_vec_pretty(&shard).map_err(|error| error.to_string())?;
            fs::write(directory.join(filename), encoded).map_err(|error| error.to_string())
        })();
        if let Err(error) = result {
            self.error = Some(error);
        }
        Compilation::Continue
    }
}

fn override_queries(_: &rustc_session::Session, providers: &mut Providers) {
    MIR_PROVIDER.set(providers.queries.mir_built).unwrap();
    providers.queries.mir_built = capture_mir;
}
fn capture_mir<'tcx>(tcx: TyCtxt<'tcx>, definition: LocalDefId) -> &'tcx Steal<mir::Body<'tcx>> {
    let result = MIR_PROVIDER.get().unwrap()(tcx, definition);
    if !is_infrastructure(tcx, definition.to_def_id()) {
        let body = result.borrow();
        let mut collector = BodyCalls {
            tcx,
            body: &body,
            caller: definition.to_def_id(),
            capture: Capture::default(),
        };
        collector.capture.coverage.bodies = 1;
        if let Some(api) = api(tcx, definition.to_def_id()) {
            collector.capture.insert_api(definition.to_def_id(), api);
        }
        collector.visit_body(&body);
        CAPTURE
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .merge(collector.capture);
    }
    result
}

fn is_infrastructure(tcx: TyCtxt<'_>, mut definition: DefId) -> bool {
    if tcx.sess.opts.test
        && tcx
            .entry_fn(())
            .is_some_and(|(entry, _)| entry == definition)
    {
        return true;
    }
    loop {
        if tcx
            .opt_item_name(definition)
            .is_some_and(|name| name.as_str().starts_with(TARGET_MARKER))
        {
            return true;
        }
        if matches!(tcx.def_kind(definition), DefKind::Static { .. }) {
            #[allow(deprecated)]
            if tcx.get_all_attrs(definition).iter().any(|attribute| {
                matches!(
                    attribute,
                    Attribute::Parsed(AttributeKind::RustcTestMarker(_))
                )
            }) {
                return true;
            }
        }
        if tcx.opt_parent(definition).is_none() {
            return false;
        }
        definition = tcx.parent(definition);
    }
}
fn source_path(tcx: TyCtxt<'_>, span: Span) -> Option<PathBuf> {
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    match &location.file.name {
        FileName::Real(name) => name.local_path().map(|path| {
            let path = if path.is_absolute() {
                path.to_owned()
            } else {
                env::current_dir().unwrap().join(path)
            };
            path.canonicalize().unwrap_or(path)
        }),
        _ => None,
    }
}
fn source_location(tcx: TyCtxt<'_>, span: Span) -> Location {
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    let workspace = WORKSPACE.get().unwrap();
    let source = source_path(tcx, span);
    let callsite = span.source_callsite();
    if callsite != span
        && source
            .as_ref()
            .is_some_and(|path| workspace.package_at(path).is_none())
    {
        return source_location(tcx, callsite);
    }
    let path = source
        .map(|path| {
            if workspace.package_at(&path).is_some() {
                return workspace.relative(&path);
            }
            let root = DefId {
                krate: location.file.cnum,
                index: CRATE_DEF_INDEX,
            };
            let Some(package) = owner_package(tcx, root) else {
                // The compiler crate identity retains version and configuration context.
                // Its source hash identifies the file without a machine-specific cache path.
                return format!(
                    "@compiler-source/{}/{:x}/{}",
                    tcx.crate_name(location.file.cnum),
                    tcx.stable_crate_id(location.file.cnum),
                    location.file.src_hash,
                );
            };
            let components = path.components().collect::<Vec<_>>();
            let out = components
                .iter()
                .rposition(|component| component.as_os_str() == "out");
            let relative = if let Some(out) = out {
                components[out + 1..].iter().collect::<PathBuf>()
            } else {
                PathBuf::from(path.file_name().unwrap())
            };
            let key = format!("@generated/{}/{}", package.name, relative.to_string_lossy());
            let contents = if let Some(contents) = &location.file.src {
                (**contents).clone()
            } else {
                fs::read_to_string(&path).unwrap_or_else(|error| {
                    panic!("cannot capture compiler source {}: {error}", path.display())
                })
            };
            let mut sources = GENERATED_SOURCES.get().unwrap().lock().unwrap();
            if let Some(previous) = sources.insert(key.clone(), contents.clone()) {
                assert_eq!(
                    previous, contents,
                    "generated source identity collision for {key}"
                );
            }
            key
        })
        .unwrap_or_else(|| {
            location
                .file
                .name
                .prefer_local_unconditionally()
                .to_string()
        });
    Location {
        path,
        line: location.line,
    }
}
fn owner_package(tcx: TyCtxt<'_>, definition: DefId) -> Option<&'static crate::model::Package> {
    let workspace = WORKSPACE.get().unwrap();
    if definition.is_local() {
        let name = env::var("CARGO_PKG_NAME").unwrap();
        return workspace
            .packages
            .iter()
            .find(|package| package.name == name);
    }
    let root = DefId {
        krate: definition.krate,
        index: CRATE_DEF_INDEX,
    };
    let root_span = tcx.def_span(root);
    if !root_span.is_dummy()
        && let Some(path) = source_path(tcx, root_span)
    {
        return workspace.package_at(&path);
    }
    let name = tcx.crate_name(definition.krate);
    let mut packages = workspace.packages.iter().filter(|package| {
        package
            .targets
            .iter()
            .any(|target| target.name.replace('-', "_") == name.as_str())
    });
    let package = packages.next()?;
    packages.next().is_none().then_some(package)
}
fn target_identity(tcx: TyCtxt<'_>, definition: DefId) -> Option<String> {
    if definition.is_local() {
        return UNIT_TARGET.get().cloned();
    }
    let package = owner_package(tcx, definition)?;
    let workspace = WORKSPACE.get().unwrap();
    let root = DefId {
        krate: definition.krate,
        index: CRATE_DEF_INDEX,
    };
    let span = tcx.def_span(root);
    if !span.is_dummy()
        && let Some(path) = source_path(tcx, span)
        && let Some(target) = package.targets.iter().find(|target| target.source == path)
    {
        return Some(format!(
            "{}:{}:{}",
            target.kind,
            target.name,
            workspace.relative(&target.source)
        ));
    }
    let name = tcx.crate_name(definition.krate);
    let mut targets = package
        .targets
        .iter()
        .filter(|target| target.name.replace('-', "_") == name.as_str());
    let target = targets.next()?;
    targets.next().is_none().then(|| {
        format!(
            "{}:{}:{}",
            target.kind,
            target.name,
            workspace.relative(&target.source)
        )
    })
}
fn canonical_name(tcx: TyCtxt<'_>, definition: DefId) -> String {
    let path = tcx
        .def_path(definition)
        .data
        .iter()
        .filter_map(|part| part.data.get_opt_name())
        .map(|name| name.to_string())
        .collect::<Vec<_>>()
        .join("::");
    if let Some(item) = tcx.opt_associated_item(definition)
        && let Some(implementation) = item.impl_container(tcx)
    {
        let receiver = canonical_type(
            tcx,
            tcx.type_of(implementation)
                .instantiate_identity()
                .skip_norm_wip(),
            false,
        );
        return match item.container {
            ty::AssocContainer::TraitImpl(_) => {
                let trait_ref = tcx
                    .impl_trait_ref(implementation)
                    .instantiate_identity()
                    .skip_norm_wip();
                format!(
                    "<{receiver} as {}>::{}",
                    canonical_generic_path(
                        tcx,
                        trait_ref.def_id,
                        &trait_ref.args.as_slice()[1..],
                        false
                    ),
                    item.name()
                )
            }
            _ => format!("{receiver}::{}", item.name()),
        };
    }
    let name = format!("{}::{path}", tcx.crate_name(definition.krate));
    if matches!(
        tcx.def_kind(definition),
        DefKind::Closure | DefKind::SyntheticCoroutineBody
    ) {
        let location = source_location(tcx, tcx.def_span(definition));
        format!("{name}::{{closure at {}:{}}}", location.path, location.line)
    } else {
        name
    }
}
fn canonical_generic_path(
    tcx: TyCtxt<'_>,
    definition: DefId,
    arguments: &[ty::GenericArg<'_>],
    identity: bool,
) -> String {
    let name = if identity {
        definition_identity(tcx, definition)
    } else {
        canonical_name(tcx, definition)
    };
    if arguments.is_empty() {
        return name;
    }
    let arguments = arguments
        .iter()
        .map(|argument| match argument.kind() {
            ty::GenericArgKind::Type(ty) => canonical_type(tcx, ty, identity),
            ty::GenericArgKind::Lifetime(region) => region.to_string(),
            ty::GenericArgKind::Const(value) => canonical_const(tcx, value, identity),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}
fn canonical_const(tcx: TyCtxt<'_>, value: ty::Const<'_>, identity: bool) -> String {
    match value.kind() {
        ty::ConstKind::Alias(_, ty::AliasConst { kind, args, .. }) => match kind {
            ty::AliasConstKind::Projection { def_id }
            | ty::AliasConstKind::InherentSelf { def_id }
            | ty::AliasConstKind::InherentImpl { def_id }
            | ty::AliasConstKind::Free { def_id }
            | ty::AliasConstKind::Anon { def_id } => {
                canonical_generic_path(tcx, def_id, args.as_slice(), identity)
            }
        },
        _ => value.to_string(),
    }
}
fn canonical_type(tcx: TyCtxt<'_>, ty: ty::Ty<'_>, identity: bool) -> String {
    match *ty.kind() {
        ty::Adt(adt, arguments) => {
            canonical_generic_path(tcx, adt.did(), arguments.as_slice(), identity)
        }
        ty::Array(inner, length) => format!(
            "[{}; {}]",
            canonical_type(tcx, inner, identity),
            canonical_const(tcx, length, identity),
        ),
        ty::Ref(_, inner, mutability) => format!(
            "&{}{}",
            if mutability.is_mut() { "mut " } else { "" },
            canonical_type(tcx, inner, identity)
        ),
        ty::RawPtr(inner, mutability) => format!(
            "*{} {}",
            if mutability.is_mut() { "mut" } else { "const" },
            canonical_type(tcx, inner, identity)
        ),
        ty::Slice(inner) => format!("[{}]", canonical_type(tcx, inner, identity)),
        ty::Tuple(elements) => format!(
            "({})",
            elements
                .iter()
                .map(|inner| canonical_type(tcx, inner, identity))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => ty.to_string(),
    }
}
fn definition_origin(tcx: TyCtxt<'_>, definition: DefId) -> String {
    let span = tcx.def_span(definition);
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    let root = DefId {
        krate: location.file.cnum,
        index: CRATE_DEF_INDEX,
    };
    let workspace_source = source_path(tcx, span)
        .is_some_and(|path| WORKSPACE.get().unwrap().package_at(&path).is_some());
    let source = if workspace_source || owner_package(tcx, root).is_some() {
        source_location(tcx, span).path
    } else {
        format!(
            "@compiler-source/{}/{:x}/{}",
            tcx.crate_name(location.file.cnum),
            tcx.stable_crate_id(location.file.cnum),
            location.file.src_hash,
        )
    };
    format!(
        "{source}:{}:{}:{}",
        location.line,
        location.col.0,
        span.hi().0 - span.lo().0,
    )
}
fn implementation_identity(tcx: TyCtxt<'_>, definition: DefId) -> String {
    let receiver = canonical_type(
        tcx,
        tcx.type_of(definition)
            .instantiate_identity()
            .skip_norm_wip(),
        true,
    );
    let trait_ref = tcx.impl_opt_trait_ref(definition).map(|reference| {
        let reference = reference.instantiate_identity().skip_norm_wip();
        canonical_generic_path(tcx, reference.def_id, &reference.args.as_slice()[1..], true)
    });
    format!(
        "Impl<{receiver}{}>@{}",
        trait_ref
            .map(|name| format!(" as {name}"))
            .unwrap_or_default(),
        definition_origin(tcx, definition),
    )
}
fn definition_identity(tcx: TyCtxt<'_>, definition: DefId) -> String {
    let mut path = Vec::new();
    let mut parent = Some(definition);
    while let Some(current) = parent {
        let part = tcx.def_key(current).disambiguated_data;
        if part.data == hir::definitions::DefPathData::Impl {
            path.push(implementation_identity(tcx, current));
        } else {
            path.push(format!(
                "{:?}#{}@{}",
                part.data,
                origin_disambiguator(tcx, current),
                definition_origin(tcx, current),
            ));
        }
        parent = tcx.opt_parent(current);
    }
    path.reverse();
    let context = target_identity(tcx, definition)
        .unwrap_or_else(|| format!("{:x}", tcx.stable_crate_id(definition.krate)));
    format!(
        "{}[{context}]::{}@{}",
        tcx.crate_name(definition.krate),
        path.join("/"),
        definition_origin(tcx, definition),
    )
}
fn origin_disambiguator(tcx: TyCtxt<'_>, definition: DefId) -> usize {
    let key = tcx.def_key(definition);
    if key.disambiguated_data.disambiguator == 0 {
        return 0;
    }
    let span = tcx.def_span(definition);
    let siblings = DEFINITION_SIBLINGS
        .get_or_init(|| Mutex::new(DefinitionSiblings::default()))
        .lock()
        .unwrap()
        .get(tcx, definition);
    // Cfg can remove earlier siblings. Only indistinguishable source origins need numbering.
    siblings
        .into_iter()
        .filter(|&sibling| {
            let other = tcx.def_key(sibling);
            other.disambiguated_data.disambiguator < key.disambiguated_data.disambiguator
        })
        .filter(|&sibling| {
            let other = tcx.def_span(sibling);
            other.lo() == span.lo() && other.hi() == span.hi()
        })
        .count()
}
fn api(tcx: TyCtxt<'_>, definition: DefId) -> Option<Api> {
    let span = tcx.def_span(definition);
    if span.is_dummy() {
        return None;
    }
    source_path(tcx, span)?;
    let package = owner_package(tcx, definition)?;
    let location = source_location(tcx, span);
    let name = canonical_name(tcx, definition);
    let kind = match tcx.def_kind(definition) {
        DefKind::AssocFn => match tcx.associated_item(definition).container {
            ty::AssocContainer::InherentImpl => "inherent-method",
            ty::AssocContainer::Trait => "trait-method",
            ty::AssocContainer::TraitImpl(_) => "trait-implementation-method",
        },
        DefKind::Ctor(..) | DefKind::Struct | DefKind::Variant => "constructor",
        DefKind::Closure | DefKind::SyntheticCoroutineBody => "closure",
        DefKind::Fn => "function",
        _ => "other",
    }
    .to_owned();
    let id = definition_identity(tcx, definition);
    Some(Api {
        id,
        package: package.name.clone(),
        name,
        kind,
        path: location.path,
        line: location.line,
    })
}
fn trait_declaration(tcx: TyCtxt<'_>, definition: DefId) -> DefId {
    tcx.opt_associated_item(definition)
        .and_then(|item| item.trait_item_def_id())
        .unwrap_or(definition)
}
fn edge(
    tcx: TyCtxt<'_>,
    caller: DefId,
    callee: DefId,
    span: Span,
    dispatch: &str,
    capture: &mut Capture,
    reference: bool,
) -> bool {
    let caller_definition = caller;
    let callee_definition = callee;
    let Some(caller) = api(tcx, caller) else {
        return false;
    };
    let Some(callee) = api(tcx, callee) else {
        return false;
    };
    let callsite = span.source_callsite();
    let location = source_location(tcx, callsite);
    let expansion = (callsite != span).then(|| source_location(tcx, span));
    let value = Call {
        caller: caller.id.clone(),
        caller_package: caller.package.clone(),
        callee: callee.id.clone(),
        path: location.path,
        line: location.line,
        dispatch: dispatch.into(),
        expansion,
    };
    capture.insert_api(caller_definition, caller);
    capture.insert_api(callee_definition, callee);
    if reference {
        capture.references.push(value);
    } else {
        capture.calls.push(value);
    }
    true
}

struct BodyCalls<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'a mir::Body<'tcx>,
    caller: DefId,
    capture: Capture,
}
impl<'tcx> BodyCalls<'_, 'tcx> {
    fn resolved(&mut self, callee: DefId, span: Span, dispatch: &str) {
        self.capture.coverage.calls += 1;
        let callee = trait_declaration(self.tcx, callee);
        if owner_package(self.tcx, callee).is_none() {
            self.capture.coverage.external_calls += 1;
        } else if edge(
            self.tcx,
            self.caller,
            callee,
            span,
            dispatch,
            &mut self.capture,
            false,
        ) {
            self.capture.coverage.workspace_calls += 1;
        } else {
            self.capture.coverage.calls -= 1;
            self.unresolved(
                span,
                format!(
                    "workspace callable {} has no available source origin",
                    self.tcx.def_path_str(callee)
                ),
            );
        }
    }
    fn unresolved(&mut self, span: Span, reason: String) {
        self.capture.coverage.calls += 1;
        self.capture.coverage.unresolved_calls += 1;
        let location = source_location(self.tcx, span.source_callsite());
        let caller = api(self.tcx, self.caller)
            .map(|api| api.id)
            .unwrap_or_else(|| self.tcx.def_path_str(self.caller));
        self.capture.unresolved.push(Unresolved {
            caller,
            path: location.path,
            line: location.line,
            reason,
        });
    }
    fn call(
        &mut self,
        func: &mir::Operand<'tcx>,
        args: &[rustc_span::Spanned<mir::Operand<'tcx>>],
        span: Span,
        implicit: bool,
    ) {
        let ty = func.ty(self.body, self.tcx);
        match *ty.kind() {
            ty::FnDef(callee, substitutions) => {
                let declaration = trait_declaration(self.tcx, callee);
                let dispatch = if implicit {
                    "implicit"
                } else if self
                    .tcx
                    .opt_associated_item(declaration)
                    .is_some_and(|item| item.trait_container(self.tcx).is_some())
                {
                    match substitutions
                        .skip_binder()
                        .types()
                        .next()
                        .map(|ty| *ty.kind())
                    {
                        Some(ty::Dynamic(..)) => "dyn",
                        Some(ty::Param(_)) | Some(ty::Alias(..)) => "generic",
                        _ => "trait",
                    }
                } else {
                    "static"
                };
                if [LangItem::FnOnce, LangItem::FnMut, LangItem::Fn]
                    .iter()
                    .any(|item| self.tcx.is_lang_item(self.tcx.parent(callee), *item))
                    && let Some(receiver) = args.first()
                {
                    let mut receiver = receiver.node.ty(self.body, self.tcx);
                    while let ty::Ref(_, ty, _) = *receiver.kind() {
                        receiver = ty;
                    }
                    if let ty::Closure(closure, _) = *receiver.kind() {
                        self.resolved(closure, span, "closure");
                        return;
                    }
                }
                self.resolved(callee, span, dispatch);
            }
            ty::Closure(callee, _) => self.resolved(callee, span, "closure"),
            ty::FnPtr(..) => self.unresolved(
                span,
                "function pointer target is not statically known".into(),
            ),
            _ => self.unresolved(span, format!("unclassified callable type {ty}")),
        }
    }
}
impl<'tcx> MirVisitor<'tcx> for BodyCalls<'_, 'tcx> {
    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, location: mir::Location) {
        match &terminator.kind {
            mir::TerminatorKind::Call {
                func,
                args,
                fn_span,
                call_source,
                ..
            } => self.call(
                func,
                args,
                *fn_span,
                *call_source != mir::CallSource::Normal,
            ),
            mir::TerminatorKind::TailCall {
                func,
                args,
                fn_span,
            } => self.call(func, args, *fn_span, false),
            mir::TerminatorKind::Drop { place, .. } => {
                self.capture.coverage.implicit_drops += 1;
                if let ty::Adt(adt, _) = place.ty(self.body, self.tcx).ty.kind()
                    && let Some(destructor) = adt.destructor(self.tcx)
                {
                    self.resolved(destructor.did, terminator.source_info.span, "implicit");
                }
            }
            mir::TerminatorKind::Assert { .. } => self.capture.coverage.assertions += 1,
            mir::TerminatorKind::InlineAsm { .. } => {
                self.capture.coverage.inline_assembly += 1;
                self.unresolved(
                    terminator.source_info.span,
                    "inline assembly may call unobservable targets".into(),
                );
            }
            _ => {}
        }
        self.super_terminator(terminator, location);
    }
    fn visit_rvalue(&mut self, rvalue: &mir::Rvalue<'tcx>, location: mir::Location) {
        if let mir::Rvalue::Aggregate(kind, _) = rvalue
            && let mir::AggregateKind::Adt(definition, variant, ..) = **kind
        {
            let adt = self.tcx.adt_def(definition);
            let variant = adt.variant(variant);
            let callee = variant.ctor_def_id().unwrap_or(variant.def_id);
            let span = self.body.source_info(location).span;
            self.resolved(callee, span, "constructor");
        }
        self.super_rvalue(rvalue, location);
    }
}

struct References<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx ty::TypeckResults<'tcx>,
    caller: DefId,
    marker: bool,
    targets: BTreeSet<String>,
    invalid_targets: Vec<String>,
    capture: &'a mut Capture,
}
impl<'tcx> Visitor<'tcx> for References<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx hir::Expr<'tcx>) {
        if matches!(expression.kind, hir::ExprKind::Path(_))
            && let Some(ty) = self.typeck.expr_ty_opt(expression)
        {
            if let ty::FnDef(definition, substitutions) = *ty.kind() {
                if self.marker {
                    let resolved = ty::Instance::try_resolve(
                        self.tcx,
                        ty::TypingEnv::post_analysis(self.tcx, self.caller),
                        definition,
                        substitutions.skip_binder(),
                    )
                    .ok()
                    .flatten()
                    .map(|instance| instance.def_id())
                    .unwrap_or(definition);
                    if let Some(api) = api(self.tcx, resolved) {
                        self.targets.insert(api.id.clone());
                        self.capture.insert_api(resolved, api);
                    } else {
                        let location = source_location(self.tcx, expression.span.source_callsite());
                        self.invalid_targets.push(format!(
                            "{}:{}: test target has no workspace callable source origin",
                            location.path, location.line
                        ));
                    }
                } else if !is_infrastructure(self.tcx, self.caller) {
                    self.capture.coverage.function_references += 1;
                    edge(
                        self.tcx,
                        self.caller,
                        definition,
                        expression.span,
                        "function-reference",
                        self.capture,
                        true,
                    );
                }
            } else if self.marker {
                let location = source_location(self.tcx, expression.span.source_callsite());
                self.invalid_targets.push(format!(
                    "{}:{}: test target must resolve to a Rust function or method item, got {ty}",
                    location.path, location.line
                ));
            }
        }
        intravisit::walk_expr(self, expression);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hir::def_id::{DefIndex, LOCAL_CRATE};

    #[test]
    fn identity_collision_preserves_first_error_without_poisoning_capture() {
        let capture = Mutex::new(Capture::default());
        let definition = |index| DefId {
            krate: LOCAL_CRATE,
            index: DefIndex::from_u32(index),
        };
        let api = Api {
            id: "long-compiler-definition-path".repeat(1000),
            package: "fixture".into(),
            name: "fixture::generated".into(),
            kind: "function".into(),
            path: "src/lib.rs".into(),
            line: 1,
        };
        {
            let mut guard = capture.lock().unwrap();
            guard.insert_api(definition(1), api.clone());
            guard.insert_api(definition(2), api.clone());
        }
        assert!(!capture.is_poisoned());
        let mut guard = capture.lock().unwrap();
        let first_error = guard.error.clone().unwrap();
        assert!(first_error.len() < 500, "{first_error}");
        assert!(first_error.contains("between DefId(0:1) and DefId(0:2)"));
        for _ in 0..1000 {
            guard.insert_api(definition(3), api.clone());
        }
        let mut next = Capture::default();
        next.insert_api(definition(4), api.clone());
        next.insert_api(definition(5), api.clone());
        guard.merge(next);
        assert_eq!(guard.error.as_deref(), Some(first_error.as_str()));
        assert_eq!(guard.apis.len(), 1);
        assert_eq!(guard.apis[&api.id].0, definition(1));
        assert_eq!(guard.apis[&api.id].1.line, 1);
    }
}
