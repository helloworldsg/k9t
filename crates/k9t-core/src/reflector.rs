use std::sync::Arc;

use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client, Resource, runtime::{watcher, reflector, WatchStreamExt}};
use serde::de::DeserializeOwned;

use crate::resource::PodInfo;

pub struct K9sReflector<K>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: Eq + std::hash::Hash + Clone + Default + Send,
{
    reader: reflector::Store<K>,
}

impl<K> K9sReflector<K>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: Eq + std::hash::Hash + Clone + Default + Send,
{
    pub fn start(client: Client) -> anyhow::Result<Self> {
        let api: Api<K> = Api::all(client);
        let (reader, writer) = reflector::store();

        tokio::spawn(async move {
            watcher(api, watcher::Config::default())
                .default_backoff()
                .reflect(writer)
                .applied_objects()
                .for_each(|_| std::future::ready(()))
                .await;
        });

        Ok(Self { reader })
    }

    pub fn items(&self) -> Vec<Arc<K>> {
        self.reader.state().to_vec()
    }
}

pub type PodReflector = K9sReflector<Pod>;

impl PodReflector {
    pub fn store(&self) -> Vec<PodInfo> {
        self.reader
            .state()
            .iter()
            .map(|p| PodInfo::from(p.as_ref()))
            .collect()
    }
}