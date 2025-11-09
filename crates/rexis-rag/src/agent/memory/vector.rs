//! Vector embeddings and similarity search for semantic memory
//!
//! Provides embedding generation and vector similarity search capabilities
//! for semantic memory facts. Supports multiple embedding backends.

use crate::error::{RragError, RragResult};
use serde::{Deserialize, Serialize};

/// A vector embedding (dense float vector)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// The vector dimensions
    pub vector: Vec<f32>,

    /// Dimensionality of the embedding
    pub dimensions: usize,

    /// Model used to generate the embedding
    pub model: String,
}

impl Embedding {
    /// Create a new embedding
    pub fn new(vector: Vec<f32>, model: impl Into<String>) -> Self {
        let dimensions = vector.len();
        Self {
            vector,
            dimensions,
            model: model.into(),
        }
    }

    /// Calculate cosine similarity with another embedding
    pub fn cosine_similarity(&self, other: &Embedding) -> RragResult<f32> {
        if self.dimensions != other.dimensions {
            return Err(RragError::validation(
                "embedding_dimensions",
                "dimensions must match",
                format!("{} vs {}", self.dimensions, other.dimensions),
            ));
        }

        let dot_product: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f32 = self.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.vector.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(0.0);
        }

        Ok(dot_product / (norm_a * norm_b))
    }

    /// Calculate Euclidean distance to another embedding
    pub fn euclidean_distance(&self, other: &Embedding) -> RragResult<f32> {
        if self.dimensions != other.dimensions {
            return Err(RragError::validation(
                "embedding_dimensions",
                "dimensions must match",
                format!("{} vs {}", self.dimensions, other.dimensions),
            ));
        }

        let sum_of_squares: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        Ok(sum_of_squares.sqrt())
    }
}

/// Trait for embedding generation backends
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for the given text
    async fn embed(&self, text: &str) -> RragResult<Embedding>;

    /// Get the model name
    fn model_name(&self) -> &str;

    /// Get the embedding dimensions
    fn dimensions(&self) -> usize;
}

/// Simple embedding provider that uses hash-based vectors (for testing/demo)
pub struct HashEmbeddingProvider {
    dimensions: usize,
}

impl HashEmbeddingProvider {
    /// Create a new hash-based embedding provider
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }

    /// Generate a simple hash-based embedding (NOT for production!)
    fn hash_embed(&self, text: &str) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut vector = vec![0.0; self.dimensions];

        // Use multiple hash functions with different seeds
        for i in 0..self.dimensions {
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            i.hash(&mut hasher);

            let hash = hasher.finish();
            // Normalize to [-1, 1]
            vector[i] = ((hash as f32) / (u64::MAX as f32)) * 2.0 - 1.0;
        }

        vector
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for HashEmbeddingProvider {
    async fn embed(&self, text: &str) -> RragResult<Embedding> {
        let vector = self.hash_embed(text);
        Ok(Embedding::new(vector, "hash"))
    }

    fn model_name(&self) -> &str {
        "hash-embedding"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Search result with similarity score
#[derive(Debug, Clone)]
pub struct SearchResult<T> {
    /// The item that was found
    pub item: T,

    /// Similarity score (0.0 to 1.0, higher is more similar)
    pub score: f32,

    /// Distance metric (if applicable)
    pub distance: Option<f32>,
}

impl<T> SearchResult<T> {
    /// Create a new search result
    pub fn new(item: T, score: f32) -> Self {
        Self {
            item,
            score,
            distance: None,
        }
    }

    /// Set the distance metric
    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = Some(distance);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_cosine_similarity() {
        let emb1 = Embedding::new(vec![1.0, 0.0, 0.0], "test");
        let emb2 = Embedding::new(vec![1.0, 0.0, 0.0], "test");
        let emb3 = Embedding::new(vec![0.0, 1.0, 0.0], "test");

        // Identical vectors
        let sim = emb1.cosine_similarity(&emb2).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);

        // Orthogonal vectors
        let sim = emb1.cosine_similarity(&emb3).unwrap();
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_embedding_euclidean_distance() {
        let emb1 = Embedding::new(vec![1.0, 0.0, 0.0], "test");
        let emb2 = Embedding::new(vec![1.0, 0.0, 0.0], "test");
        let emb3 = Embedding::new(vec![0.0, 1.0, 0.0], "test");

        // Identical vectors
        let dist = emb1.euclidean_distance(&emb2).unwrap();
        assert!(dist.abs() < 1e-6);

        // Distance of sqrt(2)
        let dist = emb1.euclidean_distance(&emb3).unwrap();
        assert!((dist - 1.41421356).abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_hash_embedding_provider() {
        let provider = HashEmbeddingProvider::new(128);

        let emb1 = provider.embed("Hello world").await.unwrap();
        let emb2 = provider.embed("Hello world").await.unwrap();
        let emb3 = provider.embed("Different text").await.unwrap();

        assert_eq!(emb1.dimensions, 128);

        // Same text should produce same embedding
        let sim = emb1.cosine_similarity(&emb2).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);

        // Different text should produce different embedding
        let sim = emb1.cosine_similarity(&emb3).unwrap();
        assert!(sim < 1.0);
    }
}
