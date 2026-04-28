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
