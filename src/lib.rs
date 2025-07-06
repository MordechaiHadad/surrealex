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
    base_select: String,
    graph_expansions: Vec<String>,
    traverse_clauses: Vec<String>,
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
        Self {
            base_select: "*".to_string(),
            ..Default::default()
        }
    }

    /// Sets the base fields to select (e.g., "id, title"). Defaults to "*".
    pub fn select(&mut self, fields: &str) -> &mut Self {
        self.base_select = fields.to_string();
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
    pub fn r#where(&mut self, condition: &str) -> &mut Self {
        self.where_clauses.push(Condition::Simple(condition.to_string()));
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
    
    /// Assembles all the pieces into a final SurrealQL query string.
    pub fn build(&self) -> Result<String, &'static str> {
        let from_table = self.from_table.as_ref().ok_or("The FROM clause is required.")?;

        let mut all_selects = vec![self.base_select.clone()];
        all_selects.extend(self.graph_expansions.iter().cloned());
        let final_select_clause = all_selects.join(", ");

        let mut query = format!("SELECT {} FROM {}", final_select_clause, from_table);
        // insert any graph_traverse clauses after FROM
        if !self.traverse_clauses.is_empty() {
            for clause in &self.traverse_clauses {
                query.push(' ');
                query.push_str(clause);
            }
        }
        // then apply WHERE clause
        if !self.where_clauses.is_empty() {
            let rendered: Vec<String> = self.where_clauses.iter().map(|c| render_condition(c)).collect();
            query.push_str(" WHERE ");
            query.push_str(&rendered.join(" AND "));
        }
        // then apply FETCH, etc.
        if !self.fetch_clauses.is_empty() {
            query.push_str(" FETCH ");
            query.push_str(&self.fetch_clauses.join(", "));
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
        
        Ok(query)
    }

    /// Add a two-step graph traversal with optional alias.
    pub fn graph_traverse(&mut self, params: GraphExpandParams) -> &mut Self {
        let mut clause = String::new();
        let (ref dir1, ref tbl1) = params.from;
        clause.push_str(match dir1 { Direction::Out => "->", Direction::In => "<-" });
        clause.push_str(tbl1);
        // always expand outgoing from intermediate results
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
        let sql = QueryBuilder::new()
            .from("user")
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT * FROM user");
    }

    #[test]
    fn select_where_order_limit_start() {
        let sql = QueryBuilder::new()
            .select("id, name")
            .from("user")
            .r#where("active = true")
            .order_by("name ASC")
            .limit(5)
            .start(10)
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT id, name FROM user WHERE active = true ORDER BY name ASC LIMIT 5 START 10");
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
            .select("foo")
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
            .r#where("x = 1")
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
            .select("a")
            .from("t")
            .r#where("w")
            .fetch("f")
            .graph_expand("g")
            .order_by("o")
            .limit(1)
            .start(2)
            .build()
            .unwrap();
        assert_eq!(sql, "SELECT a, g FROM t WHERE w FETCH f ORDER BY o LIMIT 1 START 2");
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
}