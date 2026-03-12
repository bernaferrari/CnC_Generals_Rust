//! Pipeline Orchestration
//!
//! This module provides orchestration and coordination for the asset processing pipeline.
//! It manages the flow of assets through import -> process -> export stages.

use crate::{Asset, AssetError, ProcessingJob, ProcessingResult, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Pipeline stage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Queued,
    Importing,
    Processing,
    Exporting,
    Complete,
    Failed,
}

/// Pipeline job tracker
#[derive(Debug, Clone)]
pub struct PipelineJobStatus {
    pub stage: PipelineStage,
    pub progress: f32,
    pub message: String,
    pub error: Option<String>,
}

impl Default for PipelineJobStatus {
    fn default() -> Self {
        Self {
            stage: PipelineStage::Queued,
            progress: 0.0,
            message: "Queued".to_string(),
            error: None,
        }
    }
}

/// Pipeline coordinator
#[derive(Debug)]
pub struct PipelineCoordinator {
    jobs: Arc<RwLock<HashMap<uuid::Uuid, PipelineJobStatus>>>,
    max_concurrent: usize,
}

impl PipelineCoordinator {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent,
        }
    }

    /// Register a new job
    pub async fn register_job(&self, job_id: uuid::Uuid) {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job_id, PipelineJobStatus::default());
    }

    /// Update job status
    pub async fn update_status(
        &self,
        job_id: uuid::Uuid,
        stage: PipelineStage,
        progress: f32,
        message: String,
    ) {
        let mut jobs = self.jobs.write().await;
        if let Some(status) = jobs.get_mut(&job_id) {
            status.stage = stage;
            status.progress = progress;
            status.message = message;
        }
    }

    /// Mark job as failed
    pub async fn fail_job(&self, job_id: uuid::Uuid, error: String) {
        let mut jobs = self.jobs.write().await;
        if let Some(status) = jobs.get_mut(&job_id) {
            status.stage = PipelineStage::Failed;
            status.error = Some(error);
        }
    }

    /// Get job status
    pub async fn get_status(&self, job_id: uuid::Uuid) -> Option<PipelineJobStatus> {
        let jobs = self.jobs.read().await;
        jobs.get(&job_id).cloned()
    }

    /// Get all job statuses
    pub async fn get_all_statuses(&self) -> HashMap<uuid::Uuid, PipelineJobStatus> {
        let jobs = self.jobs.read().await;
        jobs.clone()
    }

    /// Remove completed jobs
    pub async fn cleanup_completed(&self) {
        let mut jobs = self.jobs.write().await;
        jobs.retain(|_, status| {
            !matches!(
                status.stage,
                PipelineStage::Complete | PipelineStage::Failed
            )
        });
    }

    /// Get number of active jobs
    pub async fn active_count(&self) -> usize {
        let jobs = self.jobs.read().await;
        jobs.iter()
            .filter(|(_, status)| {
                !matches!(
                    status.stage,
                    PipelineStage::Complete | PipelineStage::Failed
                )
            })
            .count()
    }

    /// Check if can accept new job
    pub async fn can_accept(&self) -> bool {
        self.active_count().await < self.max_concurrent
    }
}

/// Pipeline batch processor
#[derive(Debug)]
pub struct BatchProcessor {
    batch_size: usize,
    retry_count: u32,
    retry_delay_ms: u64,
}

impl BatchProcessor {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            retry_count: 3,
            retry_delay_ms: 1000,
        }
    }

    pub fn with_retries(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    pub fn with_retry_delay(mut self, delay_ms: u64) -> Self {
        self.retry_delay_ms = delay_ms;
        self
    }

    /// Process batch with retries
    pub async fn process_batch<F, Fut>(
        &self,
        items: Vec<ProcessingJob>,
        processor: F,
    ) -> Vec<Result<ProcessingResult>>
    where
        F: Fn(ProcessingJob) -> Fut,
        Fut: std::future::Future<Output = Result<ProcessingResult>>,
    {
        let mut results = Vec::new();

        for chunk in items.chunks(self.batch_size) {
            for job in chunk {
                let mut attempts = 0;
                let mut last_error = None;

                while attempts < self.retry_count {
                    match processor(job.clone()).await {
                        Ok(result) => {
                            results.push(Ok(result));
                            break;
                        }
                        Err(e) => {
                            attempts += 1;
                            last_error = Some(e);

                            if attempts < self.retry_count {
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    self.retry_delay_ms,
                                ))
                                .await;
                            }
                        }
                    }
                }

                if attempts >= self.retry_count {
                    if let Some(err) = last_error {
                        results.push(Err(err));
                    }
                }
            }
        }

        results
    }
}

/// Pipeline dependency resolver
#[derive(Debug)]
pub struct DependencyResolver {
    dependencies: HashMap<uuid::Uuid, Vec<uuid::Uuid>>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
        }
    }

    /// Add dependency relationship
    pub fn add_dependency(&mut self, asset_id: uuid::Uuid, depends_on: uuid::Uuid) {
        self.dependencies
            .entry(asset_id)
            .or_insert_with(Vec::new)
            .push(depends_on);
    }

    /// Get dependencies for an asset
    pub fn get_dependencies(&self, asset_id: &uuid::Uuid) -> Vec<uuid::Uuid> {
        self.dependencies.get(asset_id).cloned().unwrap_or_default()
    }

    /// Resolve processing order (topological sort)
    pub fn resolve_order(&self, assets: &[uuid::Uuid]) -> Result<Vec<uuid::Uuid>> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        for asset in assets {
            if !visited.contains(asset) {
                self.visit(*asset, &mut visited, &mut visiting, &mut result)?;
            }
        }

        Ok(result)
    }

    fn visit(
        &self,
        asset: uuid::Uuid,
        visited: &mut std::collections::HashSet<uuid::Uuid>,
        visiting: &mut std::collections::HashSet<uuid::Uuid>,
        result: &mut Vec<uuid::Uuid>,
    ) -> Result<()> {
        if visiting.contains(&asset) {
            return Err(AssetError::ProcessingFailed(
                "Circular dependency detected".to_string(),
            ));
        }

        if visited.contains(&asset) {
            return Ok(());
        }

        visiting.insert(asset);

        for dep in self.get_dependencies(&asset) {
            self.visit(dep, visited, visiting, result)?;
        }

        visiting.remove(&asset);
        visited.insert(asset);
        result.push(asset);

        Ok(())
    }
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress reporter
#[derive(Clone)]
pub struct ProgressReporter {
    total: usize,
    current: usize,
    callback: Option<Arc<dyn Fn(f32, String) + Send + Sync>>,
}

impl ProgressReporter {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            current: 0,
            callback: None,
        }
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(f32, String) + Send + Sync + 'static,
    {
        self.callback = Some(Arc::new(callback));
        self
    }

    pub fn increment(&mut self, message: String) {
        self.current += 1;
        let progress = if self.total > 0 {
            self.current as f32 / self.total as f32
        } else {
            0.0
        };

        if let Some(callback) = &self.callback {
            callback(progress, message);
        }
    }

    pub fn set_progress(&mut self, current: usize, message: String) {
        self.current = current;
        let progress = if self.total > 0 {
            self.current as f32 / self.total as f32
        } else {
            0.0
        };

        if let Some(callback) = &self.callback {
            callback(progress, message);
        }
    }

    pub fn progress(&self) -> f32 {
        if self.total > 0 {
            self.current as f32 / self.total as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_coordinator() {
        let coordinator = PipelineCoordinator::new(4);
        let job_id = uuid::Uuid::new_v4();

        coordinator.register_job(job_id).await;

        let status = coordinator.get_status(job_id).await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().stage, PipelineStage::Queued);

        coordinator
            .update_status(
                job_id,
                PipelineStage::Processing,
                0.5,
                "Processing".to_string(),
            )
            .await;

        let status = coordinator.get_status(job_id).await.unwrap();
        assert_eq!(status.stage, PipelineStage::Processing);
        assert_eq!(status.progress, 0.5);
    }

    #[tokio::test]
    async fn test_coordinator_active_count() {
        let coordinator = PipelineCoordinator::new(2);

        let job1 = uuid::Uuid::new_v4();
        let job2 = uuid::Uuid::new_v4();

        coordinator.register_job(job1).await;
        coordinator.register_job(job2).await;

        assert_eq!(coordinator.active_count().await, 2);

        coordinator
            .update_status(job1, PipelineStage::Complete, 1.0, "Done".to_string())
            .await;

        assert_eq!(coordinator.active_count().await, 1);
    }

    #[test]
    fn test_dependency_resolver() {
        let mut resolver = DependencyResolver::new();

        let asset1 = uuid::Uuid::new_v4();
        let asset2 = uuid::Uuid::new_v4();
        let asset3 = uuid::Uuid::new_v4();

        // asset3 depends on asset2, asset2 depends on asset1
        resolver.add_dependency(asset2, asset1);
        resolver.add_dependency(asset3, asset2);

        let order = resolver.resolve_order(&[asset3, asset2, asset1]).unwrap();

        // Should process in order: asset1, asset2, asset3
        assert_eq!(order[0], asset1);
        assert_eq!(order[1], asset2);
        assert_eq!(order[2], asset3);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut resolver = DependencyResolver::new();

        let asset1 = uuid::Uuid::new_v4();
        let asset2 = uuid::Uuid::new_v4();

        // Create circular dependency
        resolver.add_dependency(asset1, asset2);
        resolver.add_dependency(asset2, asset1);

        let result = resolver.resolve_order(&[asset1, asset2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_progress_reporter() {
        let mut reporter = ProgressReporter::new(10);

        assert_eq!(reporter.progress(), 0.0);

        reporter.increment("Step 1".to_string());
        assert_eq!(reporter.progress(), 0.1);

        reporter.set_progress(5, "Half done".to_string());
        assert_eq!(reporter.progress(), 0.5);
    }

    #[test]
    fn test_batch_processor_creation() {
        let processor = BatchProcessor::new(10);
        assert_eq!(processor.batch_size, 10);
        assert_eq!(processor.retry_count, 3);
    }
}
