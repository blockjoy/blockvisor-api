/// This struct contains fields by which we can customize which host to pick when starting a new
/// node. The fields are sorted by precendence from top to bottom, i.e. if the `similarity` field
/// and the `resource` field are both set, then `similarity` takes precedence over `resource`. Note
/// that the final field of this struct is required, in order to make sure that the affinity of
/// nodes is always defined.
pub struct NodeScheduler {
    /// Controls in which region the node should be deployed.
    pub region: Option<String>,
    /// Controls whether we want to group nodes of the same kind together or spread them out over
    /// multiple hosts.
    pub similarity: Option<SimilarNodeAffinity>,
    /// Controls whether a node should prefer the host that has the most or the least free
    /// resources. That is, do we fill breadth first or depth first.
    pub resource: ResourceAffinity,
}

/// Controls whether nodes should first be deployed onto hosts that have another node of the same
/// kind running on it. "The same kind" is defined as having the same blockchain_id and node_type,
/// but the version field is _not_ used here.
#[derive(Clone, Copy, Debug, diesel_derive_enum::DbEnum)]
#[ExistingTypePath = "crate::models::schema::sql_types::EnumNodeSimilarityAffinity"]
pub enum SimilarNodeAffinity {
    /// Prefer to deploy new nodes onto hosts that have a similar node running. This is desired when
    /// the nodes form a cluster and thus they have to have a low network latency between them.
    Cluster,
    /// Prefer to deploy new nodes onto hosts that do _not_ have similar nodes running on them. This
    /// is desired when multiple nodes are ran for the sake of redundancy, and one hosts failing
    /// must not bring down all of the nodes.
    Spread,
}

/// This enum indicates whether we should prefer to fill hosts that have the most resources or the
/// least resources first.
#[derive(Clone, Copy, Debug, diesel_derive_enum::DbEnum)]
#[ExistingTypePath = "crate::models::schema::sql_types::EnumNodeResourceAffinity"]
pub enum ResourceAffinity {
    /// Prefer to fill out hosts that have the most availably resources.
    MostResources,
    /// Prefer to spread load out over hosts by picking the least crowded host first.
    LeastResources,
}

impl NodeScheduler {
    /// The scheduler can influence which node is selected through this function. It does so by
    /// transforming itself into a string of the form:
    /// ```sql
    /// ORDER BY
    ///     *[<column> "ASC" | "DESC"],
    /// ```
    /// This string in intented to be embedded into the query used in models::Host::host_candidates.
    pub fn order_clause(&self) -> String {
        let mut clause = "ORDER BY \n    ".to_string();
        if let Some(similarity) = &self.similarity {
            clause += similarity.order_clause();
        }
        clause + self.resource.order_clause()
    }
}

impl SimilarNodeAffinity {
    /// Since we are sorting by number of similar nodes, we want the greatest number (DESC) of
    /// similar nodes when we are `Cluster`ing, and the least number (ASC) of similar nodes when we
    /// are `Spread`ing.
    fn order_clause(&self) -> &'static str {
        // Quick note, we can place a trailing comma here, because ResourceAffinity is required and
        // therefore there is always at least one other order_clause following this one.
        match self {
            Self::Cluster => "n_similar DESC, ",
            Self::Spread => "n_similar ASC, ",
        }
    }
}

impl ResourceAffinity {
    /// When we want the greatest number (DESC) of resources, we take all of the resources in order
    /// of priority, and mark sort by them one by one, lexicographically. We do the same for the
    /// least number of resources, but sort ascendingly.
    fn order_clause(&self) -> &'static str {
        match self {
            Self::MostResources => "av_cpus DESC, av_mem DESC, av_disk DESC",
            Self::LeastResources => "av_cpus ASC, av_mem ASC, av_disk ASC",
        }
    }
}
