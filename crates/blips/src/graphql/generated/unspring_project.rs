#![allow(clippy::all, warnings)]
pub struct UnspringProject;
pub mod unspring_project {
    #![allow(dead_code)]
    use std::result::Result;
    pub const OPERATION_NAME: &str = "UnspringProject";
    pub const QUERY : & str = "mutation UnspringProject($project_id: ID!) {\n    unspringProject(projectId: $project_id) {\n        ...Project\n    }\n}\n\nfragment Project on Project {\n    __typename\n    collapseCompleted\n    completed\n    completedAt\n    date\n    endDate\n    id\n    link\n    name\n    noteBody\n    order\n    springEnabled\n    supportsNotes\n}" ;
    use super::*;
    use serde::{Deserialize, Serialize};
    #[allow(dead_code)]
    type Boolean = bool;
    #[allow(dead_code)]
    type Float = f64;
    #[allow(dead_code)]
    type Int = i64;
    #[allow(dead_code)]
    type ID = String;
    type DateTime = crate::graphql::custom_scalars::DateTime;
    type Date = crate::graphql::custom_scalars::Date;
    #[derive(Serialize)]
    pub struct Variables {
        pub project_id: ID,
    }
    impl Variables {}
    #[derive(Deserialize, Debug)]
    pub struct Project {
        #[serde(rename = "collapseCompleted")]
        pub collapse_completed: Boolean,
        pub completed: Boolean,
        #[serde(rename = "completedAt")]
        pub completed_at: Option<DateTime>,
        pub date: Option<Date>,
        #[serde(rename = "endDate")]
        pub end_date: Option<Date>,
        pub id: ID,
        pub link: Option<String>,
        pub name: String,
        #[serde(rename = "noteBody")]
        pub note_body: Option<String>,
        pub order: Option<Int>,
        #[serde(rename = "springEnabled")]
        pub spring_enabled: Boolean,
        #[serde(rename = "supportsNotes")]
        pub supports_notes: Boolean,
    }
    #[derive(Deserialize, Debug)]
    pub struct ResponseData {
        #[serde(rename = "unspringProject")]
        pub unspring_project: UnspringProjectUnspringProject,
    }
    pub type UnspringProjectUnspringProject = Project;
}
impl graphql_client::GraphQLQuery for UnspringProject {
    type Variables = unspring_project::Variables;
    type ResponseData = unspring_project::ResponseData;
    fn build_query(variables: Self::Variables) -> ::graphql_client::QueryBody<Self::Variables> {
        graphql_client::QueryBody {
            variables,
            query: unspring_project::QUERY,
            operation_name: unspring_project::OPERATION_NAME,
        }
    }
}
