use crate::indexer::SessionIndex;
use chrono::DateTime;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::Value;
use tantivy::{Order, TantivyDocument};

/// 搜索结果条目
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub session_id: String,
    pub provider: String,
    pub project_path: String,
    pub project_name: String,
    pub first_prompt: String,
    pub last_prompt: String,
    pub updated_at: String,
    pub message_count: u64,
    pub total_tokens: u64,
    pub score: f32,
}

/// 全文搜索
pub fn search(
    index: &SessionIndex,
    query_str: &str,
    max_results: usize,
) -> Vec<SearchResult> {
    let searcher = index.reader().searcher();
    let schema = index.schema();

    let project_name = schema.get_field("project_name").unwrap();
    let first_prompt = schema.get_field("first_prompt").unwrap();
    let last_prompt = schema.get_field("last_prompt").unwrap();
    let content = schema.get_field("content").unwrap();

    let query_parser = QueryParser::for_index(
        index.index(),
        vec![project_name, first_prompt, last_prompt, content],
    );

    let query = match query_parser.parse_query(query_str) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let top_docs = match searcher.search(&query, &TopDocs::with_limit(max_results)) {
        Ok(docs) => docs,
        Err(_) => return Vec::new(),
    };

    extract_results(&searcher, schema, &top_docs)
}

/// 列出会话（按更新时间降序），可按 provider 过滤
pub fn list_all(
    index: &SessionIndex,
    max_results: usize,
    provider_filter: Option<&str>,
) -> Vec<SearchResult> {
    let searcher = index.reader().searcher();
    let schema = index.schema();

    // 构造查询：全部 或 按 provider 过滤
    let query: Box<dyn tantivy::query::Query> = match provider_filter {
        Some(provider) => {
            let provider_field = schema.get_field("provider").unwrap();
            let term = tantivy::Term::from_field_text(provider_field, provider);
            Box::new(tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic))
        }
        None => Box::new(AllQuery),
    };

    let collector = TopDocs::with_limit(max_results)
        .order_by_fast_field::<tantivy::DateTime>("updated_at", Order::Desc);

    let top_docs = match searcher.search(&*query, &collector) {
        Ok(docs) => docs,
        Err(_) => return Vec::new(),
    };

    let results: Vec<(f32, tantivy::DocAddress)> = top_docs
        .into_iter()
        .map(|(_date, addr)| (0.0f32, addr))
        .collect();

    extract_results(&searcher, schema, &results)
}

/// 从搜索结果文档地址中提取结构化数据
fn extract_results(
    searcher: &tantivy::Searcher,
    schema: &tantivy::schema::Schema,
    docs: &[(f32, tantivy::DocAddress)],
) -> Vec<SearchResult> {
    let session_id_field = schema.get_field("session_id").unwrap();
    let provider_field = schema.get_field("provider").unwrap();
    let project_path_field = schema.get_field("project_path").unwrap();
    let project_name_field = schema.get_field("project_name").unwrap();
    let first_prompt_field = schema.get_field("first_prompt").unwrap();
    let last_prompt_field = schema.get_field("last_prompt").unwrap();
    let updated_at_field = schema.get_field("updated_at").unwrap();
    let message_count_field = schema.get_field("message_count").unwrap();
    let total_tokens_field = schema.get_field("total_tokens").unwrap();

    let mut results = Vec::new();
    for (score, doc_addr) in docs {
        // tantivy 0.22: doc() 需要类型参数 TantivyDocument
        let doc: TantivyDocument = match searcher.doc(*doc_addr) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // tantivy 0.22: get_first 返回 Option<&OwnedValue>，
        // as_str() / as_datetime() / as_u64() 来自 Value trait
        let get_text = |field| -> String {
            doc.get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let updated_str = doc
            .get_first(updated_at_field)
            .and_then(|v| v.as_datetime())
            .map(|dt| {
                let ts = dt.into_timestamp_micros();
                DateTime::from_timestamp_micros(ts)
                    .unwrap_or_default()
                    .format("%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();

        let msg_count = doc
            .get_first(message_count_field)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let tokens = doc
            .get_first(total_tokens_field)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        results.push(SearchResult {
            session_id: get_text(session_id_field),
            provider: get_text(provider_field),
            project_path: get_text(project_path_field),
            project_name: get_text(project_name_field),
            first_prompt: get_text(first_prompt_field),
            last_prompt: get_text(last_prompt_field),
            updated_at: updated_str,
            message_count: msg_count,
            total_tokens: tokens,
            score: *score,
        });
    }
    results
}
