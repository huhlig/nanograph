---
parent: ADR
nav_order: 0018
title: Embedding Lifecycle and Model Integration
status: accepted
date: 2026-01-05
deciders: Hans W. Uhlig
---

# ADR-0018: Embedding Lifecycle and Model Integration

## Status

Accepted

## Context

Nanograph supports vector embeddings as a first-class feature for semantic search and AI-assisted queries. Key challenges include:

1. **Model flexibility** - Support various embedding models (local, remote, custom)
2. **Performance** - Embedding generation can be slow (100ms-1s per item)
3. **Consistency** - Embeddings must stay synchronized with source data
4. **Versioning** - Model upgrades require re-embedding existing data
5. **Cost** - External API calls can be expensive
6. **Reproducibility** - Same input should produce same embedding

Traditional approaches have limitations:
- **Synchronous generation** - Blocks writes, poor user experience
- **Always external** - Expensive, network dependent
- **No versioning** - Model changes break existing embeddings
- **Tight coupling** - Hard to swap models or providers

## Decision

Treat embeddings as **derived, asynchronously maintainable data** with explicit lifecycle management:

1. **Pluggable embedding providers** - Support local and remote models
2. **Async-first generation** - Background jobs for embedding creation
3. **Optional sync mode** - For latency-sensitive applications
4. **Model versioning** - Track which model generated each embedding
5. **Incremental re-embedding** - Update embeddings when models change
6. **Caching and batching** - Optimize for cost and performance

## Decision Drivers

* **Flexibility** - Support multiple embedding providers
* **Performance** - Don't block writes on slow inference
* **Cost efficiency** - Batch operations, cache results
* **Reproducibility** - Track model versions

## Architecture

### Embedding Pipeline Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Application Layer                           │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Document Insert     │
                    │  (with text field)    │
                    └───────────────────────┘
                                │
                ┌───────────────┴───────────────┐
                ▼                               ▼
    ┌───────────────────────┐       ┌───────────────────────┐
    │  Synchronous Path     │       │  Asynchronous Path    │
    │  (optional)           │       │  (default)            │
    └───────────────────────┘       └───────────────────────┘
                │                               │
                ▼                               ▼
    ┌───────────────────────┐       ┌───────────────────────┐
    │  Generate Embedding   │       │  Queue Embedding Job  │
    │  (blocks write)       │       │  (non-blocking)       │
    └───────────────────────┘       └───────────────────────┘
                │                               │
                │                               ▼
                │                   ┌───────────────────────┐
                │                   │  Background Worker    │
                │                   │  Pool                 │
                │                   └───────────────────────┘
                │                               │
                │                               ▼
                │                   ┌───────────────────────┐
                │                   │  Batch Processor      │
                │                   │  (groups jobs)        │
                │                   └───────────────────────┘
                │                               │
                └───────────────┬───────────────┘
                                ▼
                    ┌───────────────────────┐
                    │  Embedding Provider   │
                    │  Interface            │
                    └───────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│   Local      │      │   OpenAI     │      │   Custom     │
│   Model      │      │   API        │      │   Provider   │
│              │      │              │      │              │
│ • ONNX       │      │ • REST API   │      │ • gRPC       │
│ • Candle     │      │ • Batching   │      │ • HTTP       │
│ • llama.cpp  │      │ • Retry      │      │ • Plugin     │
└──────────────┘      └──────────────┘      └──────────────┘
        │                       │                       │
        └───────────────────────┴───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Store Embedding      │
                    │  + Metadata           │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Update Vector Index  │
                    │  (HNSW)               │
                    └───────────────────────┘
```

### Embedding Lifecycle States

```
┌─────────────┐
│   PENDING   │  Initial state when document inserted
└─────────────┘
       │
       │ Background worker picks up job
       ▼
┌─────────────┐
│ PROCESSING  │  Embedding generation in progress
└─────────────┘
       │
       ├─────────────┐
       ▼             ▼
┌─────────────┐  ┌─────────────┐
│  COMPLETED  │  │   FAILED    │
└─────────────┘  └─────────────┘
       │             │
       │             │ Retry logic
       │             ▼
       │         ┌─────────────┐
       │         │  RETRYING   │
       │         └─────────────┘
       │             │
       │             └──────────┐
       ▼                        ▼
┌─────────────┐          ┌─────────────┐
│   INDEXED   │          │  PERMANENT  │
│             │          │   FAILURE   │
│ (searchable)│          └─────────────┘
└─────────────┘
       │
       │ Model version changes
       ▼
┌─────────────┐
│   STALE     │  Needs re-embedding
└─────────────┘
       │
       │ Re-embedding triggered
       ▼
┌─────────────┐
│ PROCESSING  │  (cycle repeats)
└─────────────┘
```

### Model Version Management

```
Document Timeline:

Time: T0
┌──────────────────────────────────────┐
│ Document: "Machine learning basics"  │
│ Embedding: [0.1, 0.2, ..., 0.8]     │
│ Model: text-embedding-ada-002-v1     │
│ Version: 1                           │
└──────────────────────────────────────┘

Time: T1 (Model upgrade)
┌──────────────────────────────────────┐
│ Document: "Machine learning basics"  │
│ Embedding: [0.1, 0.2, ..., 0.8]     │ ← Old embedding (STALE)
│ Model: text-embedding-ada-002-v1     │
│ Version: 1                           │
│                                      │
│ New Model Available:                 │
│   text-embedding-ada-002-v2          │
│   Version: 2                         │
└──────────────────────────────────────┘

Time: T2 (Re-embedding complete)
┌──────────────────────────────────────┐
│ Document: "Machine learning basics"  │
│ Embedding: [0.15, 0.25, ..., 0.85]  │ ← New embedding
│ Model: text-embedding-ada-002-v2     │
│ Version: 2                           │
│                                      │
│ Old Embedding (archived):            │
│   [0.1, 0.2, ..., 0.8]              │
│   Model: v1                          │
└──────────────────────────────────────┘
```

### Batch Processing Flow

```
Individual Jobs:
┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐
│Job1│ │Job2│ │Job3│ │Job4│ │Job5│
└────┘ └────┘ └────┘ └────┘ └────┘
  │      │      │      │      │
  └──────┴──────┴──────┴──────┘
              │
              ▼
    ┌─────────────────┐
    │  Batch Builder  │
    │  (wait 100ms or │
    │   32 items)     │
    └─────────────────┘
              │
              ▼
    ┌─────────────────┐
    │  Batch Request  │
    │  [Job1...Job5]  │
    └─────────────────┘
              │
              ▼
    ┌─────────────────┐
    │  Provider API   │
    │  (single call)  │
    └─────────────────┘
              │
              ▼
    ┌─────────────────┐
    │ Batch Response  │
    │ [Emb1...Emb5]   │
    └─────────────────┘
              │
    ┌─────────┴─────────┐
    ▼         ▼         ▼
┌────────┐ ┌────────┐ ┌────────┐
│ Store  │ │ Store  │ │ Store  │
│  Emb1  │ │  Emb2  │ │  Emb3  │
└────────┘ └────────┘ └────────┘

Benefits:
• Reduced API calls (5 → 1)
• Lower latency per item
• Better throughput
• Cost savings
```

### Caching Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                    Embedding Request                         │
│                "Machine learning basics"                     │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │  Check Cache          │
                │  (content hash)       │
                └───────────────────────┘
                            │
                ┌───────────┴───────────┐
                ▼                       ▼
        ┌──────────────┐        ┌──────────────┐
        │  Cache HIT   │        │  Cache MISS  │
        └──────────────┘        └──────────────┘
                │                       │
                │                       ▼
                │           ┌───────────────────────┐
                │           │  Generate Embedding   │
                │           │  (via provider)       │
                │           └───────────────────────┘
                │                       │
                │                       ▼
                │           ┌───────────────────────┐
                │           │  Store in Cache       │
                │           │  (with TTL)           │
                │           └───────────────────────┘
                │                       │
                └───────────┬───────────┘
                            ▼
                ┌───────────────────────┐
                │  Return Embedding     │
                └───────────────────────┘

Cache Key: hash(model_id + model_version + content)
Cache TTL: 7 days (configurable)
Cache Size: LRU eviction, 10GB default
```

### Re-embedding Strategy

```
Scenario: Model upgrade from v1 to v2

Step 1: Identify stale embeddings
┌─────────────────────────────────────┐
│  SELECT * FROM embeddings           │
│  WHERE model_version < 2            │
│  ORDER BY last_accessed DESC        │
└─────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│  Priority Queue                     │
│  1. Recently accessed (hot)         │
│  2. Frequently queried              │
│  3. High importance documents       │
│  4. Everything else (cold)          │
└─────────────────────────────────────┘

Step 2: Incremental re-embedding
┌─────────────────────────────────────┐
│  Background Worker                  │
│  • Process in batches               │
│  • Rate limit to avoid overload     │
│  • Pause during peak hours          │
│  • Resume on restart                │
└─────────────────────────────────────┘

Step 3: Dual-version support
┌─────────────────────────────────────┐
│  Query Time                         │
│  • Search both v1 and v2 indexes    │
│  • Merge results                    │
│  • Gradually phase out v1           │
└─────────────────────────────────────┘

Step 4: Cleanup
┌─────────────────────────────────────┐
│  After 100% migration               │
│  • Archive old embeddings           │
│  • Drop v1 index                    │
│  • Update metadata                  │
└─────────────────────────────────────┘
```

### Error Handling and Retry Logic

```
Embedding Generation Attempt:

Attempt 1:
┌─────────────┐
│  Generate   │ ──X──▶ Network Error
└─────────────┘
       │
       │ Wait: 1 second
       ▼
Attempt 2:
┌─────────────┐
│  Generate   │ ──X──▶ Rate Limit (429)
└─────────────┘
       │
       │ Wait: 5 seconds (exponential backoff)
       ▼
Attempt 3:
┌─────────────┐
│  Generate   │ ──X──▶ Timeout
└─────────────┘
       │
       │ Wait: 15 seconds
       ▼
Attempt 4:
┌─────────────┐
│  Generate   │ ──✓──▶ Success
└─────────────┘
       │
       ▼
┌─────────────┐
│   Store     │
└─────────────┘

Retry Policy:
• Max attempts: 5
• Backoff: exponential (1s, 2s, 4s, 8s, 16s)
• Jitter: ±20% to avoid thundering herd
• Circuit breaker: pause after 10 consecutive failures
• Dead letter queue: permanent failures for manual review
```

* **Operational simplicity** - Automatic background processing
* **User experience** - Fast writes, eventual consistency acceptable

## Design

### 1. Embedding Provider Interface

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Get provider metadata
    fn metadata(&self) -> ProviderMetadata;
    
    /// Generate embedding for single text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    
    /// Generate embeddings for batch of texts (more efficient)
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    
    /// Get embedding dimensions
    fn dimensions(&self) -> usize;
    
    /// Get model identifier
    fn model_id(&self) -> &str;
    
    /// Check if provider is available
    async fn health_check(&self) -> Result<()>;
}

pub struct ProviderMetadata {
    pub name: String,
    pub model_id: String,
    pub dimensions: usize,
    pub max_batch_size: usize,
    pub cost_per_token: Option<f64>,
}
```

### 2. Built-in Providers

#### Local Model Provider

```rust
pub struct LocalEmbeddingProvider {
    model: Box<dyn LocalModel>,
    dimensions: usize,
    model_id: String,
}

impl LocalEmbeddingProvider {
    pub fn new(model_path: &Path) -> Result<Self> {
        // Load model from disk (e.g., ONNX, SafeTensors)
        let model = load_model(model_path)?;
        
        Ok(LocalEmbeddingProvider {
            model,
            dimensions: model.output_dimensions(),
            model_id: format!("local:{}", model_path.display()),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for LocalEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize
        let tokens = self.model.tokenize(text)?;
        
        // Run inference
        let embedding = self.model.forward(&tokens)?;
        
        Ok(embedding)
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Batch tokenization
        let token_batches = texts.iter()
            .map(|t| self.model.tokenize(t))
            .collect::<Result<Vec<_>>>()?;
        
        // Batch inference
        self.model.forward_batch(&token_batches)
    }
    
    fn dimensions(&self) -> usize {
        self.dimensions
    }
    
    fn model_id(&self) -> &str {
        &self.model_id
    }
    
    async fn health_check(&self) -> Result<()> {
        // Test inference with dummy input
        self.embed("test").await?;
        Ok(())
    }
}
```

#### OpenAI Provider

```rust
pub struct OpenAIEmbeddingProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimensions: usize,
}

impl OpenAIEmbeddingProvider {
    pub fn new(api_key: String, model: String) -> Self {
        OpenAIEmbeddingProvider {
            client: reqwest::Client::new(),
            api_key,
            dimensions: match model.as_str() {
                "text-embedding-3-small" => 1536,
                "text-embedding-3-large" => 3072,
                _ => 1536,
            },
            model,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAIEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let response = self.client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "input": text,
                "model": self.model,
            }))
            .send()
            .await?;
        
        let data: OpenAIResponse = response.json().await?;
        Ok(data.data[0].embedding.clone())
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // OpenAI supports batch requests
        let response = self.client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "input": texts,
                "model": self.model,
            }))
            .send()
            .await?;
        
        let data: OpenAIResponse = response.json().await?;
        Ok(data.data.into_iter().map(|d| d.embedding).collect())
    }
    
    fn dimensions(&self) -> usize {
        self.dimensions
    }
    
    fn model_id(&self) -> &str {
        &self.model
    }
    
    async fn health_check(&self) -> Result<()> {
        self.embed("test").await?;
        Ok(())
    }
}
```

### 3. Embedding Manager

```rust
pub struct EmbeddingManager {
    providers: HashMap<String, Arc<dyn EmbeddingProvider>>,
    job_queue: Arc<EmbeddingJobQueue>,
    cache: Arc<EmbeddingCache>,
}

impl EmbeddingManager {
    pub fn register_provider(&mut self, name: String, provider: Arc<dyn EmbeddingProvider>) {
        self.providers.insert(name, provider);
    }
    
    pub async fn embed_sync(&self, provider: &str, text: &str) -> Result<Vec<f32>> {
        // Check cache first
        if let Some(cached) = self.cache.get(provider, text).await? {
            return Ok(cached);
        }
        
        // Generate embedding
        let provider = self.get_provider(provider)?;
        let embedding = provider.embed(text).await?;
        
        // Cache result
        self.cache.put(provider.model_id(), text, &embedding).await?;
        
        Ok(embedding)
    }
    
    pub async fn embed_async(&self, provider: &str, text: String, metadata: EmbeddingMetadata) -> Result<JobId> {
        // Queue embedding job
        let job = EmbeddingJob {
            provider: provider.to_string(),
            text,
            metadata,
            status: JobStatus::Pending,
            created_at: now(),
        };
        
        let job_id = self.job_queue.enqueue(job).await?;
        Ok(job_id)
    }
    
    pub async fn embed_batch_async(&self, provider: &str, items: Vec<(String, EmbeddingMetadata)>) -> Result<Vec<JobId>> {
        let mut job_ids = Vec::new();
        
        for (text, metadata) in items {
            let job_id = self.embed_async(provider, text, metadata).await?;
            job_ids.push(job_id);
        }
        
        Ok(job_ids)
    }
}
```

### 4. Embedding Storage

```rust
pub struct EmbeddingRecord {
    pub id: EmbeddingId,
    pub vector: Vec<f32>,
    pub source_id: DocumentId,
    pub source_field: String,
    pub model_id: String,
    pub model_version: String,
    pub created_at: Timestamp,
    pub metadata: HashMap<String, Value>,
}

impl EmbeddingRecord {
    pub fn store(&self, db: &Database) -> Result<()> {
        // Store in vector collection
        db.vectors()
            .collection("embeddings")
            .insert()
            .vector(self.vector.clone())
            .metadata(json!({
                "source_id": self.source_id,
                "source_field": self.source_field,
                "model_id": self.model_id,
                "model_version": self.model_version,
                "created_at": self.created_at,
            }))
            .execute()
    }
}
```

### 5. Background Job Processing

```rust
pub struct EmbeddingWorker {
    manager: Arc<EmbeddingManager>,
    job_queue: Arc<EmbeddingJobQueue>,
    db: Arc<Database>,
    concurrency: usize,
}

impl EmbeddingWorker {
    pub async fn run(&self) -> Result<()> {
        loop {
            // Fetch batch of jobs
            let jobs = self.job_queue.dequeue_batch(self.concurrency).await?;
            
            if jobs.is_empty() {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            
            // Process jobs concurrently
            let futures = jobs.into_iter().map(|job| {
                let manager = self.manager.clone();
                let db = self.db.clone();
                
                async move {
                    self.process_job(job, &manager, &db).await
                }
            });
            
            let results = futures::future::join_all(futures).await;
            
            // Handle results
            for result in results {
                if let Err(e) = result {
                    eprintln!("Embedding job failed: {}", e);
                }
            }
        }
    }
    
    async fn process_job(&self, job: EmbeddingJob, manager: &EmbeddingManager, db: &Database) -> Result<()> {
        // Update job status
        self.job_queue.update_status(job.id, JobStatus::Processing).await?;
        
        // Generate embedding
        let embedding = manager.embed_sync(&job.provider, &job.text).await?;
        
        // Store embedding
        let record = EmbeddingRecord {
            id: EmbeddingId::new(),
            vector: embedding,
            source_id: job.metadata.source_id,
            source_field: job.metadata.source_field,
            model_id: job.provider.clone(),
            model_version: manager.get_provider(&job.provider)?.model_id().to_string(),
            created_at: now(),
            metadata: job.metadata.extra,
        };
        
        record.store(db)?;
        
        // Mark job complete
        self.job_queue.update_status(job.id, JobStatus::Completed).await?;
        
        Ok(())
    }
}
```

### 6. Automatic Embedding Triggers

```rust
pub struct EmbeddingTrigger {
    collection: CollectionId,
    field: String,
    provider: String,
    mode: TriggerMode,
}

pub enum TriggerMode {
    Sync,   // Generate immediately
    Async,  // Queue for background processing
}

impl Database {
    pub async fn create_embedding_trigger(&self, trigger: EmbeddingTrigger) -> Result<()> {
        // Register trigger
        self.triggers.register(trigger.clone())?;
        
        // Hook into document insert/update
        self.on_document_change(move |doc| {
            if let Some(text) = doc.get(&trigger.field) {
                match trigger.mode {
                    TriggerMode::Sync => {
                        // Generate embedding synchronously
                        let embedding = self.embedding_manager
                            .embed_sync(&trigger.provider, text)
                            .await?;
                        
                        // Store with document
                        doc.set_embedding(&trigger.field, embedding);
                    }
                    TriggerMode::Async => {
                        // Queue for background processing
                        self.embedding_manager.embed_async(
                            &trigger.provider,
                            text.to_string(),
                            EmbeddingMetadata {
                                source_id: doc.id,
                                source_field: trigger.field.clone(),
                                extra: HashMap::new(),
                            }
                        ).await?;
                    }
                }
            }
            Ok(())
        });
        
        Ok(())
    }
}
```

### 7. Model Versioning and Re-embedding

```rust
pub struct ModelVersion {
    pub model_id: String,
    pub version: String,
    pub created_at: Timestamp,
    pub dimensions: usize,
}

impl EmbeddingManager {
    pub async fn upgrade_model(&self, old_model: &str, new_model: &str) -> Result<ReembeddingJob> {
        // Create re-embedding job
        let job = ReembeddingJob {
            old_model: old_model.to_string(),
            new_model: new_model.to_string(),
            status: ReembeddingStatus::Pending,
            progress: 0,
            total: 0,
        };
        
        // Find all embeddings with old model
        let old_embeddings = self.db.vectors()
            .collection("embeddings")
            .query()
            .filter(Filter::eq("model_id", old_model))
            .execute()
            .await?;
        
        job.total = old_embeddings.len();
        
        // Queue re-embedding jobs
        for embedding in old_embeddings {
            let source_text = self.get_source_text(embedding.metadata["source_id"]).await?;
            
            self.embed_async(
                new_model,
                source_text,
                EmbeddingMetadata {
                    source_id: embedding.metadata["source_id"],
                    source_field: embedding.metadata["source_field"],
                    extra: HashMap::new(),
                }
            ).await?;
        }
        
        Ok(job)
    }
}
```

### 8. Embedding Cache

```rust
pub struct EmbeddingCache {
    cache: Arc<RwLock<LruCache<CacheKey, Vec<f32>>>>,
    ttl: Duration,
}

struct CacheKey {
    model_id: String,
    text_hash: u64,
}

impl EmbeddingCache {
    pub async fn get(&self, model_id: &str, text: &str) -> Result<Option<Vec<f32>>> {
        let key = CacheKey {
            model_id: model_id.to_string(),
            text_hash: hash_text(text),
        };
        
        let cache = self.cache.read().await;
        Ok(cache.get(&key).cloned())
    }
    
    pub async fn put(&self, model_id: &str, text: &str, embedding: &[f32]) -> Result<()> {
        let key = CacheKey {
            model_id: model_id.to_string(),
            text_hash: hash_text(text),
        };
        
        let mut cache = self.cache.write().await;
        cache.put(key, embedding.to_vec());
        
        Ok(())
    }
}
```

### 9. Cost Tracking

```rust
pub struct EmbeddingCostTracker {
    costs: Arc<RwLock<HashMap<String, CostMetrics>>>,
}

pub struct CostMetrics {
    pub total_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub last_updated: Timestamp,
}

impl EmbeddingCostTracker {
    pub async fn record_usage(&self, provider: &str, tokens: u64, cost: f64) {
        let mut costs = self.costs.write().await;
        let metrics = costs.entry(provider.to_string()).or_insert_with(|| CostMetrics {
            total_requests: 0,
            total_tokens: 0,
            total_cost: 0.0,
            last_updated: now(),
        });
        
        metrics.total_requests += 1;
        metrics.total_tokens += tokens;
        metrics.total_cost += cost;
        metrics.last_updated = now();
    }
    
    pub async fn get_metrics(&self, provider: &str) -> Option<CostMetrics> {
        let costs = self.costs.read().await;
        costs.get(provider).cloned()
    }
}
```

## Consequences

### Positive

* **Flexibility** - Easy to swap embedding providers
* **Performance** - Async generation doesn't block writes
* **Cost efficiency** - Caching and batching reduce API costs
* **Reproducibility** - Model versioning enables re-embedding
* **Scalability** - Background workers can scale independently
* **User experience** - Fast writes, eventual consistency
* **Operational simplicity** - Automatic background processing

### Negative

* **Eventual consistency** - Embeddings may lag behind source data
* **Complexity** - More moving parts (workers, queues, cache)
* **Storage overhead** - Multiple model versions consume space
* **Debugging** - Async processing harder to debug

### Risks

* **Queue backlog** - Slow embedding generation can cause delays
* **Model drift** - Different model versions may produce incompatible embeddings
* **Cost overruns** - External API costs can grow unexpectedly
* **Cache invalidation** - Stale cache entries can cause issues

## Alternatives Considered

### 1. Synchronous Embeddings Only

**Rejected** - Blocks writes, poor user experience, doesn't scale.

### 2. External Service Only

**Rejected** - Expensive, network dependent, no offline support.

### 3. No Model Versioning

**Rejected** - Makes model upgrades impossible without data loss.

### 4. Tight Coupling to Specific Provider

**Rejected** - Limits flexibility, vendor lock-in.

## Implementation Notes

### Phase 1: Provider Interface (Week 26)
- Define embedding provider trait
- Implement local model provider
- Add OpenAI provider

### Phase 2: Async Processing (Week 27)
- Implement job queue
- Create background workers
- Add job monitoring

### Phase 3: Caching and Optimization (Week 28)
- Implement embedding cache
- Add batch processing
- Create cost tracking

### Phase 4: Model Management (Week 29)
- Add model versioning
- Implement re-embedding
- Create migration tools

## Related ADRs

* [ADR-0008: Indexing Options](ADR-0008-Indexing-Options.md)
* [ADR-0017: Hybrid Query Execution](ADR-0017-Hybrid-Query-Execution.md)
* [ADR-0019: Semantic Ranking and Scoring Strategy](ADR-0019-Semantic-Ranking-and-Scoring-Strategy.md)
* [ADR-0025: Core API Specifications](ADR-0025-Core-API-Specifications.md)

## References

* OpenAI Embeddings API
* Sentence Transformers
* ONNX Runtime
* Hugging Face model hub
* Vector database best practices

---

**Next Steps:**
1. Define embedding provider interface
2. Implement local model support (ONNX)
3. Add OpenAI provider
4. Create job queue system
5. Implement background workers
6. Add caching layer
7. Create model versioning system
