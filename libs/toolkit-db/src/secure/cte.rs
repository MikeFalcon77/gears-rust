//! Common Table Expression (`WITH`) support for the Secure ORM.
//!
//! Implements [ADR-0001](../../../../docs/arch/secure-orm/ADR/0001-secure-cte-policy.md).
//!
//! # Why this is a separate type
//!
//! A CTE body is an independent `SELECT` over arbitrary tables, so the outer
//! query's scope `WHERE` does **not** reach inside it. Left unchecked that is a
//! direct tenant-isolation hole. The rule this module enforces is therefore
//! *not* "scope the outer query" but **"embed scope inside the body of every
//! CTE"** — then any table a CTE touches is already filtered.
//!
//! Isolation is structural rather than checked at runtime: a [`SecureCteSelect`]
//! is reachable only from [`SecureSelect<E, Scoped>::with_ctes`], and every CTE
//! registered on it is scoped with *that query's* `Arc<AccessScope>`. There is no
//! way to hand it a body carrying a different scope, so there is nothing to
//! compare and no error case to return.
//!
//! # Why it cannot reuse `Select<E>`
//!
//! `SecureSelect` wraps a [`sea_orm::Select<E>`], which has no `.with()` —
//! `WithClause`/`CommonTableExpression` live in `sea_query`. So a CTE query
//! cannot be a `Select<E>` at all, and executing one has to go through
//! `FromQueryResult::find_by_statement` instead of `Select<E>::all()`. Returning
//! `Self` from `with_ctes` would let the `WITH` clause silently vanish at
//! execution time; returning a distinct type makes that unrepresentable.
//!
//! # Example
//!
//! ```rust,ignore
//! // Rows of `order` that have at least one line item in the same scope.
//! let rows = order::Entity::find()
//!     .secure()
//!     .scope_with(&scope)
//!     .with_ctes()
//!     .cte::<line_item::Entity>("scoped_items", |q| {
//!         q.filter(line_item::Column::Quantity.gt(0))
//!     })
//!     .join_cte(
//!         "scoped_items",
//!         Condition::all().add(
//!             Expr::col((Alias::new("scoped_items"), Alias::new("order_id")))
//!                 .equals((order::Entity, order::Column::Id)),
//!         ),
//!     )
//!     .all(&conn)
//!     .await?;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;

use sea_orm::sea_query::{
    Alias, CommonTableExpression, Expr, IntoColumnRef, IntoIden, JoinType, Order, SelectStatement,
    UnionType, WithClause,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, ExprTrait, FromQueryResult, QueryFilter, QueryTrait,
    StatementBuilder,
};

use crate::secure::error::ScopeError;
use crate::secure::select::{Scoped, SecureEntityExt, SecureSelect};
use crate::secure::{AccessScope, DBRunner, DBRunnerInternal, ScopableEntity};

/// Column a recursive CTE exposes its distance-from-seed on.
///
/// Emitted into every recursive CTE body, because the depth cap has to be
/// expressible as a predicate on the recursive member. A CTE whose source entity
/// already has a column of this name would collide, so the name is deliberately
/// unlikely: double leading underscore, not a plausible domain column.
const DEPTH_COLUMN: &str = "__cte_depth";

/// A scoped query that carries `WITH` definitions.
///
/// Built by [`SecureSelect::with_ctes`]. Every CTE registered here is scoped
/// with the originating query's `AccessScope`, so mixing scopes in one statement
/// is unrepresentable rather than rejected.
///
/// Executes through `find_by_statement`, not `Select<E>` — see the module docs.
#[must_use]
pub struct SecureCteSelect<E: EntityTrait> {
    /// Outer query, scope `WHERE` already embedded by `scope_with`.
    outer: SelectStatement,
    with: WithClause,
    /// Seeds every CTE body. This is what makes same-scope structural.
    scope: Arc<AccessScope>,
    _entity: PhantomData<E>,
}

/// A recursive CTE that walks a self-referencing hierarchy.
///
/// Both the seed and the recursive member carry the outer query's scope: the
/// recursive member reads the *real table* joined against the CTE self-reference,
/// so `build_scope_condition` applies to it exactly as it does to the seed. That
/// is why recursion is admissible here at all.
///
/// The hazard recursion does introduce is **cycles**, and it is not optional to
/// handle: `sea_query` renders `PostgreSQL`'s `CYCLE` clause but silently drops it
/// on `MySQL` and `SQLite`, so a portable guard cannot rely on it. Hence `max_depth`
/// has no default — a caller must state a bound, and it is emitted as a predicate
/// on the recursive member.
///
/// # One row per hop, not per node — and no visited set
///
/// The walk emits a row for every hop it takes, and **nothing deduplicates**.
/// There is no visited set: this enumerates *paths*, not distinct reachable
/// nodes.
///
/// On a tree that just means a node reachable by two paths appears twice. The
/// consequences are much larger elsewhere:
///
/// - On a **cyclic** graph the cycle members repeat until the depth cap stops the
///   walk, so the result size is bounded by the cap, not by the node count.
/// - On a **dense or hub-heavy** graph the path count grows with the branching
///   factor to the power of the depth. A hub with a thousand edges, walked four
///   hops, is a very large intermediate result even though few distinct nodes are
///   involved.
///
/// So this primitive suits **bounded, sparse, acyclic** hierarchies. For a general
/// graph, prefer expanding one hop per query with [`SecureCteSelect::cte`] and
/// keeping the frontier on the caller side, where you control deduplication
/// between hops.
///
/// A portable visited set is not on offer: the usual trick carries the path in an
/// array and tests `NOT id = ANY(path)`, which relies on `PostgreSQL` arrays and has
/// no MySQL/SQLite equivalent. [`SecureCteSelect::distinct`] deduplicates the
/// *final* rows but does not prune the walk itself.
#[must_use]
pub struct RecursiveCte<J: EntityTrait> {
    name: &'static str,
    seed: Condition,
    child_col: J::Column,
    parent_col: J::Column,
    max_depth: u32,
}

impl<J: EntityTrait> RecursiveCte<J> {
    /// Name of the column carrying distance-from-seed, for ordering or filtering
    /// the CTE from the outer query.
    ///
    /// Exposed so callers do not hardcode the literal; see
    /// [`SecureCteSelect::order_by`] for the usual use.
    pub const DEPTH_COLUMN: &'static str = DEPTH_COLUMN;

    /// Describe a recursive walk over `J`.
    ///
    /// - `name` — the CTE's name, referenced later via
    ///   [`SecureCteSelect::join_cte`].
    /// - `seed` — which rows start the walk, e.g. `Column::Id.eq(root)`. Scope is
    ///   applied on top of this; it is not a substitute for it.
    /// - `child_col` — the column holding a row's pointer to its parent.
    /// - `parent_col` — the column that pointer refers to, usually the primary key.
    /// - `max_depth` — how many hops past the seed to follow. `0` yields the seed
    ///   rows only. Mandatory: see the type docs on cycles.
    pub fn new(
        name: &'static str,
        seed: Condition,
        child_col: J::Column,
        parent_col: J::Column,
        max_depth: u32,
    ) -> Self {
        Self {
            name,
            seed,
            child_col,
            parent_col,
            max_depth,
        }
    }
}

// `with_ctes` lives on the scoped select: a CTE query is only reachable from an
// already-scoped query, which is what carries the scope the bodies inherit.
impl<E> SecureSelect<E, Scoped>
where
    E: EntityTrait,
{
    /// Begin a query that carries `WITH` definitions.
    ///
    /// Every CTE registered on the result is scoped with **this** query's
    /// `AccessScope`, so a differently-scoped CTE cannot be constructed. The
    /// returned type executes through its own path; see [`SecureCteSelect`].
    pub fn with_ctes(self) -> SecureCteSelect<E> {
        let scope = self.scope_arc();
        SecureCteSelect {
            outer: QueryTrait::into_query(self.into_inner()),
            with: WithClause::new(),
            scope,
            _entity: PhantomData,
        }
    }
}

impl<E> SecureCteSelect<E>
where
    E: EntityTrait,
{
    /// Register a non-recursive CTE over `J`.
    ///
    /// `body` shapes the CTE's `SELECT` — filters, ordering, limits. Scope is
    /// applied *after* it returns, so a body cannot filter the scope predicate
    /// back off.
    ///
    /// Attaching a definition is not the same as using it: reference the CTE from
    /// the outer query with [`join_cte`](Self::join_cte).
    pub fn cte<J>(
        mut self,
        name: &'static str,
        body: impl FnOnce(sea_orm::Select<J>) -> sea_orm::Select<J>,
    ) -> Self
    where
        J: ScopableEntity + EntityTrait,
        J::Column: ColumnTrait + Copy,
    {
        // Routed through `scope_with_arc` rather than calling
        // `build_scope_condition` directly, so there is exactly one definition of
        // "apply scope to a select" in the crate.
        let scoped = body(J::find())
            .secure()
            .scope_with_arc(Arc::clone(&self.scope));

        let mut cte = CommonTableExpression::new();
        cte.table_name(Alias::new(name))
            .query(QueryTrait::into_query(scoped.into_inner()));
        self.with.cte(cte);
        self
    }

    /// Register a recursive CTE over a self-referencing `J`.
    ///
    /// Emits, with `<scope>` present in **both** members:
    ///
    /// ```sql
    /// WITH RECURSIVE n AS (
    ///     SELECT <J cols>, 0 AS __cte_depth FROM j
    ///      WHERE <seed> AND <scope>
    ///     UNION ALL
    ///     SELECT <J cols>, n.__cte_depth + 1 FROM j
    ///       JOIN n ON j.<child_col> = n.<parent_col>
    ///      WHERE <scope> AND n.__cte_depth < <max_depth>
    /// )
    /// ```
    pub fn recursive_cte<J>(mut self, spec: RecursiveCte<J>) -> Self
    where
        J: ScopableEntity + EntityTrait,
        J::Column: ColumnTrait + Copy,
    {
        let cte_ref = Alias::new(spec.name);
        let depth = Alias::new(DEPTH_COLUMN);

        // Seed: the caller's starting predicate, AND the scope.
        let mut seed = QueryTrait::into_query(
            J::find()
                .filter(spec.seed)
                .secure()
                .scope_with_arc(Arc::clone(&self.scope))
                .into_inner(),
        );
        seed.expr_as(Expr::val(0), depth.clone());

        // Recursive member: reads the real table `J` joined to the CTE, so the
        // same scope condition applies here as to the seed. The depth predicate
        // is emitted by us, not the caller -- it is the termination guarantee.
        let mut step = QueryTrait::into_query(
            J::find()
                .secure()
                .scope_with_arc(Arc::clone(&self.scope))
                .into_inner(),
        );
        step.expr_as(
            Expr::col((cte_ref.clone(), depth.clone())).add(Expr::val(1)),
            depth.clone(),
        )
        .join(
            JoinType::InnerJoin,
            cte_ref.clone(),
            Expr::col((J::default(), spec.child_col))
                .equals((cte_ref.clone(), spec.parent_col.into_iden())),
        )
        .and_where(Expr::col((cte_ref, depth)).lt(spec.max_depth));

        seed.union(UnionType::All, step);

        let mut cte = CommonTableExpression::new();
        cte.table_name(Alias::new(spec.name)).query(seed);
        self.with.recursive(true).cte(cte);
        self
    }

    /// Join a registered CTE into the outer query.
    ///
    /// `with_ctes`/`cte` only *define*; without a reference the `WITH` clause is
    /// valid SQL that computes nothing. `on` ties the CTE to the outer entity and
    /// is the caller's responsibility — getting it wrong changes which rows come
    /// back, but cannot cross a tenant boundary, because the CTE body never held
    /// another tenant's rows to begin with.
    ///
    /// # Do not join on a disjunction
    ///
    /// A join predicate of the form `outer.id = cte.a OR outer.id = cte.b` — the
    /// natural way to say "either endpoint of an edge" when expanding a graph
    /// frontier — reads as one condition but plans as two. `PostgreSQL` cannot
    /// drive an index from two hashed subplans under an `OR` and falls back to a
    /// sequential scan of the outer table: on 199k rows the same 11-row result
    /// took 15.2 ms that way against 0.30 ms expressed as a single semi-join.
    ///
    /// Prefer one membership test over the union of the CTE's columns, through
    /// [`filter`](Self::filter):
    ///
    /// ```rust,ignore
    /// let endpoints = Query::select()
    ///     .column(Alias::new("src_id"))
    ///     .from(Alias::new("edges"))
    ///     .to_owned()
    ///     .union(UnionType::Distinct, Query::select()
    ///         .column(Alias::new("dst_id"))
    ///         .from(Alias::new("edges"))
    ///         .to_owned())
    ///     .to_owned();
    ///
    /// .filter(Condition::all().add(Expr::col(node::Column::Id).in_subquery(endpoints)))
    /// ```
    pub fn join_cte(mut self, name: &'static str, on: Condition) -> Self {
        self.outer.join(JoinType::InnerJoin, Alias::new(name), on);
        self
    }

    /// Narrow the outer query's projection to `columns`.
    ///
    /// Without this the outer query selects every column of `E`, and
    /// [`all_as`](Self::all_as) discards the surplus only after the database has
    /// read and shipped it. That is the difference between an index-only scan and
    /// a heap visit per row: on a 199k-row table a hop returning 11 ids measured
    /// 0.371 ms selecting all columns against 0.079 ms selecting the key alone,
    /// and the gap widens with the width of the row -- a `jsonb` payload or a
    /// full-text column is carried on every row that matches.
    ///
    /// Pair with [`all_as`](Self::all_as): after narrowing, the row no longer has
    /// the shape of `E::Model`, so [`all`](Self::all) and [`one`](Self::one) would
    /// fail to deserialize.
    ///
    /// Columns are taken as `E::Column` and rendered table-qualified. An
    /// unqualified name would be ambiguous the moment a CTE joined by
    /// [`join_cte`](Self::join_cte) carries a column of the same name — `id` being
    /// the obvious case — and the database rejects the statement at execution
    /// rather than at build time.
    pub fn columns<I>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = E::Column>,
    {
        self.outer.clear_selects();
        for column in columns {
            self.outer.column((E::default(), column));
        }
        self
    }

    /// Add a filter to the outer query, on top of its scope.
    pub fn filter(mut self, filter: Condition) -> Self {
        self.outer.cond_where(filter);
        self
    }

    /// Limit the outer query.
    pub fn limit(mut self, limit: u64) -> Self {
        self.outer.limit(limit);
        self
    }

    /// `SELECT DISTINCT` on the outer query.
    ///
    /// [`join_cte`](Self::join_cte) emits an inner join, so an outer row is
    /// repeated once per matching CTE row. That is usually not what you want when
    /// the CTE is a set you are testing membership against — e.g. expanding a
    /// breadth-first frontier, where several edges converge on the same node.
    pub fn distinct(mut self) -> Self {
        self.outer.distinct();
        self
    }

    /// Order the outer query.
    ///
    /// Accepts a bare column or a qualified `(table, column)` pair, so a CTE's own
    /// columns can be ordered on:
    ///
    /// ```rust,ignore
    /// // Shallowest hop first.
    /// .order_by(
    ///     (Alias::new("subtree"), Alias::new(RecursiveCte::<E>::DEPTH_COLUMN)),
    ///     Order::Asc,
    /// )
    /// ```
    pub fn order_by(mut self, col: impl IntoColumnRef, order: Order) -> Self {
        self.outer.order_by(col, order);
        self
    }

    /// Render the statement for `backend` without executing it.
    ///
    /// Exists because this crate has no mock database: it is the only way for a
    /// test to assert on the emitted SQL, including that the `WITH` clause is
    /// present at all.
    #[doc(hidden)]
    #[must_use]
    pub fn build_statement(&self, backend: sea_orm::DbBackend) -> sea_orm::Statement {
        let query = self.outer.clone().with(self.with.clone());
        StatementBuilder::build(&query, &backend)
    }

    /// Execute and return all matching rows of `E`.
    ///
    /// # Errors
    /// Returns `ScopeError::Db` if the query fails.
    pub async fn all(self, runner: &impl DBRunner) -> Result<Vec<E::Model>, ScopeError>
    where
        E::Model: Send + Sync,
    {
        self.all_as::<E::Model>(runner).await
    }

    /// Execute and return at most one row of `E`.
    ///
    /// # Errors
    /// Returns `ScopeError::Db` if the query fails.
    pub async fn one(self, runner: &impl DBRunner) -> Result<Option<E::Model>, ScopeError> {
        let stmt = self.build_statement(DBRunnerInternal::as_seaorm(runner).backend());
        Ok(match DBRunnerInternal::as_seaorm(runner) {
            crate::secure::SeaOrmRunner::Conn(db) => {
                E::Model::find_by_statement(stmt).one(db).await?
            }
            crate::secure::SeaOrmRunner::Tx(tx) => {
                E::Model::find_by_statement(stmt).one(tx).await?
            }
        })
    }

    /// Execute and deserialize into a custom projection `T`.
    ///
    /// CTE queries usually exist to compute something the entity model has no
    /// shape for — aggregates, window functions, dedup counts — so the row type
    /// is not always `E::Model`.
    ///
    /// # Errors
    /// Returns `ScopeError::Db` if the query fails.
    pub async fn all_as<T>(self, runner: &impl DBRunner) -> Result<Vec<T>, ScopeError>
    where
        T: FromQueryResult + Send + Sync,
    {
        let stmt = self.build_statement(DBRunnerInternal::as_seaorm(runner).backend());
        Ok(match DBRunnerInternal::as_seaorm(runner) {
            crate::secure::SeaOrmRunner::Conn(db) => T::find_by_statement(stmt).all(db).await?,
            crate::secure::SeaOrmRunner::Tx(tx) => T::find_by_statement(stmt).all(tx).await?,
        })
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[path = "cte_tests.rs"]
mod tests;
