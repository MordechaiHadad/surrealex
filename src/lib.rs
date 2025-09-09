/// Represents a logical condition or a group of conditions for a WHERE clause.
/// This enum allows for building a tree of logical operations.
#[derive(Debug, Clone)]
pub enum Condition {
    /// A simple, raw condition string (e.g., "price > 50").
    Simple(String),
    /// A list of conditions that will be joined by 'AND'.
    And(Vec<Condition>),
    /// A list of conditions that will be joined by 'OR'.
    Or(Vec<Condition>),
}

/// A helper function that recursively renders a `Condition` tree into a SQL string.
fn render_condition(condition: &Condition) -> String {
    match condition {
        Condition::Simple(s) => s.clone(),
        Condition::And(conditions) => {
            let rendered: Vec<String> = conditions.iter().map(render_condition).collect();
            // Wrap in parentheses to ensure correct precedence when nested.
            format!("({})", rendered.join(" AND "))
        }
        Condition::Or(conditions) => {
            let rendered: Vec<String> = conditions.iter().map(render_condition).collect();
            // Parentheses are crucial for OR groups.
            format!("({})", rendered.join(" OR "))
        }
    }
}

#[derive(Debug, Default)]
pub struct QueryBuilder {
    /// SELECT items (defaults to ["*"])
    select_items: Vec<String>,
    graph_expansions: Vec<String>,
    traverse_clauses: Vec<String>,
    /// Whether to include DISTINCT in the SELECT clause.
    distinct: bool,
    from_table: Option<String>,
    fetch_clauses: Vec<String>,
    where_clauses: Vec<Condition>,
    order_by: Vec<String>,
    limit: Option<u64>,
    start: Option<u64>,
}

impl QueryBuilder {
    /// Creates a new `QueryBuilder` instance, defaulting to `SELECT *`.
    pub fn new() -> Self {
        let mut qb = Self {
            select_items: vec!["*".to_string()],
            ..Default::default()
        };
        qb.distinct = false;
        qb
    }

    /// Adds a field or expression to select, with optional alias.
    /// Example: `.select("col", Some("alias"))` yields `col AS alias`.
    pub fn select(&mut self, expr: &str, alias: Option<&str>) -> &mut Self {
        // clear default '*' on first custom select
        if self.select_items.len() == 1 && self.select_items[0] == "*" {
            self.select_items.clear();
        }
        let item = if let Some(a) = alias {
            format!("{} AS {}", expr, a)
        } else {
            expr.to_string()
        };
        self.select_items.push(item);
        self
    }

    /// Sets the table to select data FROM. This is a required clause.
    pub fn from(&mut self, table: &str) -> &mut Self {
        self.from_table = Some(table.to_string());
        self
    }

    /// Adds a field to the FETCH clause. Can be called multiple times.
    pub fn fetch(&mut self, field: &str) -> &mut Self {
        self.fetch_clauses.push(field.to_string());
        self
    }

    /// Adds a graph traversal or complex projection to the SELECT list.
    pub fn graph_expand(&mut self, expansion_clause: &str) -> &mut Self {
        self.graph_expansions.push(expansion_clause.to_string());
        self
    }

    /// A convenience shortcut to add a simple, raw condition string.
    /// This is equivalent to `add_condition(Condition::Simple(...))`.
    pub fn where_simple(&mut self, condition: &str) -> &mut Self {
        self.where_clauses
            .push(Condition::Simple(condition.to_string()));
        self
    }

    /// Adds a complex `Condition` to the WHERE clause. All top-level
    /// conditions are joined by AND.
    pub fn where_complex(&mut self, condition: Condition) -> &mut Self {
        self.where_clauses.push(condition);
        self
    }

    /// Adds an ORDER BY clause. Can be called multiple times.
    pub fn order_by(&mut self, field_and_direction: &str) -> &mut Self {
        self.order_by.push(field_and_direction.to_string());
        self
    }

    /// Sets the LIMIT clause.
    pub fn limit(&mut self, count: u64) -> &mut Self {
        self.limit = Some(count);
        self
    }

    /// Sets the START (offset) clause.
    pub fn start(&mut self, offset: u64) -> &mut Self {
        self.start = Some(offset);
        self
    }

    /// Enables DISTINCT in the SELECT clause.
    pub fn distinct(&mut self) -> &mut Self {
        self.distinct = true;
        self
    }

    pub fn build(&self) -> Result<String, &'static str> {
        let from_table = self
            .from_table
            .as_ref()
            .ok_or("The FROM clause is required.")?;

        let mut all_selects = self.select_items.clone();
        all_selects.extend(self.graph_expansions.iter().cloned());
        let final_select_clause = all_selects.join(", ");

        let mut query = if self.distinct {
            format!(
                "SELECT DISTINCT {} FROM {}",
                final_select_clause, from_table
            )
        } else {
            format!("SELECT {} FROM {}", final_select_clause, from_table)
        };
        if !self.traverse_clauses.is_empty() {
            for clause in &self.traverse_clauses {
                query.push(' ');
                query.push_str(clause);
            }
        }

        if !self.where_clauses.is_empty() {
            let rendered: Vec<String> = self
                .where_clauses
                .iter()
                .map(|c| render_condition(c))
                .collect();
            query.push_str(" WHERE ");
            query.push_str(&rendered.join(" AND "));
        }

        if !self.order_by.is_empty() {
            query.push_str(" ORDER BY ");
            query.push_str(&self.order_by.join(", "));
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(start) = self.start {
            query.push_str(&format!(" START {}", start));
        }

        if !self.fetch_clauses.is_empty() {
            query.push_str(" FETCH ");
            query.push_str(&self.fetch_clauses.join(", "));
        }

        Ok(query)
    }

    /// Add a two-step graph traversal with optional alias.
    pub fn graph_traverse(&mut self, params: GraphExpandParams) -> &mut Self {
        let mut clause = String::new();
        let (ref dir1, ref tbl1) = params.from;
        clause.push_str(match dir1 {
            Direction::Out => "->",
            Direction::In => "<-",
        });
        clause.push_str(tbl1);
        let tbl2 = &params.to.1;
        clause.push_str("->");
        clause.push_str(tbl2);
        clause.push_str(".*");
        if let Some(ref a) = params.alias {
            clause.push_str(" AS ");
            clause.push_str(a);
        }
        self.traverse_clauses.push(clause);
        self
    }
}

/// Helper to build a SurrealQL script composed of `LET` assignments and a final `RETURN` object.
///
/// Example:
/// ```rust,ignore
/// let mut sb = ScriptBuilder::new();
/// let q = QueryBuilder::new().from("widget").where_simple("active = true");
/// sb.let_query("widgets", &q).unwrap();
/// sb.returning(vec![("items", "$widgets")]);
/// ```
#[derive(Debug, Default)]
pub struct ScriptBuilder {
    statements: Vec<String>,
    return_map: Option<Vec<(String, String)>>,
}

impl ScriptBuilder {
    /// Create a new empty script builder.
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
            return_map: None,
        }
    }

    /// Add a raw LET assignment where the expression is wrapped in parentheses.
    /// Example: let $name = (SELECT * FROM t WHERE ...);
    pub fn let_raw(&mut self, name: &str, expr: &str) -> &mut Self {
        let s = format!("LET ${} = ({});", name, expr);
        self.statements.push(s);
        self
    }

    /// Add a LET assignment where the expression is wrapped in parentheses and
    /// a suffix (like an index or field access) is appended outside the
    /// parentheses. Example suffix: "[0].count" -> (SELECT ...)[0].count
    pub fn let_raw_with_suffix(&mut self, name: &str, expr: &str, suffix: &str) -> &mut Self {
        let s = format!("LET ${} = ({}){};", name, expr, suffix);
        self.statements.push(s);
        self
    }

    /// Accept a `QueryBuilder`, build its query string and create a LET
    /// assignment using the built query. Returns Err if the inner query
    /// cannot be built.
    pub fn let_query(&mut self, name: &str, qb: &QueryBuilder) -> Result<&mut Self, &'static str> {
        let q = qb.build()?;
        Ok(self.let_raw(name, &q))
    }

    /// Same as `let_query` but allows appending a suffix (for indexing / field access)
    /// outside the parenthesized expression.
    pub fn let_query_with_suffix(
        &mut self,
        name: &str,
        qb: &QueryBuilder,
        suffix: &str,
    ) -> Result<&mut Self, &'static str> {
        let q = qb.build()?;
        Ok(self.let_raw_with_suffix(name, &q, suffix))
    }

    /// Provide the return mapping as a list of (key, value) pairs. Values are
    /// verbatim strings (e.g. `$product` or an expression).
    pub fn returning(&mut self, map: Vec<(&str, &str)>) -> &mut Self {
        self.return_map = Some(
            map.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        self
    }

    /// Build the final script string.
    pub fn build(&self) -> Result<String, &'static str> {
        let ret = match &self.return_map {
            Some(m) if !m.is_empty() => m,
            _ => return Err("A return object is required."),
        };

        let mut out = String::new();
        for st in &self.statements {
            out.push_str(st);
            out.push('\n');
        }

        out.push_str("RETURN { ");
        let pairs: Vec<String> = ret.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
        out.push_str(&pairs.join(", "));
        out.push_str(" }");
        Ok(out)
    }
}

/// Builder for SurrealQL transactions.
///
/// Usage: create a TransactionBuilder, call `begin()`, add statements (raw strings,
/// queries from `QueryBuilder`, or full `ScriptBuilder` scripts), then `commit()` or
/// `cancel()` and `build()` to get the final SurrealQL transaction script.
#[derive(Debug, Default)]
pub struct TransactionBuilder {
    statements: Vec<String>,
}

impl TransactionBuilder {
    /// Create a new empty transaction builder.
    pub fn new() -> Self {
        Self { statements: Vec::new() }
    }

    /// Start the transaction block. Uses `BEGIN TRANSACTION;`.
    pub fn begin(&mut self) -> &mut Self {
        self.statements.push("BEGIN TRANSACTION;".to_string());
        self
    }

    /// Add a raw statement (will be terminated with a semicolon if missing).
    pub fn add_statement(&mut self, stmt: &str) -> &mut Self {
        let s = stmt.trim();
        if s.ends_with(';') {
            self.statements.push(s.to_string());
        } else {
            self.statements.push(format!("{};", s));
        }
        self
    }

    /// Add a `QueryBuilder`'s built query as a statement.
    pub fn add_query(&mut self, qb: &QueryBuilder) -> Result<&mut Self, &'static str> {
        let q = qb.build()?;
        Ok(self.add_statement(&q))
    }

    /// Add a `QueryBuilder`'s built query with a suffix (e.g., `[0].count`).
    pub fn add_query_with_suffix(&mut self, qb: &QueryBuilder, suffix: &str) -> Result<&mut Self, &'static str> {
        let q = qb.build()?;
        Ok(self.add_statement(&format!("({}){}", q, suffix)))
    }

    /// Add an entire `ScriptBuilder` script (it may contain multiple lines).
    pub fn add_script(&mut self, script: &str) -> &mut Self {
        // push verbatim; the script may contain its own semicolons and newlines
        self.statements.push(script.to_string());
        self
    }

    /// Add a COMMIT statement. Use this to finalise the transaction.
    pub fn commit(&mut self) -> &mut Self {
        self.statements.push("COMMIT TRANSACTION;".to_string());
        self
    }

    /// Add a CANCEL statement. Use this to rollback the transaction.
    pub fn cancel(&mut self) -> &mut Self {
        self.statements.push("CANCEL TRANSACTION;".to_string());
        self
    }

    /// Build the final transaction script as a single string.
    pub fn build(&self) -> String {
        self.statements.join("\n")
    }
}

/// Direction of graph traversal arrows.
#[derive(Debug, Clone)]
pub enum Direction {
    /// Outgoing (`->`).
    Out,
    /// Incoming (`<-`).
    In,
}

/// Parameters for a two-step graph traversal expansion.
#[derive(Debug, Clone)]
pub struct GraphExpandParams {
    /// First traversal (direction and graph table).
    pub from: (Direction, String),
    /// Second traversal (direction and edge table).
    pub to: (Direction, String),
    /// Optional alias for the expansion.
    pub alias: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_select_from() {
        let sql = QueryBuilder::new().from("user").build().unwrap();
        assert_eq!(sql, "SELECT * FROM user");
    }

    #[test]
    fn select_where_order_limit_start() {
        let sql = QueryBuilder::new()
            .select("id, name", None)
            .from("user")
            .where_simple("active = true")
            .order_by("name ASC")
            .limit(5)
            .start(10)
            .build()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT id, name FROM user WHERE active = true ORDER BY name ASC LIMIT 5 START 10"
        );
    }

    #[test]
    fn fetch_and_graph_expand() {
        let sql = QueryBuilder::new()
            .from("post")
            .fetch("comments")
            .graph_expand("likes")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT *, likes FROM post FETCH comments");
    }

    #[test]
    fn complex_where_conditions() {
        let cond = Condition::And(vec![
            Condition::Simple("a = 1".into()),
            Condition::Or(vec![
                Condition::Simple("b = 2".into()),
                Condition::Simple("c = 3".into()),
            ]),
        ]);
        let sql = QueryBuilder::new()
            .from("t")
            .where_complex(cond)
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE (a = 1 AND (b = 2 OR c = 3))");
    }

    #[test]
    fn missing_from_clause() {
        let err = QueryBuilder::new().build().unwrap_err();
        assert_eq!(err, "The FROM clause is required.");
    }

    #[test]
    fn multi_fetch_and_graph_expand() {
        let sql = QueryBuilder::new()
            .from("tbl")
            .fetch("a")
            .fetch("b")
            .graph_expand("x")
            .graph_expand("y")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT *, x, y FROM tbl FETCH a, b");
    }

    #[test]
    fn select_and_graph_expand() {
        let sql = QueryBuilder::new()
            .select("foo", None)
            .graph_expand("bar")
            .from("t")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT foo, bar FROM t");
    }

    #[test]
    fn where_and_where_complex_equal() {
        let simple = QueryBuilder::new()
            .from("t")
            .where_simple("x = 1")
            .build()
            .unwrap();
        let complex = QueryBuilder::new()
            .from("t")
            .where_complex(Condition::Simple("x = 1".into()))
            .build()
            .unwrap();
        assert_eq!(simple, complex);
    }

    #[test]
    fn full_chaining_all_clauses() {
        let sql = QueryBuilder::new()
            .select("a", None)
            .from("t")
            .where_simple("w")
            .fetch("f")
            .graph_expand("g")
            .order_by("o")
            .limit(1)
            .start(2)
            .build()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT a, g FROM t WHERE w ORDER BY o LIMIT 1 START 2 FETCH f"
        );
    }

    #[test]
    fn graph_traverse_example() {
        let sql = QueryBuilder::new()
            .from("user")
            .graph_traverse(GraphExpandParams {
                from: (Direction::Out, "friends".into()),
                to: (Direction::In, "posts".into()),
                alias: Some("friend_posts".into()),
            })
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT * FROM user ->friends->posts.* AS friend_posts");
    }

    #[test]
    fn graph_traverse_in_out() {
        let sql = QueryBuilder::new()
            .from("x")
            .graph_traverse(GraphExpandParams {
                from: (Direction::In, "t".into()),
                to: (Direction::Out, "e".into()),
                alias: None,
            })
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT * FROM x <-t->e.*");
    }

    #[test]
    fn distinct_basic() {
        let sql = QueryBuilder::new().distinct().from("user").build().unwrap();
        assert_eq!(sql, "SELECT DISTINCT * FROM user");
    }

    #[test]
    fn distinct_with_fields() {
        let sql = QueryBuilder::new()
            .distinct()
            .select("id, name", None)
            .from("user")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT DISTINCT id, name FROM user");
    }

    #[test]
    fn distinct_with_graph_expand() {
        let sql = QueryBuilder::new()
            .distinct()
            .select("foo", None)
            .graph_expand("bar")
            .from("t")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT DISTINCT foo, bar FROM t");
    }

    #[test]
    fn multiple_selects() {
        let sql = QueryBuilder::new()
            .select("id", None)
            .select("name", None)
            .from("users")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT id, name FROM users");
    }

    #[test]
    fn select_with_alias() {
        let sql = QueryBuilder::new()
            .select("user_id", Some("uid"))
            .from("accounts")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT user_id AS uid FROM accounts");
    }

    #[test]
    fn mixed_select_alias_and_plain() {
        let sql = QueryBuilder::new()
            .select("user_id", Some("uid"))
            .select("name", None)
            .from("accounts")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT user_id AS uid, name FROM accounts");
    }

    #[test]
    fn script_builder_example() {
        // Build inner queries with QueryBuilder for `widget`
        let mut qb1 = QueryBuilder::new();
        qb1.from("widget").where_simple("status != \"archived\"");
        let mut qb2 = QueryBuilder::new();
        qb2.select("count()", None)
            .from("widget")
            .where_simple("status != \"archived\"");

        let mut sb = super::ScriptBuilder::new();
        sb.let_query("widget_list", &qb1)
            .unwrap()
            .let_query_with_suffix("widget_count", &qb2, "[0].count")
            .unwrap()
            .returning(vec![
                ("widgets", "$widget_list"),
                ("count", "$widget_count"),
            ]);

        let script = sb.build().unwrap();
        let expected = "LET $widget_list = (SELECT * FROM widget WHERE status != \"archived\");\nLET $widget_count = (SELECT count() FROM widget WHERE status != \"archived\")[0].count;\nRETURN { widgets: $widget_list, count: $widget_count }";
        assert_eq!(script, expected);
    }

    #[test]
    fn transaction_builder_commit_example() {
        let mut qb_create1 = QueryBuilder::new();
        qb_create1.from("widget:one").select("", None); // will produce SELECT * FROM widget:one but used as example

        let mut tb = super::TransactionBuilder::new();
        tb.begin()
            .add_statement("CREATE widget:one SET value = 100")
            .add_statement("CREATE widget:two SET value = 50")
            .add_statement("UPDATE widget:one SET value += 10")
            .add_statement("UPDATE widget:two SET value -= 10")
            .commit();

        let script = tb.build();
        let expected_start = "BEGIN TRANSACTION;\nCREATE widget:one SET value = 100;";
        assert!(script.starts_with(expected_start));
        assert!(script.contains("COMMIT TRANSACTION;"));
    }

    #[test]
    fn transaction_builder_cancel_example() {
        let mut tb = super::TransactionBuilder::new();
        tb.begin()
            .add_statement("CREATE widget:one SET value = 100")
            .add_statement("UPDATE widget:one SET value -= 200")
            .cancel();

        let script = tb.build();
        assert!(script.contains("CANCEL TRANSACTION;"));
        assert!(script.contains("CREATE widget:one SET value = 100;"));
    }
}
