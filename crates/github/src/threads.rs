#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use crate::GitHub;

#[derive(Debug, Deserialize)]
struct ResponseDataWrapper<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct PullRequestData {
    repository: PullRequestThreadsResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestThreadsResponse {
    name: String,
    owner: Actor,
    pull_request: PullRequest,
}

#[derive(Debug, Deserialize)]
pub struct Actor {
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    number: u64,
    author: Actor,
    comments: CommentConnection<IssueComment>,
    review_threads: ReviewThreadConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentConnection<T> {
    nodes: Vec<T>,
    page_info: Option<PageInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    end_cursor: Option<String>,
    has_next_page: bool,
    has_previous_page: bool,
    start_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueComment {
    author: Actor,
    body: String,
    created_at: DateTime<Utc>,
    full_database_id: Option<String>,
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThreadConnection {
    nodes: Vec<ReviewThread>,
    page_info: Option<PageInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThread {
    id: String,
    is_resolved: bool,
    path: String,
    comments: CommentConnection<ReviewComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewComment {
    author: Actor,
    body: String,
    created_at: DateTime<Utc>,
    diff_hunk: String,
    full_database_id: Option<String>,
    id: String,
    line: Option<i64>,
    original_line: Option<i64>,
    original_start_line: Option<i64>,
    outdated: bool,
    path: String,
    repository: CommentRepository,
    start_line: Option<i64>,
    subject_type: String,
}

#[derive(Debug, Deserialize)]
pub struct CommentRepository {
    name: String,
    owner: Actor,
}

impl GitHub {
    pub async fn fetch_threads(
        &self,
        owner: &str,
        repo: &str,
        pr: u64,
    ) -> Result<PullRequestThreadsResponse> {
        let mut response: Option<ResponseDataWrapper<PullRequestData>> = None;

        todo!()
    }

    async fn fetch_threads_page(
        &self,
        owner: &str,
        repo: &str,
        pr: u64,
        pr_comments_cursor: Option<&str>,
        pr_review_threads_cursor: Option<&str>,
        pr_review_threads_comments_cursor: Option<&str>,
    ) -> Result<PullRequestThreadsResponse> {
        let pr_comments_cursor = pr_comments_cursor
            .map(|c| format!(r#""{c}""#))
            .unwrap_or("null".to_string());

        let pr_review_threads_cursor = pr_review_threads_cursor
            .map(|c| format!(r#""{c}""#))
            .unwrap_or("null".to_string());

        let pr_review_threads_comments_cursor = pr_review_threads_comments_cursor
            .map(|c| format!(r#""{c}""#))
            .unwrap_or("null".to_string());

        let response: ResponseDataWrapper<PullRequestData> = self
            .graphql(&json!({
                "query": format!(r#"
                query {{
                    repository(owner: "{owner}", name: "{repo}") {{
                        name
                        owner {{
                            login
                        }}
                        pullRequest(number: {pr}) {{
                            number
                            author {{
                                login
                            }}
                            comments(first: 100, after: {pr_comments_cursor}) {{
                                nodes {{
                                    author {{
                                        login
                                    }}
                                    body
                                    createdAt
                                    fullDatabaseId
                                    id
                                }}
                                pageInfo {{
                                    endCursor
                                    startCursor
                                    hasNextPage
                                    hasPreviousPage
                                }}
                            }}
                            reviewThreads(first: 100, after: {pr_review_threads_cursor}) {{
                                nodes {{
                                    id
                                    isResolved
                                    path
                                    comments(first: 100, after: {pr_review_threads_comments_cursor}) {{
                                        nodes {{
                                            author {{
                                                login
                                            }}
                                            body
                                            createdAt
                                            diffHunk
                                            fullDatabaseId
                                            id
                                            line
                                            originalLine
                                            originalStartLine
                                            outdated
                                            path
                                            repository {{
                                                name
                                                owner {{
                                                    login
                                                }}
                                            }}
                                            startLine
                                            subjectType
                                        }}
                                    }}
                                }}
                                pageInfo {{
                                    endCursor
                                    startCursor
                                    hasNextPage
                                    hasPreviousPage
                                }}
                            }}
                        }}
                    }}
                }}"#)
            }))
            .await?;

        Ok(response.data.repository)
    }
}
