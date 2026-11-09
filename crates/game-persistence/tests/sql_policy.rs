use std::{collections::HashMap, fs, path::Path};
use syn::{
    Expr, Meta, Pat, Token,
    parse::Parser,
    punctuated::Punctuated,
    visit::{self, Visit},
};

#[derive(Clone)]
enum Source {
    Built(usize, bool),
    Literals(Vec<String>),
    Schema,
}
#[derive(Default)]
struct Policy {
    bindings: HashMap<String, Source>,
    errors: Vec<String>,
    next_pair: usize,
    wrapper: bool,
}

fn name(path: &syn::Path) -> String {
    path.segments.last().unwrap().ident.to_string()
}
fn bare(expr: &Expr) -> &Expr {
    match expr {
        Expr::Reference(e) => bare(&e.expr),
        Expr::Paren(e) => bare(&e.expr),
        _ => expr,
    }
}
fn literal(expr: &Expr) -> Option<String> {
    match bare(expr) {
        Expr::Lit(e) => match &e.lit {
            syn::Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        _ => None,
    }
}
fn format_args(expr: &Expr) -> Vec<Expr> {
    match bare(expr) {
        Expr::Macro(e) if e.mac.path.is_ident("format") => e
            .mac
            .parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
            .map(|a| a.into_iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}
fn cfg_without_tests(meta: &Meta) -> Option<bool> {
    match meta {
        Meta::Path(p) if p.is_ident("test") => Some(false),
        Meta::List(list) => {
            let parts = Punctuated::<Meta, Token![,]>::parse_terminated
                .parse2(list.tokens.clone())
                .ok()?;
            let states: Vec<_> = parts.iter().map(cfg_without_tests).collect();
            if list.path.is_ident("not") && states.len() == 1 {
                states[0].map(|v| !v)
            } else if list.path.is_ident("all") {
                if states.contains(&Some(false)) {
                    Some(false)
                } else if states.iter().all(|v| *v == Some(true)) {
                    Some(true)
                } else {
                    None
                }
            } else if list.path.is_ident("any") {
                if states.contains(&Some(true)) {
                    Some(true)
                } else if states.iter().all(|v| *v == Some(false)) {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}
fn excluded(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && a.parse_args::<Meta>()
                .ok()
                .is_some_and(|m| cfg_without_tests(&m) == Some(false))
    })
}
fn postgres_sql(sql: &str) -> bool {
    let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if matches!(
        normalized.as_str(),
        "SET default_transaction_read_only = on"
            | "SET statement_timeout = '2s'"
            | "SET lock_timeout = '2s'"
            | "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY"
            | "SET LOCAL search_path TO public"
            | "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))"
    ) {
        return true;
    }
    if normalized.contains(';') || normalized.contains("--") || normalized.contains("/*") {
        return false;
    }
    for prefix in [
        "SELECT pg_advisory_xact_lock(",
        "SELECT pg_try_advisory_lock(",
    ] {
        if let Some(argument) = normalized
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix(')'))
        {
            return !argument.is_empty()
                && argument.chars().all(|c| c.is_ascii_digit() || c == '$');
        }
    }
    if !normalized.starts_with("SELECT ") {
        return false;
    }
    let separated = normalized
        .replace(',', " , ")
        .replace('(', " ( ")
        .replace(')', " ) ");
    let tokens: Vec<_> = separated
        .split(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '_' | '.' | ',' | '(' | ')'))
        .filter(|s| !s.is_empty())
        .collect();
    let mut relations = 0;
    let mut relation_list = false;
    let mut expect_relation = false;
    for token in tokens {
        let keyword = token.to_ascii_uppercase();
        if matches!(
            keyword.as_str(),
            "INSERT" | "UPDATE" | "DELETE" | "INTO" | "COPY" | "CALL"
        ) {
            return false;
        }
        if expect_relation {
            expect_relation = false;
            if token == "(" {
                relation_list = false;
                continue;
            }
            if !matches!(
                token,
                "information_schema.tables"
                    | "information_schema.columns"
                    | "pg_class"
                    | "pg_namespace"
                    | "pg_attribute"
                    | "pg_attrdef"
                    | "pg_collation"
                    | "pg_constraint"
                    | "pg_index"
            ) {
                return false;
            }
            relations += 1;
        } else if matches!(keyword.as_str(), "FROM" | "JOIN") {
            relation_list = true;
            expect_relation = true;
        } else if matches!(
            keyword.as_str(),
            "ON" | "WHERE" | "GROUP" | "ORDER" | "UNION" | ")"
        ) {
            relation_list = false;
        } else if relation_list && token == "," {
            expect_relation = true;
        }
    }
    relations > 0
}
impl Policy {
    fn fail(&mut self, site: &str) {
        self.errors.push(format!(
            "{site}: use SeaQuery with bound values or an explicitly reviewed PostgreSQL exception"
        ));
    }
    fn source(&self, expr: &Expr) -> Option<Source> {
        match bare(expr) {
            Expr::Path(p) => self.bindings.get(&name(&p.path)).cloned(),
            _ => None,
        }
    }
    fn sql(&self, expr: &Expr) -> bool {
        if let Some(sql) = literal(expr) {
            return postgres_sql(&sql);
        }
        if let Some(Source::Literals(sql)) = self.source(expr) {
            return sql.iter().all(|s| postgres_sql(s));
        }
        let values = format_args(expr);
        if values.len() == 1
            && matches!(
                literal(&values[0]).as_deref(),
                Some(
                    "CREATE SCHEMA {expected_schema}"
                        | "DROP SCHEMA {expected_schema} CASCADE"
                        | "SET LOCAL search_path TO {expected_schema}"
                )
            )
        {
            return matches!(self.bindings.get("expected_schema"), Some(Source::Schema));
        }
        false
    }
    fn built(&self, expr: &Expr) -> Option<(usize, bool)> {
        let expr = match bare(expr) {
            Expr::Call(c)
                if matches!(&*c.func, Expr::Path(p) if name(&p.path) == "AssertSqlSafe")
                    && c.args.len() == 1 =>
            {
                &c.args[0]
            }
            e => e,
        };
        match self.source(expr) {
            Some(Source::Built(id, sql)) => Some((id, sql)),
            _ => None,
        }
    }
}
impl<'ast> Visit<'ast> for Policy {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let attrs = match item {
            syn::Item::Fn(i) => &i.attrs,
            syn::Item::Mod(i) => &i.attrs,
            syn::Item::Impl(i) => &i.attrs,
            syn::Item::Const(i) => &i.attrs,
            syn::Item::Static(i) => &i.attrs,
            syn::Item::Macro(i) => &i.attrs,
            _ => return visit::visit_item(self, item),
        };
        if !excluded(attrs) {
            visit::visit_item(self, item);
        }
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::take(&mut self.bindings);
        let wrapper = self.wrapper;
        self.wrapper = item.sig.ident == "postgres_statement"
            && item.sig.inputs.len() == 2
            && item.block.stmts.len() == 1
            && matches!(&item.block.stmts[0], syn::Stmt::Expr(Expr::Call(c), None)
                if matches!(&*c.func, Expr::Path(p) if p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>() == ["Statement", "from_sql_and_values"])
                && c.args.len() == 3);
        visit::visit_item_fn(self, item);
        self.bindings = previous;
        self.wrapper = wrapper;
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if excluded(&item.attrs) {
            return;
        }
        let previous = std::mem::take(&mut self.bindings);
        visit::visit_impl_item_fn(self, item);
        self.bindings = previous;
    }
    fn visit_block(&mut self, block: &'ast syn::Block) {
        let previous = self.bindings.clone();
        visit::visit_block(self, block);
        self.bindings = previous;
    }
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if excluded(&local.attrs) {
            return;
        }
        visit::visit_local(self, local);
        let Some(init) = &local.init else {
            return;
        };
        if let (Pat::Tuple(pat), Expr::MethodCall(call)) = (&local.pat, bare(&init.expr))
            && call.method == "build_sqlx"
            && pat.elems.len() == 2
        {
            self.next_pair += 1;
            for (index, pat) in pat.elems.iter().enumerate() {
                if let Pat::Ident(p) = pat {
                    self.bindings.insert(
                        p.ident.to_string(),
                        Source::Built(self.next_pair, index == 0),
                    );
                }
            }
        }
        let values = format_args(&init.expr);
        if let Pat::Ident(p) = &local.pat {
            self.bindings.remove(&p.ident.to_string());
            if values.len() == 2
                && matches!(
                    literal(&values[0]).as_deref(),
                    Some("account_expected_{}" | "game_expected_{}")
                )
                && let Expr::MethodCall(simple) = &values[1]
                && simple.method == "simple"
                && simple.args.is_empty()
                && let Expr::Call(uuid) = &*simple.receiver
                && matches!(&*uuid.func, Expr::Path(p) if p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>() == ["uuid", "Uuid", "new_v4"])
                && uuid.args.is_empty()
            {
                self.bindings.insert(p.ident.to_string(), Source::Schema);
            }
        }
    }
    fn visit_expr_for_loop(&mut self, expr: &'ast syn::ExprForLoop) {
        self.visit_expr(&expr.expr);
        let previous = self.bindings.clone();
        if let (Pat::Ident(p), Expr::Array(array)) = (&*expr.pat, bare(&expr.expr))
            && let Some(values) = array.elems.iter().map(literal).collect::<Option<Vec<_>>>()
        {
            self.bindings
                .insert(p.ident.to_string(), Source::Literals(values));
        }
        self.visit_block(&expr.body);
        self.bindings = previous;
    }
    fn visit_expr_block(&mut self, expr: &'ast syn::ExprBlock) {
        if !excluded(&expr.attrs) {
            visit::visit_expr_block(self, expr);
        }
    }
    fn visit_expr_assign(&mut self, expr: &'ast syn::ExprAssign) {
        if let Expr::Path(p) = bare(&expr.left) {
            self.bindings.remove(&name(&p.path));
        }
        visit::visit_expr_assign(self, expr);
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*call.func {
            let function = name(&path.path);
            let sql_arg = match function.as_str() {
                "query" | "query_as" | "query_scalar" | "raw_sql" | "postgres_statement" => {
                    call.args.first()
                }
                "from_string" | "from_sql_and_values" => call.args.iter().nth(1),
                _ => None,
            };
            if let Some(sql) = sql_arg {
                let delegated = self.wrapper
                    && function == "from_sql_and_values"
                    && matches!(bare(sql), Expr::Path(p) if p.path.is_ident("sql"))
                    && matches!(call.args.iter().nth(2), Some(Expr::Path(p)) if p.path.is_ident("values"));
                if !delegated && !self.sql(sql) {
                    self.fail(&function);
                }
            }
            if matches!(
                function.as_str(),
                "query_with" | "query_as_with" | "query_scalar_with"
            ) {
                let pair = call.args.first().and_then(|e| self.built(e));
                let values = call.args.iter().nth(1).and_then(|e| self.built(e));
                if !matches!((pair, values), (Some((a, true)), Some((b, false))) if a == b) {
                    self.fail(&function);
                }
            }
            if function.starts_with("cust")
                && call.args.first().and_then(literal).as_deref() != Some("id COLLATE \"C\"")
            {
                self.fail(&function);
            }
        }
        visit::visit_expr_call(self, call);
    }
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "execute_unprepared" && !call.args.first().is_some_and(|e| self.sql(e)) {
            self.fail("execute_unprepared");
        }
        if matches!(
            call.method.to_string().as_str(),
            "execute_raw" | "query_one_raw" | "query_all_raw"
        ) {
            let checked = matches!(call.args.first().map(bare), Some(Expr::Call(c)) if matches!(&*c.func, Expr::Path(p) if matches!(name(&p.path).as_str(), "postgres_statement" | "from_string" | "from_sql_and_values")));
            if !checked {
                self.fail(&call.method.to_string());
            }
        }
        if matches!(self.source(&call.receiver), Some(Source::Built(_, _))) {
            self.fail("method on generated SQL or bindings");
        }
        visit::visit_expr_method_call(self, call);
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if matches!(
            name(&mac.path).as_str(),
            "include" | "include_str" | "include_bytes" | "query" | "query_as" | "query_scalar"
        ) {
            self.fail(&name(&mac.path));
        }
        visit::visit_macro(self, mac);
    }
}
fn violations(source: &str) -> Vec<String> {
    let mut policy = Policy::default();
    policy.visit_file(&syn::parse_file(source).expect("valid Rust source"));
    policy.errors
}
fn inspect(root: &Path, path: &Path, errors: &mut Vec<String>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            inspect(root, &path, errors);
        } else if path.extension().is_some_and(|e| e == "rs") {
            errors.extend(
                violations(&fs::read_to_string(&path).unwrap())
                    .into_iter()
                    .map(|e| format!("{}: {e}", path.strip_prefix(root).unwrap().display())),
            );
        }
    }
}
#[test]
fn production_sql_uses_builders_or_reviewed_postgres_operations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut errors = Vec::new();
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let path = entry.unwrap().path();
        if path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-persistence")
        {
            inspect(root, &path.join("src"), &mut errors);
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
#[test]
fn policy_rejects_application_sql_and_interpolation() {
    for source in [
        r#"fn f() { sqlx::query("SELECT * FROM games"); }"#,
        r#"fn f() { sqlx::query("SELECT * FROM accounts WHERE id = $1").bind(id); }"#,
        r#"fn f() { sqlx::query("INSERT INTO accounts VALUES ($1)").bind(id); }"#,
        r#"fn f() { db.execute_unprepared(&format!("SELECT * FROM games WHERE id = '{id}'")); }"#,
        r#"fn f() { postgres_statement(&format!("CREATE SCHEMA {expected_schema}"), vec![]); }"#,
        r#"fn f() { let expected_schema = format!("game_expected_{}", user); postgres_statement(&format!("CREATE SCHEMA {expected_schema}"), vec![]); }"#,
        r#"fn f() { let (sql, values) = query.build_sqlx(PostgresQueryBuilder); let sql = format!("{sql} {user}"); sqlx::query_with(AssertSqlSafe(sql), values); }"#,
        r#"fn f() { sqlx::query("SELECT * FROM pg_class JOIN accounts ON true"); }"#,
        r#"fn f() { sqlx::query("SELECT * FROM pg_class, accounts"); }"#,
        r#"fn f() { sqlx::query("SELECT * FROM pg_class join accounts ON true"); }"#,
        r#"fn f() { db.execute_raw(make_statement(user)); }"#,
        r#"fn f() { let (sql, values) = query.build_sqlx(PostgresQueryBuilder); sql.push_str(user); sqlx::query_with(AssertSqlSafe(sql), values); }"#,
        r#"fn f() { include!("sql.rs"); }"#,
        r#"#[cfg(any(test, feature = "production"))] fn f() { sqlx::query("SELECT * FROM games"); }"#,
    ] {
        assert!(!violations(source).is_empty(), "accepted {source}");
    }
}
#[test]
fn policy_accepts_builders_bound_catalog_queries_and_test_fixtures() {
    for source in [
        r#"fn f() { let (sql, values) = query.build_sqlx(PostgresQueryBuilder); sqlx::query_with(AssertSqlSafe(sql), values); }"#,
        r#"fn f() { sqlx::query("SELECT table_type FROM information_schema.tables WHERE table_name = $1").bind(table); }"#,
        r#"fn f() { sqlx::query_scalar("SELECT pg_try_advisory_lock(123)"); Expr::cust("id COLLATE \"C\""); }"#,
        r#"fn f() { let expected_schema = format!("game_expected_{}", uuid::Uuid::new_v4().simple()); db.execute_raw(Statement::from_string(DbBackend::Postgres, format!("CREATE SCHEMA {expected_schema}"))); }"#,
        r#"#[cfg(all(test, feature = "faults"))] mod tests { fn f() { sqlx::query("DELETE FROM accounts"); include_str!("legacy.sql"); } }"#,
        r#"fn f() { #[cfg(test)] { sqlx::query("DELETE FROM games"); } }"#,
    ] {
        assert!(violations(source).is_empty(), "rejected {source}");
    }
}
