use crate::config::retalk_dir;
use crate::models::Session;
use jieba_rs::Jieba;
use std::sync::Arc;
use tantivy::directory::MmapDirectory;
use tantivy::schema::*;
use tantivy::tokenizer::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};

/// Tantivy 索引管理器
pub struct SessionIndex {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    #[allow(dead_code)]
    jieba: Arc<Jieba>,
}

/// jieba 分词器适配 tantivy Tokenizer trait
#[derive(Clone)]
struct JiebaTokenizer {
    jieba: Arc<Jieba>,
}

impl Tokenizer for JiebaTokenizer {
    type TokenStream<'a> = JiebaTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let words = self.jieba.cut(text, true);
        let mut tokens = Vec::new();
        let mut offset = 0;
        for word in words {
            // 保留原始词长度用于偏移计算，再做 trim
            let raw_len = word.len();
            let trimmed = word.trim();
            if !trimmed.is_empty() {
                // 计算 trimmed 在原始 word 中的起始偏移
                let leading = word.len() - word.trim_start().len();
                tokens.push(Token {
                    offset_from: offset + leading,
                    offset_to: offset + leading + trimmed.len(),
                    position: tokens.len(),
                    text: trimmed.to_lowercase(),
                    position_length: 1,
                });
            }
            offset += raw_len;
        }
        JiebaTokenStream { tokens, index: 0 }
    }
}

/// jieba 分词结果的 TokenStream 实现
struct JiebaTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for JiebaTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl SessionIndex {
    /// 创建或打开索引，注册 jieba 分词器
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let index_dir = retalk_dir().join("index");
        std::fs::create_dir_all(&index_dir)?;

        let jieba = Arc::new(Jieba::new());

        // 构建 schema：文本字段使用 jieba 分词器
        let mut schema_builder = Schema::builder();
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let text_indexed_only = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            );

        schema_builder.add_text_field("session_id", STRING | STORED);
        schema_builder.add_text_field("provider", STRING | STORED);
        schema_builder.add_text_field("project_path", STRING | STORED);
        schema_builder.add_text_field("project_name", text_options.clone());
        schema_builder.add_text_field("first_prompt", text_options.clone());
        schema_builder.add_text_field("last_prompt", text_options.clone());
        schema_builder.add_text_field("content", text_indexed_only);
        schema_builder.add_date_field("updated_at", INDEXED | STORED | FAST);
        schema_builder.add_u64_field("message_count", STORED);
        schema_builder.add_u64_field("total_tokens", STORED);

        let schema = schema_builder.build();

        // 尝试打开已有索引，若 schema 不匹配则删除重建
        let index = match MmapDirectory::open(&index_dir)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            .and_then(|dir| Index::open_or_create(dir, schema.clone()).map_err(|e| Box::new(e) as Box<dyn std::error::Error>))
        {
            Ok(idx) => {
                // 检查 schema 是否包含必需字段（provider, total_tokens）
                if idx.schema().get_field("provider").is_err()
                    || idx.schema().get_field("total_tokens").is_err()
                {
                    drop(idx);
                    std::fs::remove_dir_all(&index_dir)?;
                    std::fs::create_dir_all(&index_dir)?;
                    let dir = MmapDirectory::open(&index_dir)?;
                    Index::open_or_create(dir, schema.clone())?
                } else {
                    idx
                }
            }
            Err(_) => {
                // 索引损坏或不兼容，删除重建
                let _ = std::fs::remove_dir_all(&index_dir);
                std::fs::create_dir_all(&index_dir)?;
                let dir = MmapDirectory::open(&index_dir)?;
                Index::open_or_create(dir, schema.clone())?
            }
        };

        // 注册 jieba 分词器到索引
        index
            .tokenizers()
            .register("jieba", JiebaTokenizer { jieba: Arc::clone(&jieba) });

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self { index, reader, schema, jieba })
    }

    /// 全量重建索引：清空后批量写入
    pub fn rebuild(&self, sessions: &[Session]) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        writer.delete_all_documents()?;
        for session in sessions {
            self.add_session_to_writer(&mut writer, session)?;
        }
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// 单条会话更新：先删除旧文档再写入新文档
    pub fn upsert_session(&self, session: &Session) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        let session_id_field = self.schema.get_field("session_id").unwrap();
        let term = tantivy::Term::from_field_text(session_id_field, &session.session_id);
        writer.delete_term(term);
        self.add_session_to_writer(&mut writer, session)?;
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// 将单个 Session 写入 IndexWriter
    fn add_session_to_writer(
        &self,
        writer: &mut IndexWriter,
        session: &Session,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_id = self.schema.get_field("session_id").unwrap();
        let provider = self.schema.get_field("provider").unwrap();
        let project_path = self.schema.get_field("project_path").unwrap();
        let project_name = self.schema.get_field("project_name").unwrap();
        let first_prompt = self.schema.get_field("first_prompt").unwrap();
        let last_prompt = self.schema.get_field("last_prompt").unwrap();
        let content = self.schema.get_field("content").unwrap();
        let updated_at = self.schema.get_field("updated_at").unwrap();
        let message_count = self.schema.get_field("message_count").unwrap();
        let total_tokens = self.schema.get_field("total_tokens").unwrap();

        // 合并所有用户消息作为全文检索内容
        let all_content = session.user_messages.join("\n");
        let date_val =
            tantivy::DateTime::from_timestamp_micros(session.updated_at.timestamp_micros());

        writer.add_document(doc!(
            session_id => session.session_id.as_str(),
            provider => session.provider.as_str(),
            project_path => session.project_path.as_str(),
            project_name => session.project_name.as_str(),
            first_prompt => session.first_prompt.as_str(),
            last_prompt => session.last_prompt.as_str(),
            content => all_content.as_str(),
            updated_at => date_val,
            message_count => session.message_count as u64,
            total_tokens => session.total_tokens,
        ))?;

        Ok(())
    }

    /// 索引中的文档数
    pub fn doc_count(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// 增量同步：只 upsert 新增/更新的会话，删除已不存在的
    pub fn incremental_sync(&self, sessions: &[Session]) -> Result<(), Box<dyn std::error::Error>> {
        use std::collections::HashSet;
        use tantivy::schema::Value;
        use tantivy::TantivyDocument;

        let searcher = self.reader.searcher();
        let session_id_field = self.schema.get_field("session_id").unwrap();
        let updated_at_field = self.schema.get_field("updated_at").unwrap();

        // 收集索引中所有 session_id -> updated_at
        let mut indexed: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for segment_reader in searcher.segment_readers() {
            let store = segment_reader.get_store_reader(1)?;
            for doc_id in 0..segment_reader.num_docs() {
                let doc: TantivyDocument = store.get(doc_id)?;
                let sid = doc.get_first(session_id_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ts = doc.get_first(updated_at_field)
                    .and_then(|v| v.as_datetime())
                    .map(|d| d.into_timestamp_micros())
                    .unwrap_or(0);
                if !sid.is_empty() {
                    indexed.insert(sid, ts);
                }
            }
        }

        // 计算需要 upsert 的和需要删除的
        let new_ids: HashSet<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        let mut to_upsert = Vec::new();
        for session in sessions {
            let new_ts = session.updated_at.timestamp_micros();
            match indexed.get(&session.session_id) {
                Some(&old_ts) if old_ts == new_ts => {} // 无变化，跳过
                _ => to_upsert.push(session),
            }
        }

        let to_delete: Vec<String> = indexed.keys()
            .filter(|id| !new_ids.contains(id.as_str()))
            .cloned()
            .collect();

        if to_upsert.is_empty() && to_delete.is_empty() {
            return Ok(()); // 无变化
        }

        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        // 删除已不存在的
        for id in &to_delete {
            let term = tantivy::Term::from_field_text(session_id_field, id);
            writer.delete_term(term);
        }

        // upsert 变化的
        for session in &to_upsert {
            let term = tantivy::Term::from_field_text(session_id_field, &session.session_id);
            writer.delete_term(term);
            self.add_session_to_writer(&mut writer, session)?;
        }

        writer.commit()?;
        self.reader.reload()?;
        eprintln!("[retalk] 增量同步: {} upsert, {} delete", to_upsert.len(), to_delete.len());
        Ok(())
    }

    /// 获取底层 Index 引用（供 searcher 模块使用）
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// 获取 IndexReader 引用（供 searcher 模块使用）
    pub fn reader(&self) -> &IndexReader {
        &self.reader
    }

    /// 获取 Schema 引用（供 searcher 模块使用）
    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}
