#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use crate::{GitHub, prelude::RenderableComment};

fn str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse()
        .map_err(|_| serde::de::Error::custom("Failed to parse u64"))
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ResponseDataWrapper<T> {
    data: T,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PullRequestData {
    repository: PullRequestThreadsResponse,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestThreadsResponse {
    pub name: String,
    pub owner: Actor,
    pub pull_request: PullRequest,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Actor {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub author: Actor,
    pub comments: CommentConnection<IssueComment>,
    pub review_threads: ReviewThreadConnection,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentConnection<T> {
    pub nodes: Vec<T>,
    pub page_info: Option<PageInfo>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IssueComment {
    pub author: Actor,
    pub body: String,
    pub created_at: DateTime<Utc>,
    #[serde(deserialize_with = "str_to_u64")]
    pub full_database_id: u64,
    pub id: String,
}

impl RenderableComment for IssueComment {
    fn author(&self) -> &str {
        &self.author.login
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn body(&self) -> &str {
        &self.body
    }
}

impl Ord for IssueComment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.created_at.cmp(&other.created_at)
    }
}

impl PartialOrd for IssueComment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThreadConnection {
    pub nodes: Vec<ReviewThread>,
    pub index: Option<HashMap<String, usize>>,
    pub page_info: Option<PageInfo>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThread {
    pub id: String,
    pub is_resolved: bool,
    pub path: String,
    pub comments: CommentConnection<ReviewComment>,
}

impl Ord for ReviewThread {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for ReviewThread {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewComment {
    pub author: Actor,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub diff_hunk: String,
    #[serde(deserialize_with = "str_to_u64")]
    pub full_database_id: u64,
    pub id: String,
    pub line: Option<i64>,
    pub original_line: Option<i64>,
    pub original_start_line: Option<i64>,
    pub outdated: bool,
    pub path: String,
    pub repository: CommentRepository,
    pub start_line: Option<i64>,
    pub subject_type: String,
}

impl RenderableComment for ReviewComment {
    fn author(&self) -> &str {
        &self.author.login
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn body(&self) -> &str {
        &self.body
    }
}

impl Ord for ReviewComment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.created_at.cmp(&other.created_at)
    }
}

impl PartialOrd for ReviewComment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommentRepository {
    pub name: String,
    pub owner: Actor,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ReviewThreadCommentsData {
    pub node: Option<ReviewThreadCommentsNode>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThreadCommentsNode {
    pub comments: CommentConnection<ReviewComment>,
}

impl GitHub {
    pub async fn fetch_threads(
        &self,
        owner: &str,
        repo: &str,
        pr: u64,
    ) -> Result<PullRequestThreadsResponse> {
        let PullRequestThreadsResponse {
            name,
            owner: repo_owner,
            pull_request,
        } = self
            .fetch_threads_page(owner, repo, pr, None, None, None)
            .await?;

        let PullRequest {
            number,
            author,
            comments,
            review_threads,
        } = pull_request;

        let mut pr_comments = comments.nodes;
        let mut pr_comments_page_info = comments.page_info;
        let mut review_threads_nodes = review_threads.nodes;
        let mut review_threads_page_info = review_threads.page_info;

        while pr_comments_page_info
            .as_ref()
            .map(|pi| pi.has_next_page)
            .unwrap_or(false)
        {
            let cursor = match pr_comments_page_info
                .as_ref()
                .and_then(|pi| pi.end_cursor.as_deref())
            {
                Some(cursor) => cursor,
                None => break,
            };

            let page = self
                .fetch_threads_page(owner, repo, pr, Some(cursor), None, None)
                .await?;

            pr_comments.extend(page.pull_request.comments.nodes);
            pr_comments_page_info = page.pull_request.comments.page_info;
        }

        while review_threads_page_info
            .as_ref()
            .map(|pi| pi.has_next_page)
            .unwrap_or(false)
        {
            let cursor = match review_threads_page_info
                .as_ref()
                .and_then(|pi| pi.end_cursor.as_deref())
            {
                Some(cursor) => cursor,
                None => break,
            };

            let page = self
                .fetch_threads_page(owner, repo, pr, None, Some(cursor), None)
                .await?;

            review_threads_nodes.extend(page.pull_request.review_threads.nodes);
            review_threads_page_info = page.pull_request.review_threads.page_info;
        }

        for thread in &mut review_threads_nodes {
            let mut comments_page_info = thread.comments.page_info.clone();

            while comments_page_info
                .as_ref()
                .map(|pi| pi.has_next_page)
                .unwrap_or(false)
            {
                let cursor = match comments_page_info
                    .as_ref()
                    .and_then(|pi| pi.end_cursor.as_deref())
                {
                    Some(cursor) => cursor,
                    None => break,
                };

                let page = self
                    .fetch_review_thread_comments_page(&thread.id, Some(cursor))
                    .await?;

                thread.comments.nodes.extend(page.nodes);
                comments_page_info = page.page_info;
            }

            thread.comments.page_info = comments_page_info;
            thread.comments.nodes.sort_unstable();
        }

        pr_comments.sort_unstable();
        review_threads_nodes.sort_unstable();

        Ok(PullRequestThreadsResponse {
            name,
            owner: repo_owner,
            pull_request: PullRequest {
                number,
                author,
                comments: CommentConnection {
                    nodes: pr_comments,
                    page_info: pr_comments_page_info,
                },
                review_threads: ReviewThreadConnection {
                    page_info: review_threads_page_info,
                    index: Some(
                        review_threads_nodes
                            .iter()
                            .enumerate()
                            .map(|(i, t)| (t.id.clone(), i))
                            .collect(),
                    ),
                    nodes: review_threads_nodes,
                },
            },
        })
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
                                        pageInfo {{
                                            endCursor
                                            startCursor
                                            hasNextPage
                                            hasPreviousPage
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

    async fn fetch_review_thread_comments_page(
        &self,
        review_thread_id: &str,
        comments_cursor: Option<&str>,
    ) -> Result<CommentConnection<ReviewComment>> {
        let comments_cursor = comments_cursor
            .map(|c| format!(r#""{c}""#))
            .unwrap_or_else(|| "null".to_string());

        let response: ResponseDataWrapper<ReviewThreadCommentsData> = self
            .graphql(&json!({
                "query": format!(r#"
                query {{
                    node(id: "{review_thread_id}") {{
                        ... on PullRequestReviewThread {{
                            comments(first: 100, after: {comments_cursor}) {{
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

        let thread = response
            .data
            .node
            .ok_or_else(|| anyhow!("Review thread {review_thread_id} not found"))?;

        Ok(thread.comments)
    }
}

#[cfg(test)]
mod tests {
    use testresult::TestResult;

    use crate::GitHub;

    #[tokio::test]
    async fn test_github_graphql() -> TestResult {
        let gh = GitHub::new();

        let res = gh.fetch_threads("jameslkingsley", "gh-threads", 2).await?;

        dbg!(res);

        Ok(())
    }
}
